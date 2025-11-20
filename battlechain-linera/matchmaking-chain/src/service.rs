use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use linera_sdk::{
    abi::WithServiceAbi,
    linera_base_types::Amount,
    views::View,
    Service, ServiceRuntime,
};

use crate::{MatchmakingAbi, MatchmakingState};

/// Matchmaking Service
pub struct MatchmakingService {
    pub state: MatchmakingState,
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
