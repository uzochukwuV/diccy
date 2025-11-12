use async_graphql::{Request, Response, Schema, EmptySubscription};
use battlechain_shared_types::*;
use linera_sdk::{
    base::{ChainId, Owner, Timestamp, WithContractAbi},
    views::{RootView, View, ViewStorageContext},
    Contract, Service,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Player Chain Application ABI
pub struct PlayerChainAbi;

impl WithContractAbi for PlayerChainAbi {
    type Operation = Operation;
    type Response = ();
}

/// Player Chain State
#[derive(RootView)]
pub struct PlayerChainState {
    /// Owner of this player chain
    pub owner: Owner,

    /// All characters owned by this player
    pub characters: Vec<CharacterNFT>,

    /// Player inventory (items, consumables)
    pub items: Vec<Item>,

    /// Currency balances (SOL, USDC, USDT, etc.)
    pub currencies: HashMap<Currency, u64>,

    /// Player stats
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub total_earned: HashMap<Currency, u64>,

    /// Active battles (references to battle chains)
    pub active_battles: Vec<ChainId>,

    /// Preferences
    pub default_stance: Stance,
    pub auto_play: bool,

    /// Timestamps
    pub created_at: Timestamp,
    pub last_active: Timestamp,
}

impl PlayerChainState {
    /// Initialize new player chain
    pub fn new(owner: Owner, created_at: Timestamp) -> Self {
        let mut currencies = HashMap::new();
        currencies.insert(Currency::SOL, 0);
        currencies.insert(Currency::USDC, 0);
        currencies.insert(Currency::USDT, 0);

        Self {
            owner,
            characters: Vec::new(),
            items: Vec::new(),
            currencies,
            total_battles: 0,
            wins: 0,
            losses: 0,
            total_earned: HashMap::new(),
            active_battles: Vec::new(),
            default_stance: Stance::Balanced,
            auto_play: false,
            created_at,
            last_active: created_at,
        }
    }

    /// Get character by NFT ID
    pub fn get_character(&self, nft_id: &str) -> Option<&CharacterNFT> {
        self.characters.iter().find(|c| c.nft_id == nft_id)
    }

    /// Get character mutably
    pub fn get_character_mut(&mut self, nft_id: &str) -> Option<&mut CharacterNFT> {
        self.characters.iter_mut().find(|c| c.nft_id == nft_id)
    }

    /// Add currency balance
    pub fn add_currency(&mut self, currency: Currency, amount: u64) -> Result<(), PlayerChainError> {
        let balance = self.currencies.entry(currency).or_insert(0);
        *balance = balance
            .checked_add(amount)
            .ok_or(PlayerChainError::MathOverflow)?;
        Ok(())
    }

    /// Subtract currency balance
    pub fn sub_currency(&mut self, currency: &Currency, amount: u64) -> Result<(), PlayerChainError> {
        let balance = self.currencies.get_mut(currency)
            .ok_or(PlayerChainError::InsufficientBalance)?;
        *balance = balance
            .checked_sub(amount)
            .ok_or(PlayerChainError::InsufficientBalance)?;
        Ok(())
    }

    /// Get win rate
    pub fn win_rate(&self) -> f64 {
        if self.total_battles == 0 {
            0.0
        } else {
            (self.wins as f64) / (self.total_battles as f64)
        }
    }
}

/// Operations on Player Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Create a new character from NFT
    CreateCharacter {
        nft_id: String,
        class: CharacterClass,
    },

    /// Apply trait bundle to character (requires trait authority signature)
    ApplyTraits {
        nft_id: String,
        trait_bundle: TraitBundle,
    },

    /// Update character after battle
    UpdateCharacterAfterBattle {
        nft_id: String,
        xp_gained: u64,
        hp_remaining: u32,
        did_win: bool,
    },

    /// Deposit currency
    DepositCurrency {
        currency: Currency,
        amount: u64,
    },

    /// Withdraw currency
    WithdrawCurrency {
        currency: Currency,
        amount: u64,
    },

    /// Lock currency for battle stake
    LockStakeForBattle {
        battle_id: String,
        currency: Currency,
        amount: u64,
    },

    /// Unlock stake after battle
    UnlockStake {
        battle_id: String,
        currency: Currency,
        amount: u64,
    },

    /// Update preferences
    UpdatePreferences {
        default_stance: Option<Stance>,
        auto_play: Option<bool>,
    },

    /// Add item to inventory
    AddItem {
        item: Item,
    },

    /// Remove item from inventory
    RemoveItem {
        item_id: String,
        quantity: u32,
    },
}

/// Messages received by Player Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle initialization notification
    BattleStarted {
        battle_id: String,
        battle_chain: ChainId,
        opponent: Owner,
    },

    /// Battle result notification
    BattleResult {
        battle_id: String,
        winner: Owner,
        xp_earned: u64,
        currency_won: Currency,
        amount_won: u64,
    },

    /// Currency transfer from another chain
    TransferCurrency {
        currency: Currency,
        amount: u64,
        from_chain: ChainId,
    },

    /// Character registered in global registry
    CharacterRegistered {
        nft_id: String,
        registry_chain: ChainId,
    },
}

/// Player Chain Errors
#[derive(Debug, Error)]
pub enum PlayerChainError {
    #[error("Character not found: {0}")]
    CharacterNotFound(String),

    #[error("Character already exists: {0}")]
    CharacterAlreadyExists(String),

    #[error("Character is currently in battle")]
    CharacterInBattle,

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Math overflow")]
    MathOverflow,

    #[error("Unauthorized operation")]
    Unauthorized,

    #[error("Invalid NFT")]
    InvalidNFT,

    #[error("Item not found: {0}")]
    ItemNotFound(String),
}

/// Player Chain Contract
pub struct PlayerChainContract {
    state: PlayerChainState,
    runtime: ContractRuntime<Self>,
}

impl Contract for PlayerChainContract {
    type Message = Message;
    type Parameters = Owner; // Initialize with owner
    type InstantiationArgument = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let owner = runtime.chain_ownership().owner().expect("Chain must have owner");
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let owner = self.runtime.chain_ownership().owner()
            .expect("Chain must have owner");
        let now = self.runtime.system_time();

        self.state = PlayerChainState::new(owner, now);
        self.runtime.emit(format!("Player chain initialized for owner: {}", owner));
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::CreateCharacter { nft_id, class } => {
                self.create_character(nft_id, class).await
            }
            Operation::ApplyTraits { nft_id, trait_bundle } => {
                self.apply_traits(nft_id, trait_bundle).await
            }
            Operation::UpdateCharacterAfterBattle {
                nft_id,
                xp_gained,
                hp_remaining,
                did_win,
            } => {
                self.update_character_after_battle(nft_id, xp_gained, hp_remaining, did_win)
                    .await
            }
            Operation::DepositCurrency { currency, amount } => {
                self.deposit_currency(currency, amount).await
            }
            Operation::WithdrawCurrency { currency, amount } => {
                self.withdraw_currency(currency, amount).await
            }
            Operation::LockStakeForBattle {
                battle_id,
                currency,
                amount,
            } => {
                self.lock_stake(battle_id, currency, amount).await
            }
            Operation::UnlockStake {
                battle_id,
                currency,
                amount,
            } => {
                self.unlock_stake(battle_id, currency, amount).await
            }
            Operation::UpdatePreferences {
                default_stance,
                auto_play,
            } => {
                self.update_preferences(default_stance, auto_play).await
            }
            Operation::AddItem { item } => {
                self.add_item(item).await
            }
            Operation::RemoveItem { item_id, quantity } => {
                self.remove_item(item_id, quantity).await
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleStarted {
                battle_id,
                battle_chain,
                opponent,
            } => {
                self.handle_battle_started(battle_id, battle_chain, opponent)
                    .await;
            }
            Message::BattleResult {
                battle_id,
                winner,
                xp_earned,
                currency_won,
                amount_won,
            } => {
                self.handle_battle_result(battle_id, winner, xp_earned, currency_won, amount_won)
                    .await;
            }
            Message::TransferCurrency {
                currency,
                amount,
                from_chain,
            } => {
                self.handle_transfer_currency(currency, amount, from_chain)
                    .await;
            }
            Message::CharacterRegistered {
                nft_id,
                registry_chain,
            } => {
                self.handle_character_registered(nft_id, registry_chain)
                    .await;
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl PlayerChainContract {
    /// Create a new character from NFT
    async fn create_character(&mut self, nft_id: String, class: CharacterClass) -> () {
        // Check if character already exists
        if self.state.get_character(&nft_id).is_some() {
            self.runtime.emit(format!("Character {} already exists", nft_id));
            return;
        }

        // TODO: Verify NFT ownership (would need NFT oracle or verification)
        // For now, trust the caller owns the NFT

        let now = self.runtime.system_time();
        let character = CharacterNFT::new(nft_id.clone(), class, now);

        self.state.characters.push(character.clone());
        self.state.last_active = now;

        self.runtime.emit(format!(
            "Character created: {} (class: {:?}, level: {})",
            nft_id, class, character.level
        ));

        // TODO: Send message to Registry chain to register character globally
        // self.send_message_to_registry(RegisterCharacter { character_snapshot });
    }

    /// Apply trait bundle to character
    async fn apply_traits(&mut self, nft_id: String, trait_bundle: TraitBundle) -> () {
        // TODO: Verify trait authority signature

        let character = self.state.get_character_mut(&nft_id);
        if let Some(char) = character {
            char.apply_traits(&trait_bundle);

            self.runtime.emit(format!(
                "Traits applied to {}: rarity={}, attack_bps={}, defense_bps={}, crit_bps={}",
                nft_id, trait_bundle.rarity, trait_bundle.attack_bps, trait_bundle.defense_bps, trait_bundle.crit_bps
            ));
        } else {
            self.runtime.emit(format!("Character {} not found", nft_id));
        }
    }

    /// Update character stats after battle
    async fn update_character_after_battle(
        &mut self,
        nft_id: String,
        xp_gained: u64,
        hp_remaining: u32,
        did_win: bool,
    ) -> () {
        let character = self.state.get_character_mut(&nft_id);
        if let Some(char) = character {
            char.xp += xp_gained;
            char.current_hp = hp_remaining;
            char.in_battle = false;

            // Check for level up
            let mut leveled_up = false;
            while char.level_up() {
                leveled_up = true;
                self.runtime.emit(format!(
                    "Character {} leveled up to level {}!",
                    nft_id, char.level
                ));
            }

            // Update player stats
            self.state.total_battles += 1;
            if did_win {
                self.state.wins += 1;
            } else {
                self.state.losses += 1;

                // Consume life if lost
                char.consume_life();
                if char.lives == 0 {
                    self.runtime.emit(format!("Character {} has no lives remaining!", nft_id));
                }
            }

            self.runtime.emit(format!(
                "Character {} updated: XP +{}, HP: {}/{}, Leveled up: {}",
                nft_id, xp_gained, char.current_hp, char.hp_max, leveled_up
            ));
        }
    }

    /// Deposit currency into player balance
    async fn deposit_currency(&mut self, currency: Currency, amount: u64) -> () {
        match self.state.add_currency(currency.clone(), amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Deposited {} of {:?}",
                    amount, currency
                ));
            }
            Err(e) => {
                self.runtime.emit(format!("Deposit failed: {}", e));
            }
        }
    }

    /// Withdraw currency from player balance
    async fn withdraw_currency(&mut self, currency: Currency, amount: u64) -> () {
        match self.state.sub_currency(&currency, amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Withdrew {} of {:?}",
                    amount, currency
                ));
            }
            Err(e) => {
                self.runtime.emit(format!("Withdrawal failed: {}", e));
            }
        }
    }

    /// Lock currency for battle stake
    async fn lock_stake(&mut self, battle_id: String, currency: Currency, amount: u64) -> () {
        match self.state.sub_currency(&currency, amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Locked {} of {:?} for battle {}",
                    amount, currency, battle_id
                ));
                // TODO: Send message to battle chain or matchmaking chain
            }
            Err(e) => {
                self.runtime.emit(format!("Lock stake failed: {}", e));
            }
        }
    }

    /// Unlock stake after battle cancelled
    async fn unlock_stake(&mut self, battle_id: String, currency: Currency, amount: u64) -> () {
        match self.state.add_currency(currency.clone(), amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Unlocked {} of {:?} from battle {}",
                    amount, currency, battle_id
                ));
            }
            Err(e) => {
                self.runtime.emit(format!("Unlock stake failed: {}", e));
            }
        }
    }

    /// Update player preferences
    async fn update_preferences(
        &mut self,
        default_stance: Option<Stance>,
        auto_play: Option<bool>,
    ) -> () {
        if let Some(stance) = default_stance {
            self.state.default_stance = stance;
        }
        if let Some(auto) = auto_play {
            self.state.auto_play = auto;
        }

        self.runtime.emit("Preferences updated".to_string());
    }

    /// Add item to inventory
    async fn add_item(&mut self, item: Item) -> () {
        // Check if item already exists
        if let Some(existing) = self.state.items.iter_mut().find(|i| i.item_id == item.item_id) {
            existing.quantity += item.quantity;
        } else {
            self.state.items.push(item.clone());
        }

        self.runtime.emit(format!("Added item: {} ({})", item.name, item.quantity));
    }

    /// Remove item from inventory
    async fn remove_item(&mut self, item_id: String, quantity: u32) -> () {
        if let Some(item) = self.state.items.iter_mut().find(|i| i.item_id == item_id) {
            if item.quantity >= quantity {
                item.quantity -= quantity;
                self.runtime.emit(format!("Removed {} of item {}", quantity, item_id));

                if item.quantity == 0 {
                    self.state.items.retain(|i| i.item_id != item_id);
                }
            } else {
                self.runtime.emit(format!("Insufficient quantity of item {}", item_id));
            }
        } else {
            self.runtime.emit(format!("Item {} not found", item_id));
        }
    }

    /// Handle battle started message
    async fn handle_battle_started(&mut self, battle_id: String, battle_chain: ChainId, opponent: Owner) {
        self.state.active_battles.push(battle_chain);

        self.runtime.emit(format!(
            "Battle {} started against opponent {} on chain {}",
            battle_id, opponent, battle_chain
        ));
    }

    /// Handle battle result message
    async fn handle_battle_result(
        &mut self,
        battle_id: String,
        winner: Owner,
        xp_earned: u64,
        currency_won: Currency,
        amount_won: u64,
    ) {
        let did_win = winner == self.state.owner;

        // Credit winnings
        if amount_won > 0 {
            let _ = self.state.add_currency(currency_won.clone(), amount_won);

            let earned = self.state.total_earned.entry(currency_won.clone()).or_insert(0);
            *earned += amount_won;
        }

        self.runtime.emit(format!(
            "Battle {} ended. Winner: {}. XP earned: {}. Won: {} {:?}",
            battle_id,
            if did_win { "You" } else { "Opponent" },
            xp_earned,
            amount_won,
            currency_won
        ));

        // TODO: Update character with battle results
    }

    /// Handle currency transfer from another chain
    async fn handle_transfer_currency(&mut self, currency: Currency, amount: u64, from_chain: ChainId) {
        let _ = self.state.add_currency(currency.clone(), amount);

        self.runtime.emit(format!(
            "Received {} {:?} from chain {}",
            amount, currency, from_chain
        ));
    }

    /// Handle character registered confirmation
    async fn handle_character_registered(&mut self, nft_id: String, registry_chain: ChainId) {
        self.runtime.emit(format!(
            "Character {} registered in global registry on chain {}",
            nft_id, registry_chain
        ));
    }
}

/// Player Chain Service (GraphQL queries)
pub struct PlayerChainService {
    state: PlayerChainState,
}

impl Service for PlayerChainService {
    type Parameters = ();

    async fn load(runtime: ServiceRuntime<Self>) -> Self {
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state),
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root
struct QueryRoot<'a> {
    state: &'a PlayerChainState,
}

impl<'a> QueryRoot<'a> {
    fn new(state: &'a PlayerChainState) -> Self {
        Self { state }
    }
}

#[async_graphql::Object]
impl<'a> QueryRoot<'a> {
    /// Get all characters
    async fn characters(&self) -> Vec<CharacterNFT> {
        self.state.characters.clone()
    }

    /// Get character by NFT ID
    async fn character(&self, nft_id: String) -> Option<CharacterNFT> {
        self.state.get_character(&nft_id).cloned()
    }

    /// Get currency balances
    async fn balances(&self) -> Vec<CurrencyBalance> {
        self.state
            .currencies
            .iter()
            .map(|(currency, amount)| CurrencyBalance {
                currency: format!("{:?}", currency),
                amount: *amount,
            })
            .collect()
    }

    /// Get player stats
    async fn stats(&self) -> PlayerStats {
        PlayerStats {
            total_battles: self.state.total_battles,
            wins: self.state.wins,
            losses: self.state.losses,
            win_rate: self.state.win_rate(),
        }
    }

    /// Get inventory items
    async fn inventory(&self) -> Vec<Item> {
        self.state.items.clone()
    }

    /// Get active battles
    async fn active_battles(&self) -> Vec<String> {
        self.state
            .active_battles
            .iter()
            .map(|chain| format!("{}", chain))
            .collect()
    }

    /// Get preferences
    async fn preferences(&self) -> PlayerPreferences {
        PlayerPreferences {
            default_stance: format!("{:?}", self.state.default_stance),
            auto_play: self.state.auto_play,
        }
    }
}

/// GraphQL types
#[derive(async_graphql::SimpleObject)]
struct CurrencyBalance {
    currency: String,
    amount: u64,
}

#[derive(async_graphql::SimpleObject)]
struct PlayerStats {
    total_battles: u64,
    wins: u64,
    losses: u64,
    win_rate: f64,
}

#[derive(async_graphql::SimpleObject)]
struct PlayerPreferences {
    default_stance: String,
    auto_play: bool,
}

/// Empty mutation (operations handled via blockchain operations)
struct EmptyMutation;

#[async_graphql::Object]
impl EmptyMutation {
    async fn placeholder(&self) -> bool {
        false
    }
}

linera_sdk::contract!(PlayerChainContract);
linera_sdk::service!(PlayerChainService);
