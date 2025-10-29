#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{Context, EmptySubscription, Request, Response, Schema};
use linera_sdk::{
    graphql::GraphQLMutationRoot as _, linera_base_types::WithServiceAbi, views::View, Service,
    ServiceRuntime,
};

use state::DiceState;

#[derive(Clone)]
pub struct DiceService {
    runtime: Arc<ServiceRuntime<DiceService>>,
    state: Arc<DiceState>,
}

linera_sdk::service!(DiceService);

impl WithServiceAbi for DiceService {
    type Abi = crate::lib::DiceAbi;
}

impl Service for DiceService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = DiceState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        DiceService {
            runtime: Arc::new(runtime),
            state: Arc::new(state),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            self.state.clone(),
            // No mutations exposed from the service; use the contract for operations.
            crate::lib::Operation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
        .data(self.runtime.clone())
        .finish();
        schema.execute(request).await
    }
}

#[async_graphql::Object]
impl DiceState {
    /// Returns winner (if a match is settled and there is a winner).
    async fn match_winner(
        &self,
        ctx: &Context<'_>,
        match_id: u64,
    ) -> Option<linera_sdk::linera_base_types::AccountOwner> {
        if let Some(record) = self.matches.get(match_id).await.unwrap() {
            return record.winner.clone();
        }
        None
    }

    /// Simple query for a player profile by owner.
    async fn profile(
        &self,
        ctx: &Context<'_>,
        owner: linera_sdk::linera_base_types::AccountOwner,
    ) -> Option<super::state::PlayerProfile> {
        self.profiles.get(owner).await.unwrap()
    }
}

#[cfg(test)]
mod tests {
    use async_graphql::Request;
    use linera_sdk::{util::BlockingWait, views::View, ServiceRuntime};

    use super::*;

    #[test]
    fn query_clock() {
        let runtime = ServiceRuntime::<DiceService>::new();
        let state = DiceState::load(runtime.root_view_storage_context())
            .blocking_wait()
            .expect("Failed to read from mock key value store");

        let service = DiceService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        };

        let response = service
            .handle_query(Request::new("{ nextMatchId }"))
            .now_or_never()
            .expect("Query should not await anything")
            .data
            .into_json()
            .expect("Response should be JSON");

        // The default nextMatchId is 0 in our state
        assert_eq!(response, serde_json::json!({"nextMatchId": 0}));
    }
}
