use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use battlechain_shared_types::*;
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Player Chain Application ABI
pub struct PlayerChainAbi;

impl ContractAbi for PlayerChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for PlayerChainAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Player Chain State - manages player inventory and stats
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PlayerChainState {
    /// All characters owned by this player
    pub characters: RegisterView<Vec<CharacterNFT>>,

    /// BATTLE token application reference
    pub battle_token_app: RegisterView<Option<ApplicationId>>,

    /// Cached BATTLE balance
    pub battle_balance: RegisterView<Amount>,

    /// Locked BATTLE tokens (in battles)
    pub locked_battle: RegisterView<Amount>,

    /// Player stats
    pub total_battles: RegisterView<u64>,
    pub wins: RegisterView<u64>,
    pub losses: RegisterView<u64>,

    /// Active battles
    pub active_battles: RegisterView<Vec<ChainId>>,

    /// Locked stakes per battle
    pub battle_stakes: MapView<ChainId, Amount>,

    /// Character in each battle (battle_chain -> character_nft_id)
    pub battle_characters: MapView<ChainId, String>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
    pub last_active: RegisterView<Timestamp>,
}

impl PlayerChainState {
    /// Get available BATTLE balance
    pub fn available_balance(&self) -> Amount {
        self.battle_balance.get().saturating_sub(*self.locked_battle.get())
    }

    /// Lock BATTLE for battle stake
    pub fn lock_battle(&mut self, battle_chain: ChainId, amount: Amount) -> Result<(), PlayerChainError> {
        if self.available_balance() < amount {
            return Err(PlayerChainError::InsufficientBalance {
                available: self.available_balance(),
                required: amount,
            });
        }

        let new_locked = self.locked_battle.get()
            .try_add(amount)
            .map_err(|_| PlayerChainError::MathOverflow)?;
        self.locked_battle.set(new_locked);

        self.battle_stakes.insert(&battle_chain, amount)
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?;

        Ok(())
    }

    /// Unlock BATTLE from battle
    pub async fn unlock_battle(&mut self, battle_chain: &ChainId) -> Result<Amount, PlayerChainError> {
        // Get the staked amount first
        let amount = self.battle_stakes
            .get(battle_chain)
            .await
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?
            .ok_or(PlayerChainError::BattleNotFound)?;

        // Remove the stake
        self.battle_stakes
            .remove(battle_chain)
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?;

        let new_locked = self.locked_battle.get()
            .try_sub(amount)
            .map_err(|_| PlayerChainError::MathOverflow)?;
        self.locked_battle.set(new_locked);

        Ok(amount)
    }

    /// Record battle result
    pub fn record_battle_result(&mut self, won: bool) {
        self.total_battles.set(*self.total_battles.get() + 1);
        if won {
            self.wins.set(*self.wins.get() + 1);
        } else {
            self.losses.set(*self.losses.get() + 1);
        }
    }
}

/// Operations on Player Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Initialize BATTLE token app reference
    Initialize { battle_token_app: ApplicationId },

    /// Create new character
    CreateCharacter { nft_id: String, class: CharacterClass },

    /// Join a battle
    JoinBattle { battle_chain: ChainId, character_nft: String, stake: Amount },

    /// Update stats after battle
    UpdateAfterBattle { battle_chain: ChainId, won: bool, reward: Amount },
}

/// Cross-chain messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle invitation
    BattleInvite {
        battle_chain: ChainId,
        stake_required: Amount,
    },

    /// Battle result notification (from battle chain)
    BattleResult {
        winner: Owner,
        loser: Owner,
        winner_payout: Amount,
        rounds_played: u8,
    },

    /// Matchmaking request to lock stake
    LockStakeRequest {
        matchmaking_chain: ChainId,
        battle_chain: ChainId,
        stake_amount: Amount,
    },
}

/// Player Chain Errors
#[derive(Debug, Error)]
pub enum PlayerChainError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Amount, required: Amount },

    #[error("Battle not found")]
    BattleNotFound,

    #[error("Character not found: {0}")]
    CharacterNotFound(String),

    #[error("Math overflow")]
    MathOverflow,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for PlayerChainError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        PlayerChainError::ViewError(format!("{:?}", err))
    }
}

/// Player Chain Contract
pub struct PlayerChainContract {
    state: PlayerChainState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(PlayerChainContract);

impl WithContractAbi for PlayerChainContract {
    type Abi = PlayerChainAbi;
}

impl Contract for PlayerChainContract {
    type Message = Message;
    type Parameters = Option<ApplicationId>;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let battle_token_app = self.runtime.application_parameters();
        let chain_ownership = self.runtime.chain_ownership();
        let owner = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("Chain must have owner")
            .clone();
        let now = self.runtime.system_time();

        // Initialize state (owner is tracked via chain ownership)
        self.state.characters.set(Vec::new());
        self.state.battle_token_app.set(battle_token_app);
        self.state.battle_balance.set(Amount::ZERO);
        self.state.locked_battle.set(Amount::ZERO);
        self.state.total_battles.set(0);
        self.state.wins.set(0);
        self.state.losses.set(0);
        self.state.active_battles.set(Vec::new());
        self.state.created_at.set(now);
        self.state.last_active.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let now = self.runtime.system_time();
        self.state.last_active.set(now);

        match operation {
            Operation::Initialize { battle_token_app } => {
                self.state.battle_token_app.set(Some(battle_token_app));
            }

            Operation::CreateCharacter { nft_id, class } => {
                let mut chars = self.state.characters.get().clone();
                let new_char = CharacterNFT::new(nft_id, class, now);

                // Validate character state after creation
                if let Err(e) = new_char.validate() {
                    log::error!("Character validation failed after creation: {}", e);
                    panic!("Invalid character state: {}", e);
                }

                chars.push(new_char);
                self.state.characters.set(chars);
            }

            Operation::JoinBattle { battle_chain, character_nft, stake } => {
                // Store which character is in this battle
                self.state.battle_characters.insert(&battle_chain, character_nft.clone())
                    .expect("Failed to store character for battle");

                // Mark character as in battle
                let mut chars = self.state.characters.get().clone();
                if let Some(character) = chars.iter_mut().find(|c| c.nft_id == character_nft) {
                    character.in_battle = true;
                }
                self.state.characters.set(chars);

                // Lock stake
                let _ = self.state.lock_battle(battle_chain, stake);

                // Add to active battles
                let mut active = self.state.active_battles.get().clone();
                active.push(battle_chain);
                self.state.active_battles.set(active);
            }

            Operation::UpdateAfterBattle { battle_chain, won, reward } => {
                // Unlock stake
                let _ = self.state.unlock_battle(&battle_chain).await;

                // Remove from active battles
                let mut active = self.state.active_battles.get().clone();
                active.retain(|c| c != &battle_chain);
                self.state.active_battles.set(active);

                // Record result
                self.state.record_battle_result(won);

                // Add reward if won
                if won && reward > Amount::ZERO {
                    let new_balance = self.state.battle_balance.get()
                        .try_add(reward)
                        .unwrap_or(*self.state.battle_balance.get());
                    self.state.battle_balance.set(new_balance);
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleInvite { battle_chain: _, stake_required: _ } => {
                // Handle battle invite - could auto-join if auto_play enabled
                // TODO: Implement auto-join logic based on player preferences
            }

            Message::BattleResult {
                winner,
                loser: _,
                winner_payout,
                rounds_played,
            } => {
                // Determine if this player won or lost
                let player_owner = self.runtime.authenticated_signer();
                let won = player_owner.map(|o| o == winner).unwrap_or(false);

                // Find battle chain from message origin
                let battle_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        // Fallback: try to find in active battles
                        if let Some(chain) = self.state.active_battles.get().first().cloned() {
                            chain
                        } else {
                            // No battle chain found - this shouldn't happen in normal operation
                            return;
                        }
                    }
                };

                // === CHARACTER PROGRESSION ===
                // Get character ID from battle
                let character_id = match self.state.battle_characters.get(&battle_chain).await {
                    Ok(Some(id)) => id,
                    _ => {
                        // No character tracked - skip progression but continue with other updates
                        String::new()
                    }
                };

                // Update character progression if we have a character ID
                if !character_id.is_empty() {
                    let mut chars = self.state.characters.get().clone();
                    let mut should_remove_character = false;

                    // Update character in a separate scope to avoid borrow conflicts
                    {
                        let character_opt = chars.iter_mut().find(|c| c.nft_id == character_id);

                        if let Some(character) = character_opt {
                            // Mark character as no longer in battle
                            character.in_battle = false;

                            if won {
                                // Award XP based on rounds played
                                let xp_reward = 100 + (rounds_played as u64 * 10);
                                character.xp += xp_reward;

                                // Check for level up
                                let xp_needed = 100 * (character.level as u64);
                                if character.xp >= xp_needed {
                                    character.level += 1;
                                    character.xp = character.xp.saturating_sub(xp_needed);

                                    // Update stats for new level
                                    character.hp_max += 10;
                                    character.current_hp = character.hp_max; // Full heal on level up
                                    character.min_damage += 1;
                                    character.max_damage += 2;

                                    log::info!(
                                        "Character {} leveled up to level {}!",
                                        character.nft_id,
                                        character.level
                                    );
                                }
                            } else {
                                // Lose a life (permadeath mechanic)
                                character.lives = character.lives.saturating_sub(1);

                                if character.lives == 0 {
                                    // Character is permanently dead - will be removed after validation
                                    should_remove_character = true;
                                    log::warn!(
                                        "Character {} has died (permadeath)!",
                                        character.nft_id
                                    );
                                } else {
                                    log::info!(
                                        "Character {} has {} lives remaining",
                                        character.nft_id,
                                        character.lives
                                    );
                                }
                            }

                            // Validate character state after modifications
                            if !should_remove_character {
                                if let Err(e) = character.validate() {
                                    log::error!("Character validation failed after battle update: {}", e);
                                    log::error!("Character: {:#?}", character);
                                    panic!("Invalid character state after battle: {}", e);
                                }
                            }
                        }
                    } // character reference dropped here

                    // Now safe to call retain() since character reference is dropped
                    if should_remove_character {
                        chars.retain(|c| c.nft_id != character_id);
                    }

                    self.state.characters.set(chars);

                    // Remove character from battle tracking
                    let _ = self.state.battle_characters.remove(&battle_chain);
                }
                // === END CHARACTER PROGRESSION ===

                // Unlock stake
                let _ = self.state.unlock_battle(&battle_chain).await;

                // Record result
                self.state.record_battle_result(won);

                // Remove from active battles
                let mut active = self.state.active_battles.get().clone();
                active.retain(|c| c != &battle_chain);
                self.state.active_battles.set(active);

                // Add payout if won
                if won && winner_payout > Amount::ZERO {
                    let new_balance = self.state.battle_balance.get()
                        .try_add(winner_payout)
                        .unwrap_or(*self.state.battle_balance.get());
                    self.state.battle_balance.set(new_balance);
                }
            }

            Message::LockStakeRequest {
                matchmaking_chain: _,
                battle_chain,
                stake_amount,
            } => {
                // Lock stake for upcoming battle
                if let Err(_e) = self.state.lock_battle(battle_chain, stake_amount) {
                    // Failed to lock - could send rejection message back
                    return;
                }

                // Add to active battles
                let mut active = self.state.active_battles.get().clone();
                if !active.contains(&battle_chain) {
                    active.push(battle_chain);
                    self.state.active_battles.set(active);
                }

                // TODO: Send confirmation message back to matchmaking chain
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Player Chain Service
pub struct PlayerChainService {
    state: PlayerChainState,
}

impl WithServiceAbi for PlayerChainService {
    type Abi = PlayerChainAbi;
}

impl Service for PlayerChainService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state).await,
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root
#[derive(Clone)]
struct QueryRoot {
    character_count: usize,
    battle_balance: Amount,
    locked_battle: Amount,
    total_battles: u64,
    wins: u64,
    losses: u64,
    active_battle_count: usize,
}

impl QueryRoot {
    async fn new(state: &PlayerChainState) -> Self {
        Self {
            character_count: state.characters.get().len(),
            battle_balance: *state.battle_balance.get(),
            locked_battle: *state.locked_battle.get(),
            total_battles: *state.total_battles.get(),
            wins: *state.wins.get(),
            losses: *state.losses.get(),
            active_battle_count: state.active_battles.get().len(),
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get number of characters owned
    async fn character_count(&self) -> i32 {
        self.character_count as i32
    }

    /// Get BATTLE balance
    async fn battle_balance(&self) -> String {
        self.battle_balance.to_string()
    }

    /// Get available balance
    async fn available_balance(&self) -> String {
        self.battle_balance.saturating_sub(self.locked_battle).to_string()
    }

    /// Get locked balance
    async fn locked_balance(&self) -> String {
        self.locked_battle.to_string()
    }

    /// Get player stats
    async fn stats(&self) -> PlayerStats {
        let win_rate = if self.total_battles == 0 {
            0.0
        } else {
            (self.wins as f64) / (self.total_battles as f64)
        };

        PlayerStats {
            total_battles: self.total_battles,
            wins: self.wins,
            losses: self.losses,
            win_rate,
        }
    }

    /// Get active battle count
    async fn active_battle_count(&self) -> i32 {
        self.active_battle_count as i32
    }
}

#[derive(SimpleObject)]
struct PlayerStats {
    total_battles: u64,
    wins: u64,
    losses: u64,
    win_rate: f64,
}
