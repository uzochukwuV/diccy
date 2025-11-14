use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use battlechain_shared_types::{CharacterSnapshot, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{
        Amount, ApplicationId, ApplicationPermissions, ChainId, ChainOwnership, Timestamp,
    },
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

/// Matchmaking Chain Application ABI
pub struct MatchmakingAbi;

impl ContractAbi for MatchmakingAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for MatchmakingAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Player queue entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub player_chain: ChainId,
    pub player_owner: Owner,
    pub character: CharacterSnapshot,
    pub stake: Amount,
    pub joined_at: Timestamp,
}

/// Battle offer waiting for confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBattle {
    pub offer_id: u64,
    pub player1: QueueEntry,
    pub player2: QueueEntry,
    pub created_at: Timestamp,
    pub player1_confirmed: bool,
    pub player2_confirmed: bool,
}

/// Matchmaking State - coordinates battle matchmaking
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct MatchmakingState {
    /// Players waiting for matches (player_chain -> queue entry)
    pub waiting_players: MapView<ChainId, QueueEntry>,

    /// Pending battle offers (offer_id -> pending battle)
    pub pending_battles: MapView<u64, PendingBattle>,

    /// Next offer ID
    pub next_offer_id: RegisterView<u64>,

    /// Active battles (battle_chain -> metadata)
    pub active_battles: MapView<ChainId, BattleMetadata>,

    /// Completed battles
    pub completed_battles: RegisterView<Vec<ChainId>>,

    /// Total battles created
    pub total_battles: RegisterView<u64>,

    /// Minimum stake required
    pub min_stake: RegisterView<Amount>,

    /// Battle chain application ID
    pub battle_app_id: RegisterView<Option<ApplicationId>>,

    /// Battle token application ID
    pub battle_token_app: RegisterView<Option<ApplicationId>>,

    /// Platform fee basis points (300 = 3%)
    pub platform_fee_bps: RegisterView<u16>,

    /// Treasury owner
    pub treasury_owner: RegisterView<Option<Owner>>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}

/// Battle metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMetadata {
    pub player1: ChainId,
    pub player2: ChainId,
    pub stake: Amount,
    pub started_at: Timestamp,
}

impl MatchmakingState {
    /// Add player to waiting queue
    pub fn add_waiting_player(&mut self, entry: QueueEntry) -> Result<(), MatchmakingError> {
        if entry.stake < *self.min_stake.get() {
            return Err(MatchmakingError::InsufficientStake {
                provided: entry.stake,
                required: *self.min_stake.get(),
            });
        }

        let player_chain = entry.player_chain;
        self.waiting_players.insert(&player_chain, entry)?;
        Ok(())
    }

    /// Remove player from waiting queue
    pub async fn remove_waiting_player(&mut self, player_chain: &ChainId) -> Result<QueueEntry, MatchmakingError> {
        let entry = self.waiting_players
            .get(player_chain)
            .await?
            .ok_or(MatchmakingError::PlayerNotWaiting)?;

        self.waiting_players.remove(player_chain)?;
        Ok(entry)
    }

    /// Find a match for a player
    pub async fn find_match(&self, _player_chain: &ChainId) -> Option<(ChainId, QueueEntry)> {
        // Simple FIFO matching: find first available opponent
        // TODO: Implement skill-based matchmaking
        // We need to iterate through waiting players to find an opponent
        // For now, this is a placeholder - in practice we'd need to collect keys first
        // Since we can't easily iterate MapView, we'll handle this in the contract

        None
    }
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Join matchmaking queue
    JoinQueue {
        player_chain: ChainId,
        player_owner: Owner,
        character: CharacterSnapshot,
        stake: Amount,
    },

    /// Leave matchmaking queue
    LeaveQueue { player_chain: ChainId },

    /// Create battle offer (matchmaker only - called after 2 players join)
    CreateBattleOffer {
        player1_chain: ChainId,
        player2_chain: ChainId,
    },

    /// Confirm battle offer (player accepts match)
    ConfirmBattleOffer {
        offer_id: u64,
        player_chain: ChainId,
    },

    /// Record battle completion
    RecordBattleCompletion { battle_chain: ChainId },

    /// Update application references
    UpdateReferences {
        battle_app_id: ApplicationId,
        battle_token_app: ApplicationId,
        treasury_owner: Owner,
    },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle offer notification - sent to both players
    BattleOffer {
        offer_id: u64,
        opponent_chain: ChainId,
        stake: Amount,
    },

    /// Battle created - sent after both confirm
    BattleCreated {
        battle_chain: ChainId,
        opponent: ChainId,
    },

    /// Battle completion notification (from battle chain)
    BattleCompleted {
        winner: Owner,
        loser: Owner,
    },
}

/// Errors
#[derive(Debug, Error)]
pub enum MatchmakingError {
    #[error("Insufficient stake: provided {provided}, required {required}")]
    InsufficientStake { provided: Amount, required: Amount },

    #[error("Player not in waiting queue")]
    PlayerNotWaiting,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for MatchmakingError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        MatchmakingError::ViewError(format!("{:?}", err))
    }
}

/// Matchmaking Contract
pub struct MatchmakingContract {
    state: MatchmakingState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(MatchmakingContract);

impl WithContractAbi for MatchmakingContract {
    type Abi = MatchmakingAbi;
}

impl MatchmakingContract {
    /// Create a new battle chain with multi-owner ownership
    async fn create_battle_chain(&mut self, pending: PendingBattle) {
        let battle_app_id = self.state.battle_app_id.get()
            .clone()
            .expect("Battle app ID not configured");

        // Create multi-owner chain ownership with both players
        // Each player gets equal weight (100)
        let mut owners = BTreeMap::new();
        owners.insert(pending.player1.player_owner, 100);
        owners.insert(pending.player2.player_owner, 100);

        let chain_ownership = ChainOwnership {
            super_owners: Default::default(), // No super owners for battle chains
            owners,
            multi_leader_rounds: 10, // Allow 10 rounds of multi-leader consensus
            open_multi_leader_rounds: false, // Only the two players can propose
            timeout_config: Default::default(), // Use default timeouts
        };

        // Configure application permissions
        // Only allow the battle application to execute operations
        let application_permissions = ApplicationPermissions {
            execute_operations: Some(vec![battle_app_id]),
            mandatory_applications: vec![],
            close_chain: vec![battle_app_id], // Battle app can close chain when battle ends
            change_application_permissions: vec![],
            call_service_as_oracle: None, // No oracle calls needed
            make_http_requests: None, // No HTTP requests needed
        };

        // Calculate total stake to fund the new chain
        let total_stake = pending.player1.stake.saturating_add(pending.player2.stake);

        // Create the battle chain!
        let battle_chain_id = self.runtime.open_chain(
            chain_ownership,
            application_permissions,
            total_stake, // Initial balance for the battle chain
        );

        // Store battle metadata
        let metadata = BattleMetadata {
            player1: pending.player1.player_chain,
            player2: pending.player2.player_chain,
            stake: total_stake,
            started_at: self.runtime.system_time(),
        };

        self.state.active_battles.insert(&battle_chain_id, metadata)
            .expect("Failed to store battle metadata");

        // Notify both players of battle creation
        let battle_msg = Message::BattleCreated {
            battle_chain: battle_chain_id,
            opponent: pending.player2.player_chain,
        };
        self.runtime
            .prepare_message(battle_msg.clone())
            .with_authentication()
            .send_to(pending.player1.player_chain);

        let battle_msg = Message::BattleCreated {
            battle_chain: battle_chain_id,
            opponent: pending.player1.player_chain,
        };
        self.runtime
            .prepare_message(battle_msg)
            .with_authentication()
            .send_to(pending.player2.player_chain);
    }
}

impl Contract for MatchmakingContract {
    type Message = Message;
    type Parameters = Amount; // Minimum stake
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = MatchmakingState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let min_stake = self.runtime.application_parameters();
        let now = self.runtime.system_time();

        self.state.next_offer_id.set(0);
        self.state.completed_battles.set(Vec::new());
        self.state.total_battles.set(0);
        self.state.min_stake.set(min_stake);
        self.state.battle_app_id.set(None);
        self.state.battle_token_app.set(None);
        self.state.platform_fee_bps.set(300); // 3% default
        self.state.treasury_owner.set(None);
        self.state.created_at.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::JoinQueue { player_chain, player_owner, character, stake } => {
                let now = self.runtime.system_time();

                let entry = QueueEntry {
                    player_chain,
                    player_owner,
                    character,
                    stake,
                    joined_at: now,
                };

                self.state.add_waiting_player(entry)
                    .expect("Failed to add player to queue");

                // TODO: Implement automatic matchmaking
                // For now, matchmaking must be triggered manually via CreateBattleOffer
            }

            Operation::LeaveQueue { player_chain } => {
                let _ = self.state.remove_waiting_player(&player_chain).await;
            }

            Operation::CreateBattleOffer { player1_chain, player2_chain } => {
                let now = self.runtime.system_time();

                // Get both players from queue
                let player1 = self.state.waiting_players.get(&player1_chain).await
                    .expect("View error")
                    .expect("Player 1 not in queue");
                let player2 = self.state.waiting_players.get(&player2_chain).await
                    .expect("View error")
                    .expect("Player 2 not in queue");

                // Create pending battle
                let offer_id = *self.state.next_offer_id.get();
                self.state.next_offer_id.set(offer_id + 1);

                let pending = PendingBattle {
                    offer_id,
                    player1: player1.clone(),
                    player2: player2.clone(),
                    created_at: now,
                    player1_confirmed: false,
                    player2_confirmed: false,
                };

                self.state.pending_battles.insert(&offer_id, pending)
                    .expect("Failed to insert pending battle");

                // Remove from waiting queue
                self.state.waiting_players.remove(&player1_chain)
                    .expect("Failed to remove player 1");
                self.state.waiting_players.remove(&player2_chain)
                    .expect("Failed to remove player 2");

                // Send battle offer notifications to both players
                let offer_msg_p1 = Message::BattleOffer {
                    offer_id,
                    opponent_chain: player2_chain,
                    stake: player1.stake,
                };
                let offer_msg_p2 = Message::BattleOffer {
                    offer_id,
                    opponent_chain: player1_chain,
                    stake: player2.stake,
                };

                self.runtime
                    .prepare_message(offer_msg_p1)
                    .with_authentication()
                    .send_to(player1_chain);

                self.runtime
                    .prepare_message(offer_msg_p2)
                    .with_authentication()
                    .send_to(player2_chain);
            }

            Operation::ConfirmBattleOffer { offer_id, player_chain } => {
                let mut pending = self.state.pending_battles.get(&offer_id).await
                    .expect("View error")
                    .expect("Battle offer not found");

                // Mark confirmation
                if pending.player1.player_chain == player_chain {
                    pending.player1_confirmed = true;
                } else if pending.player2.player_chain == player_chain {
                    pending.player2_confirmed = true;
                } else {
                    panic!("Caller not part of this battle offer");
                }

                // Check if both confirmed
                if pending.player1_confirmed && pending.player2_confirmed {
                    // Both confirmed - create battle chain!
                    self.create_battle_chain(pending.clone()).await;

                    // Remove pending battle
                    self.state.pending_battles.remove(&offer_id)
                        .expect("Failed to remove pending battle");
                } else {
                    // Update pending battle with confirmation
                    self.state.pending_battles.insert(&offer_id, pending)
                        .expect("Failed to update pending battle");
                }
            }

            Operation::RecordBattleCompletion { battle_chain } => {
                // Remove from active battles
                self.state.active_battles.remove(&battle_chain)
                    .expect("Failed to remove active battle");

                // Add to completed
                let mut completed = self.state.completed_battles.get().clone();
                completed.push(battle_chain);
                self.state.completed_battles.set(completed);

                // Increment total battles
                let total = *self.state.total_battles.get();
                self.state.total_battles.set(total + 1);
            }

            Operation::UpdateReferences { battle_app_id, battle_token_app, treasury_owner } => {
                self.state.battle_app_id.set(Some(battle_app_id));
                self.state.battle_token_app.set(Some(battle_token_app));
                self.state.treasury_owner.set(Some(treasury_owner));
            }
        }
    }

    async fn execute_message(&mut self, _message: Message) {
        // Handle match found notifications
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Matchmaking Service
pub struct MatchmakingService {
    state: MatchmakingState,
}

impl WithServiceAbi for MatchmakingService {
    type Abi = MatchmakingAbi;
}

impl Service for MatchmakingService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = MatchmakingState::load(runtime.root_view_storage_context())
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
    waiting_player_count: usize,
    active_battle_count: usize,
    pending_battle_count: usize,
    total_battles: u64,
    min_stake: Amount,
}

impl QueryRoot {
    async fn new(state: &MatchmakingState) -> Self {
        // Count entries in MapViews by getting the length of indices vectors
        let waiting_count = state.waiting_players.indices().await.expect("Failed to get indices").len();
        let active_count = state.active_battles.indices().await.expect("Failed to get indices").len();
        let pending_count = state.pending_battles.indices().await.expect("Failed to get indices").len();

        Self {
            waiting_player_count: waiting_count,
            active_battle_count: active_count,
            pending_battle_count: pending_count,
            total_battles: *state.total_battles.get(),
            min_stake: *state.min_stake.get(),
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get number of players waiting for matches
    async fn waiting_player_count(&self) -> i32 {
        self.waiting_player_count as i32
    }

    /// Get number of active battles
    async fn active_battle_count(&self) -> i32 {
        self.active_battle_count as i32
    }

    /// Get total battles created
    async fn total_battles(&self) -> i64 {
        self.total_battles as i64
    }

    /// Get minimum stake required
    async fn min_stake(&self) -> String {
        self.min_stake.to_string()
    }

    /// Get number of pending battle offers
    async fn pending_battle_count(&self) -> i32 {
        self.pending_battle_count as i32
    }

    /// Get matchmaking stats
    async fn stats(&self) -> MatchmakingStats {
        MatchmakingStats {
            waiting_players: self.waiting_player_count as i32,
            active_battles: self.active_battle_count as i32,
            pending_battles: self.pending_battle_count as i32,
            total_battles: self.total_battles,
        }
    }
}

#[derive(SimpleObject)]
struct MatchmakingStats {
    waiting_players: i32,
    active_battles: i32,
    pending_battles: i32,
    total_battles: u64,
}
