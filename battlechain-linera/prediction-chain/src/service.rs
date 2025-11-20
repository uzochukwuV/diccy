#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use linera_sdk::{
    abi::WithServiceAbi,
    views::{RootView, View},
    Service, ServiceRuntime,
};
use prediction_chain::PredictionAbi;
use self::state::PredictionState;

/// Prediction Service
pub struct PredictionService {
    pub state: PredictionState,
}

impl WithServiceAbi for PredictionService {
    type Abi = PredictionAbi;
}

impl Service for PredictionService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = PredictionState::load(runtime.root_view_storage_context())
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
    total_markets: u64,
    total_bets: u64,
    total_volume: String,
    platform_fee_bps: u16,
}

impl QueryRoot {
    async fn new(state: &PredictionState) -> Self {
        Self {
            total_markets: *state.total_markets.get(),
            total_bets: *state.total_bets.get(),
            total_volume: state.total_volume.get().to_string(),
            platform_fee_bps: *state.platform_fee_bps.get(),
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get total number of prediction markets created
    async fn total_markets(&self) -> i64 {
        self.total_markets as i64
    }

    /// Get total number of bets placed
    async fn total_bets(&self) -> i64 {
        self.total_bets as i64
    }

    /// Get total volume wagered
    async fn total_volume(&self) -> &str {
        &self.total_volume
    }

    /// Get platform fee in basis points
    async fn platform_fee_bps(&self) -> i32 {
        self.platform_fee_bps as i32
    }

    /// Get prediction market stats
    async fn stats(&self) -> PredictionStats {
        PredictionStats {
            total_markets: self.total_markets,
            total_bets: self.total_bets,
            total_volume: self.total_volume.clone(),
            platform_fee_bps: self.platform_fee_bps,
        }
    }
}

#[derive(SimpleObject)]
struct PredictionStats {
    total_markets: u64,
    total_bets: u64,
    total_volume: String,
    platform_fee_bps: u16,
}
