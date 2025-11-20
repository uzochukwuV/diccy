use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use battlechain_shared_events::{BattleEvent, CombatStats};
use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// BattleEvent is now imported from battlechain-shared-events

/// Registry Chain Application ABI
pub struct RegistryAbi;

impl ContractAbi for RegistryAbi {
    type Operation = Operation;
    type Response = Result<(), RegistryError>;
}

impl ServiceAbi for RegistryAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Character statistics in the global registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterStats {
    pub character_id: String,
    pub nft_id: String,
    pub owner: Owner,
    pub owner_chain: ChainId,
    pub class: CharacterClass,
    pub level: u16,

    // Battle statistics
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64, // Calculated as wins / total_battles
    pub current_streak: i32, // Positive for wins, negative for losses
    pub best_win_streak: u32,

    // Combat statistics
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub highest_crit: u64,
    pub total_crits: u64,
    pub total_dodges: u64,

    // Earnings
    pub total_earnings: Amount,
    pub total_wagered: Amount,

    // ELO rating (starts at 1500)
    pub elo_rating: u64,

    // Status
    pub is_alive: bool,
    pub lives_remaining: u8,

    // Timestamps
    pub registered_at: Timestamp,
    pub last_battle_at: Option<Timestamp>,
}

impl CharacterStats {
    pub fn new(
        character_id: String,
        nft_id: String,
        owner: Owner,
        owner_chain: ChainId,
        class: CharacterClass,
        level: u16,
        registered_at: Timestamp,
    ) -> Self {
        Self {
            character_id,
            nft_id,
            owner,
            owner_chain,
            class,
            level,
            total_battles: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            current_streak: 0,
            best_win_streak: 0,
            total_damage_dealt: 0,
            total_damage_taken: 0,
            highest_crit: 0,
            total_crits: 0,
            total_dodges: 0,
            total_earnings: Amount::ZERO,
            total_wagered: Amount::ZERO,
            elo_rating: 1500, // Starting ELO
            is_alive: true,
            lives_remaining: 3,
            registered_at,
            last_battle_at: None,
        }
    }

    /// Update stats after a battle
    pub fn update_after_battle(
        &mut self,
        won: bool,
        damage_dealt: u64,
        damage_taken: u64,
        crits: u64,
        dodges: u64,
        highest_crit: u64,
        earnings: Amount,
        stake: Amount,
        opponent_elo: u64,
        timestamp: Timestamp,
    ) {
        self.total_battles += 1;

        if won {
            self.wins += 1;
            self.current_streak = if self.current_streak >= 0 {
                self.current_streak + 1
            } else {
                1
            };
            if self.current_streak > self.best_win_streak as i32 {
                self.best_win_streak = self.current_streak as u32;
            }
        } else {
            self.losses += 1;
            self.current_streak = if self.current_streak <= 0 {
                self.current_streak - 1
            } else {
                -1
            };
        }

        self.win_rate = (self.wins as f64) / (self.total_battles as f64);

        self.total_damage_dealt += damage_dealt;
        self.total_damage_taken += damage_taken;
        self.total_crits += crits;
        self.total_dodges += dodges;

        if highest_crit > self.highest_crit {
            self.highest_crit = highest_crit;
        }

        self.total_earnings = self.total_earnings.saturating_add(earnings);
        self.total_wagered = self.total_wagered.saturating_add(stake);

        // Update ELO rating
        self.elo_rating = calculate_new_elo(self.elo_rating, opponent_elo, won);

        self.last_battle_at = Some(timestamp);
    }
}

/// Calculate new ELO rating using standard ELO formula
fn calculate_new_elo(player_elo: u64, opponent_elo: u64, won: bool) -> u64 {
    const K_FACTOR: f64 = 32.0; // Standard K-factor

    // Expected score
    let expected = 1.0 / (1.0 + 10f64.powf((opponent_elo as f64 - player_elo as f64) / 400.0));

    // Actual score
    let actual = if won { 1.0 } else { 0.0 };

    // New rating
    let new_rating = player_elo as f64 + K_FACTOR * (actual - expected);

    new_rating.max(100.0) as u64 // Minimum ELO of 100
}

/// Battle record for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleRecord {
    pub battle_id: u64,
    pub battle_chain: ChainId,
    pub player1_id: String,
    pub player2_id: String,
    pub winner_id: String,
    pub stake: Amount,
    pub rounds_played: u8,
    pub timestamp: Timestamp,
}

/// Registry State - tracks global game statistics
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct RegistryState {
    /// Character statistics indexed by character_id
    pub characters: MapView<String, CharacterStats>,

    /// Character ID by owner chain (for quick lookup)
    pub owner_to_character: MapView<ChainId, String>,

    /// Battle records indexed by battle_id
    pub battles: MapView<u64, BattleRecord>,

    /// Battle chain to battle ID mapping
    pub battle_chain_to_id: MapView<ChainId, u64>,

    /// Next battle ID
    pub next_battle_id: RegisterView<u64>,

    /// Total characters registered
    pub total_characters: RegisterView<u64>,

    /// Total battles recorded
    pub total_battles: RegisterView<u64>,

    /// Total volume wagered
    pub total_volume: RegisterView<Amount>,

    /// Top characters by ELO (limited to top 100)
    pub top_elo: RegisterView<Vec<String>>, // Character IDs sorted by ELO

    /// SECURITY: Track known battle chains (for message authentication)
    pub known_battle_chains: MapView<ChainId, bool>,

    /// SECURITY: Admin owner (for pause functionality)
    pub admin: RegisterView<Option<Owner>>,

    /// SECURITY: Paused state
    pub paused: RegisterView<bool>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}

impl RegistryState {
    /// Register a new character
    pub fn register_character(&mut self, stats: CharacterStats) -> Result<(), RegistryError> {
        let character_id = stats.character_id.clone();
        let owner_chain = stats.owner_chain;

        self.characters.insert(&character_id, stats)?;
        self.owner_to_character.insert(&owner_chain, character_id)?;

        let total = *self.total_characters.get();
        self.total_characters.set(total + 1);

        Ok(())
    }

    /// Update leaderboard after character stats change
    /// Sorts by ELO rating (descending) and keeps top 100
    pub async fn update_leaderboard(&mut self, character_id: String) -> Result<(), RegistryError> {
        let mut top = self.top_elo.get().clone();

        // Remove if already exists
        top.retain(|id| id != &character_id);

        // Add to list
        top.push(character_id);

        // Fetch ELO ratings for all characters in the list
        let mut character_elos: Vec<(String, u64)> = Vec::new();
        for id in top.iter() {
            if let Some(stats) = self.characters.get(id).await? {
                character_elos.push((id.clone(), stats.elo_rating));
            }
        }

        // Sort by ELO rating (descending)
        character_elos.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep only top 100 and extract character IDs
        let sorted_ids: Vec<String> = character_elos
            .into_iter()
            .take(100)
            .map(|(id, _)| id)
            .collect();

        self.top_elo.set(sorted_ids);
        Ok(())
    }
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Register a new character in the global registry
    RegisterCharacter {
        character_id: String,
        nft_id: String,
        owner: Owner,
        owner_chain: ChainId,
        class: CharacterClass,
        level: u16,
    },

    /// Update character statistics after a battle
    UpdateCharacterStats {
        character_id: String,
        won: bool,
        damage_dealt: u64,
        damage_taken: u64,
        crits: u64,
        dodges: u64,
        highest_crit: u64,
        earnings: Amount,
        stake: Amount,
        opponent_elo: u64,
    },

    /// Record a battle in the global history
    RecordBattle {
        battle_chain: ChainId,
        player1_id: String,
        player2_id: String,
        winner_id: String,
        stake: Amount,
        rounds_played: u8,
    },

    /// Update character level
    UpdateCharacterLevel {
        character_id: String,
        new_level: u16,
    },

    /// Mark character as defeated (no lives remaining)
    MarkCharacterDefeated {
        character_id: String,
    },

    /// Subscribe to battle events from a battle chain
    SubscribeToBattleEvents {
        battle_chain_id: ChainId,
        battle_app_id: linera_sdk::linera_base_types::ApplicationId,
    },

    /// SECURITY: Pause contract (admin only)
    Pause,

    /// SECURITY: Unpause contract (admin only)
    Unpause,

    /// SECURITY: Transfer admin rights (admin only)
    TransferAdmin { new_admin: Owner },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle completed - update stats
    BattleCompleted {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
        winner_chain: ChainId,
        stake: Amount,
        rounds_played: u8,
        // Combat statistics (now using shared struct)
        player1_stats: CombatStats,
        player2_stats: CombatStats,
    },

    /// Character registered
    CharacterRegistered {
        character_id: String,
        owner_chain: ChainId,
    },
}

/// Errors
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum RegistryError {
    #[error("Character not found: {0}")]
    CharacterNotFound(String),

    #[error("Character already registered")]
    CharacterAlreadyRegistered,

    #[error("Battle not found")]
    BattleNotFound,

    #[error("Unauthorized message sender: {0:?}")]
    UnauthorizedSender(ChainId),

    #[error("Contract is paused")]
    ContractPaused,

    #[error("Not authorized: only admin can perform this operation")]
    NotAuthorized,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for RegistryError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        RegistryError::ViewError(format!("{:?}", err))
    }
}

/// Registry Contract
pub struct RegistryContract {
    state: RegistryState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(RegistryContract);

impl WithContractAbi for RegistryContract {
    type Abi = RegistryAbi;
}

impl Contract for RegistryContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = RegistryState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let now = self.runtime.system_time();

        // Get creator as admin
        let chain_ownership = self.runtime.chain_ownership();
        let creator = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("No super owners found")
            .clone();

        self.state.next_battle_id.set(0);
        self.state.total_characters.set(0);
        self.state.total_battles.set(0);
        self.state.total_volume.set(Amount::ZERO);
        self.state.top_elo.set(Vec::new());
        self.state.created_at.set(now);

        // SECURITY: Initialize admin and paused state
        self.state.admin.set(Some(creator));
        self.state.paused.set(false);
        log::info!("Registry initialized with admin: {:?}", creator);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        // SECURITY: Check if contract is paused (skip for admin operations)
        match operation {
            Operation::Pause | Operation::Unpause | Operation::TransferAdmin { .. } => {
                // Admin operations allowed even when paused
            }
            _ => {
                if *self.state.paused.get() {
                    return Err(RegistryError::ContractPaused);
                }
            }
        }

        match operation {
            Operation::RegisterCharacter {
                character_id,
                nft_id,
                owner,
                owner_chain,
                class,
                level,
            } => {
                // Check if already registered
                if self.state.characters.get(&character_id).await?.is_some() {
                    return Err(RegistryError::CharacterAlreadyRegistered);
                }

                let now = self.runtime.system_time();
                let stats = CharacterStats::new(
                    character_id.clone(),
                    nft_id,
                    owner,
                    owner_chain,
                    class,
                    level,
                    now,
                );

                self.state.register_character(stats)?;
                self.state.update_leaderboard(character_id).await?;
            }

            Operation::UpdateCharacterStats {
                character_id,
                won,
                damage_dealt,
                damage_taken,
                crits,
                dodges,
                highest_crit,
                earnings,
                stake,
                opponent_elo,
            } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                let now = self.runtime.system_time();
                stats.update_after_battle(
                    won,
                    damage_dealt,
                    damage_taken,
                    crits,
                    dodges,
                    highest_crit,
                    earnings,
                    stake,
                    opponent_elo,
                    now,
                );

                self.state.characters.insert(&character_id, stats)?;
                self.state.update_leaderboard(character_id).await?;
            }

            Operation::RecordBattle {
                battle_chain,
                player1_id,
                player2_id,
                winner_id,
                stake,
                rounds_played,
            } => {
                let battle_id = *self.state.next_battle_id.get();
                self.state.next_battle_id.set(battle_id + 1);

                let now = self.runtime.system_time();
                let record = BattleRecord {
                    battle_id,
                    battle_chain,
                    player1_id,
                    player2_id,
                    winner_id,
                    stake,
                    rounds_played,
                    timestamp: now,
                };

                self.state.battles.insert(&battle_id, record)?;
                self.state.battle_chain_to_id.insert(&battle_chain, battle_id)?;

                let total = *self.state.total_battles.get();
                self.state.total_battles.set(total + 1);

                let volume = *self.state.total_volume.get();
                self.state.total_volume.set(volume.saturating_add(stake));
            }

            Operation::UpdateCharacterLevel { character_id, new_level } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                stats.level = new_level;

                self.state.characters.insert(&character_id, stats)?;
            }

            Operation::MarkCharacterDefeated { character_id } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                stats.is_alive = false;
                stats.lives_remaining = 0;

                self.state.characters.insert(&character_id, stats)?;
            }

            Operation::SubscribeToBattleEvents { battle_chain_id, battle_app_id } => {
                // Subscribe to battle events from the specified battle chain
                self.runtime.subscribe_to_events(
                    battle_chain_id,
                    battle_app_id,
                    "battle_events".into(),
                );

                // SECURITY: Track this battle chain for message authentication
                self.state.known_battle_chains.insert(&battle_chain_id, true)?;

                log::info!(
                    "Registry subscribed to battle events from chain {:?}, app {:?}",
                    battle_chain_id,
                    battle_app_id
                );
            }

            Operation::Pause => {
                // SECURITY: Only admin can pause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?;
                if &caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.paused.set(true);
                log::warn!("Registry paused by admin: {:?}", admin);
            }

            Operation::Unpause => {
                // SECURITY: Only admin can unpause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?;
                if &caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.paused.set(false);
                log::info!("Registry unpaused by admin: {:?}", admin);
            }

            Operation::TransferAdmin { new_admin } => {
                // SECURITY: Only current admin can transfer admin rights
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?.clone();
                if caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.admin.set(Some(new_admin));
                log::info!("Registry admin transferred from {:?} to {:?}", admin, new_admin);
            }
        }

        Ok(())
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleCompleted {
                battle_chain,
                player1_chain,
                player2_chain,
                winner_chain,
                stake,
                rounds_played,
                player1_stats,
                player2_stats,
            } => {
                // SECURITY: Validate message sender is a known battle chain
                let sender_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        log::error!("BattleCompleted message has no origin chain");
                        return;
                    }
                };

                // Check if this is a known battle chain
                match self.state.known_battle_chains.get(&sender_chain).await {
                    Ok(Some(true)) => {
                        // Valid battle chain, continue processing
                    }
                    _ => {
                        log::error!(
                            "SECURITY: Unauthorized BattleCompleted from unknown chain: {:?}",
                            sender_chain
                        );
                        return; // Reject message from unknown battle chain
                    }
                }

                // Get character IDs from owner chains
                let player1_id = self.state.owner_to_character.get(&player1_chain).await
                    .ok().flatten();
                let player2_id = self.state.owner_to_character.get(&player2_chain).await
                    .ok().flatten();

                if let (Some(p1_id), Some(p2_id)) = (player1_id, player2_id) {
                    let winner_id = if winner_chain == player1_chain {
                        p1_id.clone()
                    } else {
                        p2_id.clone()
                    };

                    // Get opponent ELO ratings for ELO calculation
                    let p1_stats = self.state.characters.get(&p1_id).await.ok().flatten();
                    let p2_stats = self.state.characters.get(&p2_id).await.ok().flatten();

                    if let (Some(p1_elo_rating), Some(p2_elo_rating)) =
                        (p1_stats.map(|s| s.elo_rating), p2_stats.map(|s| s.elo_rating)) {

                        // Update player 1 stats
                        let p1_won = winner_chain == player1_chain;
                        let p1_earnings = if p1_won { stake } else { Amount::ZERO };
                        let _ = self.execute_operation(Operation::UpdateCharacterStats {
                            character_id: p1_id.clone(),
                            won: p1_won,
                            damage_dealt: player1_stats.damage_dealt,
                            damage_taken: player1_stats.damage_taken,
                            crits: player1_stats.crits,
                            dodges: player1_stats.dodges,
                            highest_crit: player1_stats.highest_crit,
                            earnings: p1_earnings,
                            stake,
                            opponent_elo: p2_elo_rating,
                        }).await;

                        // Update player 2 stats
                        let p2_won = winner_chain == player2_chain;
                        let p2_earnings = if p2_won { stake } else { Amount::ZERO };
                        let _ = self.execute_operation(Operation::UpdateCharacterStats {
                            character_id: p2_id.clone(),
                            won: p2_won,
                            damage_dealt: player2_stats.damage_dealt,
                            damage_taken: player2_stats.damage_taken,
                            crits: player2_stats.crits,
                            dodges: player2_stats.dodges,
                            highest_crit: player2_stats.highest_crit,
                            earnings: p2_earnings,
                            stake,
                            opponent_elo: p1_elo_rating,
                        }).await;
                    }

                    // Record battle
                    let _ = self.execute_operation(Operation::RecordBattle {
                        battle_chain,
                        player1_id: p1_id,
                        player2_id: p2_id,
                        winner_id,
                        stake,
                        rounds_played,
                    }).await;
                }
            }

            Message::CharacterRegistered { .. } => {
                // Informational only
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Registry Service
pub struct RegistryService {
    state: RegistryState,
}

impl WithServiceAbi for RegistryService {
    type Abi = RegistryAbi;
}

impl Service for RegistryService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = RegistryState::load(runtime.root_view_storage_context())
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

/// GraphQL Query Root - simplified version without state references
#[derive(Clone)]
struct QueryRoot {
    total_characters: u64,
    total_battles: u64,
    total_volume: String,
    top_character_ids: Vec<String>,
}

impl QueryRoot {
    async fn new(state: &RegistryState) -> Self {
        Self {
            total_characters: *state.total_characters.get(),
            total_battles: *state.total_battles.get(),
            total_volume: state.total_volume.get().to_string(),
            top_character_ids: state.top_elo.get().clone(),
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get total number of registered characters
    async fn total_characters(&self) -> i64 {
        self.total_characters as i64
    }

    /// Get total number of battles recorded
    async fn total_battles(&self) -> i64 {
        self.total_battles as i64
    }

    /// Get total volume wagered
    async fn total_volume(&self) -> String {
        self.total_volume.clone()
    }

    /// Get global registry stats
    async fn stats(&self) -> RegistryStats {
        RegistryStats {
            total_characters: self.total_characters,
            total_battles: self.total_battles,
            total_volume: self.total_volume.clone(),
        }
    }

    /// Get top character IDs by ELO (for leaderboard)
    /// Note: Full character data requires separate queries per ID
    async fn top_characters(&self, limit: Option<i32>) -> Vec<String> {
        let limit = limit.unwrap_or(10).min(100) as usize;
        self.top_character_ids.iter().take(limit).cloned().collect()
    }
}

#[derive(SimpleObject)]
struct RegistryStats {
    total_characters: u64,
    total_battles: u64,
    total_volume: String,
}
