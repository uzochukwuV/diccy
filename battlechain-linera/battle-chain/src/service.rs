#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use battle_chain::BattleChainAbi;
use linera_sdk::{
    abi::WithServiceAbi,
    views::{RootView, View},
    Service, ServiceRuntime,
};
use self::state::{BattleState, BattleStatus};

/// Battle Service
pub struct BattleService {
    pub state: BattleState,
}

impl WithServiceAbi for BattleService {
    type Abi = BattleChainAbi;
}

impl Service for BattleService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = BattleState::load(runtime.root_view_storage_context())
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
    status: BattleStatus,
    current_round: u8,
    max_rounds: u8,
    round_count: usize,
    player1_hp: u32,
    player2_hp: u32,
}

impl QueryRoot {
    async fn new(state: &BattleState) -> Self {
        let (p1_hp, p2_hp) = if let (Some(p1), Some(p2)) = (state.player1.get(), state.player2.get()) {
            (p1.current_hp, p2.current_hp)
        } else {
            (0, 0)
        };

        Self {
            status: *state.status.get(),
            current_round: *state.current_round.get(),
            max_rounds: *state.max_rounds.get(),
            round_count: state.round_results.get().len(),
            player1_hp: p1_hp,
            player2_hp: p2_hp,
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get battle status
    async fn status(&self) -> String {
        format!("{:?}", self.status)
    }

    /// Get current round number
    async fn current_round(&self) -> i32 {
        self.current_round as i32
    }

    /// Get maximum rounds
    async fn max_rounds(&self) -> i32 {
        self.max_rounds as i32
    }

    /// Get completed round count
    async fn completed_rounds(&self) -> i32 {
        self.round_count as i32
    }

    /// Get player 1 HP
    async fn player1_hp(&self) -> i32 {
        self.player1_hp as i32
    }

    /// Get player 2 HP
    async fn player2_hp(&self) -> i32 {
        self.player2_hp as i32
    }

    /// Get battle info
    async fn battle_info(&self) -> BattleInfo {
        BattleInfo {
            status: format!("{:?}", self.status),
            current_round: self.current_round as i32,
            completed_rounds: self.round_count as i32,
            player1_hp: self.player1_hp as i32,
            player2_hp: self.player2_hp as i32,
        }
    }
}

#[derive(SimpleObject)]
struct BattleInfo {
    status: String,
    current_round: i32,
    completed_rounds: i32,
    player1_hp: i32,
    player2_hp: i32,
}
