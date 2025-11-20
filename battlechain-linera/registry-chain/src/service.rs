#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use linera_sdk::{
    abi::WithServiceAbi,
    views::{RootView, View},
    Service, ServiceRuntime,
};
use registry_chain::RegistryAbi;
use self::state::RegistryState;

/// Registry Service
pub struct RegistryService {
    pub state: RegistryState,
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
