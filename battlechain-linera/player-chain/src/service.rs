#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use linera_sdk::{
    abi::WithServiceAbi,
    linera_base_types::Amount,
    views::{RootView, View},
    Service, ServiceRuntime,
};
use player_chain::PlayerChainAbi;
use self::state::PlayerChainState;

/// Player Chain Service
pub struct PlayerChainService {
    pub state: PlayerChainState,
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
