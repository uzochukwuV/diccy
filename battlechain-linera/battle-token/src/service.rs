use async_graphql::{EmptyMutation, EmptySubscription, Object, Request, Response, Schema, SimpleObject};
use linera_sdk::{
    abi::WithServiceAbi,
    linera_base_types::Amount,
    views::{RootView, View},
    Service, ServiceRuntime,
};

use crate::{BattleTokenAbi, BattleTokenState};

/// Token Service (GraphQL queries)
pub struct BattleTokenService {
    pub state: BattleTokenState,
}

impl WithServiceAbi for BattleTokenService {
    type Abi = BattleTokenAbi;
}

impl Service for BattleTokenService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
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

/// Balance information for GraphQL
#[derive(Clone, SimpleObject)]
pub struct BalanceInfo {
    pub owner: String,
    pub amount: String,
}

/// Allowance information for GraphQL
#[derive(Clone, SimpleObject)]
pub struct AllowanceInfo {
    pub owner: String,
    pub spender: String,
    pub amount: String,
}

/// GraphQL Query Root
#[derive(Clone)]
struct QueryRoot {
    // Cache frequently accessed values
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Amount,
    total_burned: Amount,
    total_holders: u64,
    total_transfers: u64,
    // Store balances and allowances as GraphQL-ready structs
    balances: Vec<BalanceInfo>,
    allowances: Vec<AllowanceInfo>,
}

impl QueryRoot {
    async fn new(state: &BattleTokenState) -> Self {
        // Pre-load all balances for queries
        let balance_keys = state.balances.indices().await.expect("Failed to get balance keys");
        let mut balances = Vec::new();
        for key in balance_keys {
            if let Some(amount) = state.balances.get(&key).await.expect("Failed to get balance") {
                balances.push(BalanceInfo {
                    owner: format!("{:?}", key),  // Serialize Owner to string
                    amount: amount.to_string(),
                });
            }
        }

        // Pre-load all allowances for queries
        let allowance_keys = state.allowances.indices().await.expect("Failed to get allowance keys");
        let mut allowances = Vec::new();
        for key in allowance_keys {
            if let Some(amount) = state.allowances.get(&key).await.expect("Failed to get allowance") {
                allowances.push(AllowanceInfo {
                    owner: format!("{:?}", key.0),  // Serialize owner to string
                    spender: format!("{:?}", key.1),  // Serialize spender to string
                    amount: amount.to_string(),
                });
            }
        }

        Self {
            name: state.name.get().clone(),
            symbol: state.symbol.get().clone(),
            decimals: *state.decimals.get(),
            total_supply: *state.total_supply.get(),
            total_burned: *state.total_burned.get(),
            total_holders: *state.total_holders.get(),
            total_transfers: *state.total_transfers.get(),
            balances,
            allowances,
        }
    }
}

#[Object]
impl QueryRoot {
    /// Token name
    async fn name(&self) -> String {
        self.name.clone()
    }

    /// Token symbol
    async fn symbol(&self) -> String {
        self.symbol.clone()
    }

    /// Token decimals
    async fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Total supply
    async fn total_supply(&self) -> String {
        self.total_supply.to_string()
    }

    /// Total burned
    async fn total_burned(&self) -> String {
        self.total_burned.to_string()
    }

    /// Circulating supply (total - burned)
    async fn circulating_supply(&self) -> String {
        self.total_supply.saturating_sub(self.total_burned).to_string()
    }

    /// Get all account balances (microcard pattern)
    async fn balances(&self) -> Vec<BalanceInfo> {
        self.balances.clone()
    }

    /// Get all allowances (microcard pattern)
    async fn allowances(&self) -> Vec<AllowanceInfo> {
        self.allowances.clone()
    }

    /// Total number of token holders
    async fn total_holders(&self) -> u64 {
        self.total_holders
    }

    /// Total number of transfers
    async fn total_transfers(&self) -> u64 {
        self.total_transfers
    }

    /// Token statistics
    async fn stats(&self) -> TokenStats {
        TokenStats {
            total_supply: self.total_supply.to_string(),
            total_burned: self.total_burned.to_string(),
            circulating_supply: self.total_supply.saturating_sub(self.total_burned).to_string(),
            total_holders: self.total_holders,
            total_transfers: self.total_transfers,
        }
    }
}

#[derive(SimpleObject)]
struct TokenStats {
    total_supply: String,
    total_burned: String,
    circulating_supply: String,
    total_holders: u64,
    total_transfers: u64,
}
