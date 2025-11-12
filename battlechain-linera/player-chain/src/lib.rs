use async_graphql::{Request, Response, Schema, EmptySubscription, SimpleObject};
use battlechain_shared_types::*;
use linera_sdk::{
    base::{Amount, ApplicationId, ChainId, Owner, Timestamp, WithContractAbi},
    views::{RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
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

    /// BATTLE token application reference
    pub battle_token_app: Option<ApplicationId>,

    /// Cached BATTLE balance (updated from token app)
    pub battle_balance: Amount,

    /// Locked BATTLE tokens (in battles)
    pub locked_battle: Amount,

    /// Player stats
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub total_earned_battle: Amount,

    /// Active battles (references to battle chains)
    pub active_battles: Vec<ChainId>,

    /// Locked stakes per battle
    pub battle_stakes: HashMap<ChainId, Amount>,

    /// Preferences
    pub default_stance: Stance,
    pub auto_play: bool,

    /// Timestamps
    pub created_at: Timestamp,
    pub last_active: Timestamp,
}

impl PlayerChainState {
    /// Initialize new player chain
    pub fn new(owner: Owner, battle_token_app: Option<ApplicationId>, created_at: Timestamp) -> Self {
        Self {
            owner,
            characters: Vec::new(),
            items: Vec::new(),
            battle_token_app,
            battle_balance: Amount::ZERO,
            locked_battle: Amount::ZERO,
            total_battles: 0,
            wins: 0,
            losses: 0,
            total_earned_battle: Amount::ZERO,
            active_battles: Vec::new(),
            battle_stakes: HashMap::new(),
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

    /// Get available BATTLE balance (total - locked)
    pub fn available_balance(&self) -> Amount {
        self.battle_balance.saturating_sub(self.locked_battle)
    }

    /// Lock BATTLE for battle stake
    pub fn lock_battle(&mut self, battle_chain: ChainId, amount: Amount) -> Result<(), PlayerChainError> {
        if self.available_balance() < amount {
            return Err(PlayerChainError::InsufficientBalance {
                available: self.available_balance(),
                required: amount,
            });
        }

        self.locked_battle = self.locked_battle
            .checked_add(amount)
            .ok_or(PlayerChainError::MathOverflow)?;

        self.battle_stakes.insert(battle_chain, amount);

        Ok(())
    }

    /// Unlock BATTLE from battle
    pub fn unlock_battle(&mut self, battle_chain: &ChainId) -> Result<Amount, PlayerChainError> {
        let amount = self.battle_stakes
            .remove(battle_chain)
            .ok_or(PlayerChainError::BattleNotFound)?;

        self.locked_battle = self.locked_battle
            .checked_sub(amount)
            .ok_or(PlayerChainError::MathOverflow)?;

        Ok(amount)
    }

    /// Add BATTLE to balance (from winnings or transfers)
    pub fn credit_battle(&mut self, amount: Amount) -> Result<(), PlayerChainError> {
        self.battle_balance = self.battle_balance
            .checked_add(amount)
            .ok_or(PlayerChainError::MathOverflow)?;

        Ok(())
    }

    /// Deduct BATTLE from balance (for transfers or stakes)
    pub fn debit_battle(&mut self, amount: Amount) -> Result<(), PlayerChainError> {
        if self.available_balance() < amount {
            return Err(PlayerChainError::InsufficientBalance {
                available: self.available_balance(),
                required: amount,
            });
        }

        self.battle_balance = self.battle_balance
            .checked_sub(amount)
            .ok_or(PlayerChainError::MathOverflow)?;

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
    /// Initialize with BATTLE token app
    Initialize {
        battle_token_app: ApplicationId,
    },

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

    /// Transfer BATTLE tokens to another player
    TransferBattle {
        to: Owner,
        amount: Amount,
    },

    /// Lock BATTLE for battle stake
    LockStakeForBattle {
        battle_chain: ChainId,
        amount: Amount,
    },

    /// Unlock stake after battle cancelled
    UnlockStake {
        battle_chain: ChainId,
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

    /// Refresh BATTLE balance from token app
    RefreshBattleBalance,

    /// Register player chain with matchmaking
    RegisterWithMatchmaking {
        matchmaking_chain: ChainId,
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
        battle_chain: ChainId,
        winner: Owner,
        xp_earned: u64,
        amount_won: Amount,
    },

    /// BATTLE token credit (from token app or battle winnings)
    CreditBattle {
        amount: Amount,
        from_chain: ChainId,
        reason: String,
    },

    /// Character registered in global registry
    CharacterRegistered {
        nft_id: String,
        registry_chain: ChainId,
    },

    /// Balance update from BATTLE token app
    BalanceUpdate {
        new_balance: Amount,
    },

    /// Request from Matchmaking to lock stake for battle
    LockStakeRequest {
        match_id: u64,
        amount: Amount,
        opponent: Owner,
        battle_chain: ChainId,
        matchmaking_chain: ChainId,
    },

    /// Battle is ready to start (both stakes confirmed)
    BattleReady {
        match_id: u64,
        battle_chain: ChainId,
        opponent: Owner,
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

    #[error("Insufficient balance: have {available}, need {required}")]
    InsufficientBalance {
        available: Amount,
        required: Amount,
    },

    #[error("Math overflow")]
    MathOverflow,

    #[error("Unauthorized operation")]
    Unauthorized,

    #[error("Invalid NFT")]
    InvalidNFT,

    #[error("Item not found: {0}")]
    ItemNotFound(String),

    #[error("Battle not found")]
    BattleNotFound,

    #[error("BATTLE token app not initialized")]
    TokenAppNotInitialized,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::views::ViewError> for PlayerChainError {
    fn from(err: linera_sdk::views::views::ViewError) -> Self {
        PlayerChainError::ViewError(format!("{:?}", err))
    }
}

/// Player Chain Contract
pub struct PlayerChainContract {
    state: PlayerChainState,
    runtime: ContractRuntime<Self>,
}

impl Contract for PlayerChainContract {
    type Message = Message;
    type Parameters = Option<ApplicationId>; // Optional BATTLE token app
    type InstantiationArgument = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let battle_token_app = self.runtime.parameters();
        let owner = self.runtime.chain_ownership().owner
            .expect("Chain must have owner");
        let now = self.runtime.system_time();

        self.state = PlayerChainState::new(owner, battle_token_app, now);

        if let Some(app_id) = battle_token_app {
            self.runtime.emit(format!(
                "Player chain initialized for owner: {} with BATTLE token app: {}",
                owner, app_id
            ));
        } else {
            self.runtime.emit(format!(
                "Player chain initialized for owner: {} (BATTLE token app will be set later)",
                owner
            ));
        }
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let now = self.runtime.system_time();
        self.state.last_active = now;

        match operation {
            Operation::Initialize { battle_token_app } => {
                self.initialize_token_app(battle_token_app).await
            }
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
            Operation::TransferBattle { to, amount } => {
                self.transfer_battle(to, amount).await
            }
            Operation::LockStakeForBattle { battle_chain, amount } => {
                self.lock_stake(battle_chain, amount).await
            }
            Operation::UnlockStake { battle_chain } => {
                self.unlock_stake(battle_chain).await
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
            Operation::RefreshBattleBalance => {
                self.refresh_battle_balance().await
            }
            Operation::RegisterWithMatchmaking { matchmaking_chain } => {
                self.register_with_matchmaking(matchmaking_chain).await
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
                battle_chain,
                winner,
                xp_earned,
                amount_won,
            } => {
                self.handle_battle_result(battle_id, battle_chain, winner, xp_earned, amount_won)
                    .await;
            }
            Message::CreditBattle {
                amount,
                from_chain,
                reason,
            } => {
                self.handle_credit_battle(amount, from_chain, reason)
                    .await;
            }
            Message::CharacterRegistered {
                nft_id,
                registry_chain,
            } => {
                self.handle_character_registered(nft_id, registry_chain)
                    .await;
            }
            Message::BalanceUpdate { new_balance } => {
                self.handle_balance_update(new_balance).await;
            }
            Message::LockStakeRequest {
                match_id,
                amount,
                opponent,
                battle_chain,
                matchmaking_chain,
            } => {
                self.handle_lock_stake_request(match_id, amount, opponent, battle_chain, matchmaking_chain)
                    .await;
            }
            Message::BattleReady {
                match_id,
                battle_chain,
                opponent,
            } => {
                self.handle_battle_ready(match_id, battle_chain, opponent)
                    .await;
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl PlayerChainContract {
    /// Initialize BATTLE token app reference
    async fn initialize_token_app(&mut self, battle_token_app: ApplicationId) -> () {
        self.state.battle_token_app = Some(battle_token_app);

        self.runtime.emit(format!(
            "BATTLE token app set to: {}",
            battle_token_app
        ));

        // Refresh balance from token app
        self.refresh_battle_balance().await;
    }

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

    /// Transfer BATTLE tokens to another player
    async fn transfer_battle(&mut self, to: Owner, amount: Amount) -> () {
        let token_app = match self.state.battle_token_app {
            Some(app) => app,
            None => {
                self.runtime.emit("BATTLE token app not initialized".to_string());
                return;
            }
        };

        match self.state.debit_battle(amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Transfer {} BATTLE to {} (via token app: {})",
                    amount, to, token_app
                ));

                // TODO: Send cross-application message to BATTLE token app
                // self.runtime.call_application(
                //     token_app,
                //     BattleTokenOperation::Transfer { to, amount }
                // ).await;
            }
            Err(e) => {
                self.runtime.emit(format!("Transfer failed: {}", e));
            }
        }
    }

    /// Lock BATTLE for battle stake
    async fn lock_stake(&mut self, battle_chain: ChainId, amount: Amount) -> () {
        match self.state.lock_battle(battle_chain, amount) {
            Ok(_) => {
                self.runtime.emit(format!(
                    "Locked {} BATTLE for battle on chain {}",
                    amount, battle_chain
                ));
                // TODO: Send transfer message to battle chain via token app
            }
            Err(e) => {
                self.runtime.emit(format!("Lock stake failed: {}", e));
            }
        }
    }

    /// Unlock stake after battle cancelled
    async fn unlock_stake(&mut self, battle_chain: ChainId) -> () {
        match self.state.unlock_battle(&battle_chain) {
            Ok(amount) => {
                self.runtime.emit(format!(
                    "Unlocked {} BATTLE from battle {}",
                    amount, battle_chain
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

    /// Refresh BATTLE balance from token app
    async fn refresh_battle_balance(&mut self) -> () {
        if self.state.battle_token_app.is_none() {
            self.runtime.emit("BATTLE token app not initialized".to_string());
            return;
        }

        // TODO: Query BATTLE token app for current balance
        // let balance = self.runtime.query_application(
        //     self.state.battle_token_app.unwrap(),
        //     format!("query {{ balanceOf(account: \"{}\") }}", self.state.owner)
        // ).await;

        // self.state.battle_balance = balance;

        self.runtime.emit(format!(
            "Balance refreshed: {} BATTLE (available: {})",
            self.state.battle_balance,
            self.state.available_balance()
        ));
    }

    /// Register this player chain with the Matchmaking chain
    async fn register_with_matchmaking(&mut self, matchmaking_chain: ChainId) -> () {
        self.runtime.emit(format!(
            "Registering player chain {} with Matchmaking at {}",
            self.runtime.chain_id(),
            matchmaking_chain
        ));

        // TODO: Send RegisterPlayerChain operation to Matchmaking
        // This requires cross-application operation call:
        // self.runtime.call_application(
        //     matchmaking_chain,
        //     MatchmakingOperation::RegisterPlayerChain {
        //         player_chain: self.runtime.chain_id(),
        //     }
        // ).await;
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
        battle_chain: ChainId,
        winner: Owner,
        xp_earned: u64,
        amount_won: Amount,
    ) {
        let did_win = winner == self.state.owner;

        // Unlock stake and credit winnings
        if let Ok(stake) = self.state.unlock_battle(&battle_chain) {
            if did_win && amount_won > Amount::ZERO {
                let _ = self.state.credit_battle(amount_won);
                let _ = self.state.credit_battle(stake);

                let total_earnings = &mut self.state.total_earned_battle;
                *total_earnings = total_earnings
                    .checked_add(amount_won)
                    .unwrap_or(*total_earnings);
            } else if !did_win {
                // Lost - stake is gone
            }
        }

        // Remove from active battles
        self.state.active_battles.retain(|c| c != &battle_chain);

        self.runtime.emit(format!(
            "Battle {} ended. Winner: {}. XP earned: {}. Won: {} BATTLE",
            battle_id,
            if did_win { "You" } else { "Opponent" },
            xp_earned,
            amount_won
        ));
    }

    /// Handle BATTLE token credit
    async fn handle_credit_battle(&mut self, amount: Amount, from_chain: ChainId, reason: String) {
        let _ = self.state.credit_battle(amount);

        self.runtime.emit(format!(
            "Received {} BATTLE from chain {} (reason: {})",
            amount, from_chain, reason
        ));
    }

    /// Handle character registered confirmation
    async fn handle_character_registered(&mut self, nft_id: String, registry_chain: ChainId) {
        self.runtime.emit(format!(
            "Character {} registered in global registry on chain {}",
            nft_id, registry_chain
        ));
    }

    /// Handle balance update from token app
    async fn handle_balance_update(&mut self, new_balance: Amount) {
        self.state.battle_balance = new_balance;

        self.runtime.emit(format!(
            "Balance updated: {} BATTLE (available: {})",
            new_balance,
            self.state.available_balance()
        ));
    }

    /// Handle lock stake request from Matchmaking
    async fn handle_lock_stake_request(
        &mut self,
        match_id: u64,
        amount: Amount,
        opponent: Owner,
        battle_chain: ChainId,
        matchmaking_chain: ChainId,
    ) {
        // Try to lock the stake
        match self.state.lock_battle(battle_chain, amount) {
            Ok(()) => {
                self.runtime.emit(format!(
                    "Locked {} BATTLE for match {} against {}",
                    amount, match_id, opponent
                ));

                // TODO: Send ConfirmStake message back to Matchmaking
                // This requires cross-application messaging:
                // self.runtime.send_message(
                //     matchmaking_chain,
                //     MatchmakingMessage::ConfirmStake {
                //         match_id,
                //         player: self.state.owner,
                //     }
                // );
            }
            Err(e) => {
                self.runtime.emit(format!(
                    "Failed to lock stake for match {}: {}",
                    match_id, e
                ));

                // TODO: Send stake lock failure message to Matchmaking
                // so it can cancel the match
            }
        }
    }

    /// Handle battle ready notification
    async fn handle_battle_ready(
        &mut self,
        match_id: u64,
        battle_chain: ChainId,
        opponent: Owner,
    ) {
        self.state.active_battles.push(battle_chain);

        self.runtime.emit(format!(
            "Battle ready! Match {} on chain {} against {}. Submit your turns to begin combat.",
            match_id, battle_chain, opponent
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

    /// Get BATTLE token balance
    async fn battle_balance(&self) -> BattleBalance {
        BattleBalance {
            total: self.state.battle_balance.to_string(),
            locked: self.state.locked_battle.to_string(),
            available: self.state.available_balance().to_string(),
        }
    }

    /// Get player stats
    async fn stats(&self) -> PlayerStats {
        PlayerStats {
            total_battles: self.state.total_battles,
            wins: self.state.wins,
            losses: self.state.losses,
            win_rate: self.state.win_rate(),
            total_earned: self.state.total_earned_battle.to_string(),
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

    /// Get locked stakes per battle
    async fn locked_stakes(&self) -> Vec<LockedStake> {
        self.state
            .battle_stakes
            .iter()
            .map(|(chain, amount)| LockedStake {
                battle_chain: format!("{}", chain),
                amount: amount.to_string(),
            })
            .collect()
    }

    /// Get preferences
    async fn preferences(&self) -> PlayerPreferences {
        PlayerPreferences {
            default_stance: format!("{:?}", self.state.default_stance),
            auto_play: self.state.auto_play,
        }
    }

    /// Get BATTLE token app ID
    async fn battle_token_app(&self) -> Option<String> {
        self.state.battle_token_app.map(|app| format!("{}", app))
    }
}

/// GraphQL types
#[derive(SimpleObject)]
struct BattleBalance {
    total: String,
    locked: String,
    available: String,
}

#[derive(SimpleObject)]
struct PlayerStats {
    total_battles: u64,
    wins: u64,
    losses: u64,
    win_rate: f64,
    total_earned: String,
}

#[derive(SimpleObject)]
struct LockedStake {
    battle_chain: String,
    amount: String,
}

#[derive(SimpleObject)]
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
