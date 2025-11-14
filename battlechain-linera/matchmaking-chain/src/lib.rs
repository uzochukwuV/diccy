use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
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

/// Matchmaking State - coordinates battle matchmaking
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct MatchmakingState {
    /// Players waiting for matches (player_chain -> stake amount)
    pub waiting_players: MapView<ChainId, Amount>,

    /// Count of waiting players
    pub waiting_player_count: RegisterView<usize>,

    /// Active battles
    pub active_battles: RegisterView<Vec<ChainId>>,

    /// Completed battles
    pub completed_battles: RegisterView<Vec<ChainId>>,

    /// Total battles created
    pub total_battles: RegisterView<u64>,

    /// Minimum stake required
    pub min_stake: RegisterView<Amount>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}

impl MatchmakingState {
    /// Add player to waiting queue
    pub fn add_waiting_player(&mut self, player_chain: ChainId, stake: Amount) -> Result<(), MatchmakingError> {
        if stake < *self.min_stake.get() {
            return Err(MatchmakingError::InsufficientStake {
                provided: stake,
                required: *self.min_stake.get(),
            });
        }

        self.waiting_players.insert(&player_chain, stake)
            .map_err(|e| MatchmakingError::ViewError(format!("{:?}", e)))?;

        self.waiting_player_count.set(*self.waiting_player_count.get() + 1);

        Ok(())
    }

    /// Remove player from waiting queue
    pub async fn remove_waiting_player(&mut self, player_chain: &ChainId) -> Result<Amount, MatchmakingError> {
        let stake = self.waiting_players
            .get(player_chain)
            .await
            .map_err(|e| MatchmakingError::ViewError(format!("{:?}", e)))?
            .ok_or(MatchmakingError::PlayerNotWaiting)?;

        self.waiting_players.remove(player_chain)
            .map_err(|e| MatchmakingError::ViewError(format!("{:?}", e)))?;

        let count = self.waiting_player_count.get().saturating_sub(1);
        self.waiting_player_count.set(count);

        Ok(stake)
    }
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Join matchmaking queue
    JoinQueue { player_chain: ChainId, stake: Amount },

    /// Leave matchmaking queue
    LeaveQueue,

    /// Record battle completion
    RecordBattle { battle_chain: ChainId },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Match found notification
    MatchFound {
        battle_chain: ChainId,
        opponent: ChainId,
        stake: Amount,
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

        self.state.waiting_player_count.set(0);
        self.state.active_battles.set(Vec::new());
        self.state.completed_battles.set(Vec::new());
        self.state.total_battles.set(0);
        self.state.min_stake.set(min_stake);
        self.state.created_at.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::JoinQueue { player_chain, stake } => {
                let _ = self.state.add_waiting_player(player_chain, stake);

                // TODO: Check for matches and create battles
            }

            Operation::LeaveQueue => {
                // Get caller's chain
                // let _ = self.state.remove_waiting_player(&caller_chain).await;
            }

            Operation::RecordBattle { battle_chain } => {
                let mut active = self.state.active_battles.get().clone();
                active.retain(|c| c != &battle_chain);
                self.state.active_battles.set(active);

                let mut completed = self.state.completed_battles.get().clone();
                completed.push(battle_chain);
                self.state.completed_battles.set(completed);
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
    total_battles: u64,
    min_stake: Amount,
}

impl QueryRoot {
    async fn new(state: &MatchmakingState) -> Self {
        Self {
            waiting_player_count: *state.waiting_player_count.get(),
            active_battle_count: state.active_battles.get().len(),
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

    /// Get matchmaking stats
    async fn stats(&self) -> MatchmakingStats {
        MatchmakingStats {
            waiting_players: self.waiting_player_count as i32,
            active_battles: self.active_battle_count as i32,
            total_battles: self.total_battles,
        }
    }
}

#[derive(SimpleObject)]
struct MatchmakingStats {
    waiting_players: i32,
    active_battles: i32,
    total_battles: u64,
}
