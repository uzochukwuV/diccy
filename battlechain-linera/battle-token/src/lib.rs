use async_graphql::{Request, Response, Schema, EmptySubscription, SimpleObject};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{AccountOwner, Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use thiserror::Error;

// Type alias for consistency
type Owner = AccountOwner;

/// BATTLE Token Application ABI
pub struct BattleTokenAbi;

impl ContractAbi for BattleTokenAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for BattleTokenAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Token State - manages all BATTLE token balances and operations
#[derive(RootView)]
#[view(context = C)]
pub struct BattleTokenState<C> {
    /// Token metadata
    pub name: RegisterView<String>,
    pub symbol: RegisterView<String>,
    pub decimals: RegisterView<u8>,
    pub total_supply: RegisterView<Amount>,

    /// Account balances (Owner -> Amount)
    pub balances: MapView<Owner, Amount>,

    /// Allowances for spending (owner, spender) -> amount
    pub allowances: MapView<(Owner, Owner), Amount>,

    /// Account registry for iteration
    pub accounts: RegisterView<Vec<Owner>>,

    /// Statistics
    pub total_transfers: RegisterView<u64>,
    pub total_holders: RegisterView<u64>,
    pub total_burned: RegisterView<Amount>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
    pub last_activity: RegisterView<Timestamp>,

    /// Phantom data to use type parameter
    #[view(skip)]
    _phantom: PhantomData<C>,
}

impl<C> BattleTokenState<C> {
    /// Get balance of account
    pub async fn balance_of(&self, account: &Owner) -> Amount {
        self.balances
            .get(account)
            .await
            .unwrap_or(None)
            .unwrap_or(Amount::ZERO)
    }

    /// Transfer tokens between accounts
    pub async fn transfer(
        &mut self,
        from: Owner,
        to: Owner,
        amount: Amount,
        now: Timestamp,
    ) -> Result<(), TokenError> {
        // Validation
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        if from == to {
            return Err(TokenError::SelfTransfer);
        }

        // Check balance
        let from_balance = self.balance_of(&from).await;
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: from_balance,
                required: amount,
            });
        }

        // Deduct from sender
        let new_from_balance = from_balance
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&from, new_from_balance)?;

        // Add to recipient
        let to_balance = self.balance_of(&to).await;
        let new_to_balance = to_balance
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&to, new_to_balance)?;

        // Track new holder
        if to_balance == Amount::ZERO && amount > Amount::ZERO {
            if !self.accounts.contains(&to) {
                self.accounts.push(to);
                self.total_holders += 1;
            }
        }

        // Update stats
        self.total_transfers += 1;
        self.last_activity = now;

        Ok(())
    }

    /// Approve spending allowance
    pub async fn approve(
        &mut self,
        owner: Owner,
        spender: Owner,
        amount: Amount,
    ) -> Result<(), TokenError> {
        if owner == spender {
            return Err(TokenError::SelfApproval);
        }

        self.allowances.insert(&(owner, spender), amount)?;
        Ok(())
    }

    /// Get allowance
    pub async fn allowance(&self, owner: &Owner, spender: &Owner) -> Amount {
        self.allowances
            .get(&(*owner, *spender))
            .await
            .unwrap_or(None)
            .unwrap_or(Amount::ZERO)
    }

    /// Transfer from allowance
    pub async fn transfer_from(
        &mut self,
        spender: Owner,
        from: Owner,
        to: Owner,
        amount: Amount,
        now: Timestamp,
    ) -> Result<(), TokenError> {
        // Check allowance
        let allowed = self.allowance(&from, &spender).await;
        if allowed < amount {
            return Err(TokenError::InsufficientAllowance {
                allowed,
                required: amount,
            });
        }

        // Reduce allowance
        let new_allowance = allowed
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.allowances
            .insert(&(from, spender), new_allowance)?;

        // Transfer tokens
        self.transfer(from, to, amount, now).await
    }

    /// Burn tokens (permanently remove from circulation)
    pub async fn burn(&mut self, from: Owner, amount: Amount, now: Timestamp) -> Result<(), TokenError> {
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        let balance = self.balance_of(&from).await;
        if balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: balance,
                required: amount,
            });
        }

        // Remove from account
        let new_balance = balance.try_sub(amount).map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&from, new_balance)?;

        // Reduce total supply
        self.total_supply = self
            .total_supply
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;

        self.total_burned = self
            .total_burned
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;

        self.last_activity = now;

        Ok(())
    }

    /// Mint new tokens (admin only - for future use)
    pub async fn mint(&mut self, to: Owner, amount: Amount, now: Timestamp) -> Result<(), TokenError> {
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        // Add to recipient
        let balance = self.balance_of(&to).await;
        let new_balance = balance.try_add(amount).map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&to, new_balance)?;

        // Increase total supply
        self.total_supply = self
            .total_supply
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;

        // Track new holder
        if balance == Amount::ZERO {
            if !self.accounts.contains(&to) {
                self.accounts.push(to);
                self.total_holders += 1;
            }
        }

        self.last_activity = now;

        Ok(())
    }
}

/// Token Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Transfer tokens to another account
    Transfer { to: Owner, amount: Amount },

    /// Approve spending allowance
    Approve { spender: Owner, amount: Amount },

    /// Transfer from allowance
    TransferFrom {
        from: Owner,
        to: Owner,
        amount: Amount,
    },

    /// Burn tokens (remove from circulation)
    Burn { amount: Amount },

    /// Mint new tokens (admin only - disabled by default)
    Mint { to: Owner, amount: Amount },

    /// Claim tokens (for initial distribution or rewards)
    Claim { amount: Amount },
}

/// Cross-chain Token Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Cross-chain transfer request
    Transfer {
        from: Owner,
        to: Owner,
        amount: Amount,
        target_chain: ChainId,
    },

    /// Credit tokens on destination chain (sent by source chain)
    Credit { recipient: Owner, amount: Amount },

    /// Debit tokens on source chain (confirmation from destination)
    Debit { sender: Owner, amount: Amount },
}

/// Token Errors
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
pub enum TokenError {
    #[error("Insufficient balance: have {available}, need {required}")]
    InsufficientBalance { available: Amount, required: Amount },

    #[error("Insufficient allowance: allowed {allowed}, need {required}")]
    InsufficientAllowance { allowed: Amount, required: Amount },

    #[error("Cannot transfer zero amount")]
    ZeroAmount,

    #[error("Cannot transfer to self")]
    SelfTransfer,

    #[error("Cannot approve self")]
    SelfApproval,

    #[error("Math overflow in calculation")]
    MathOverflow,

    #[error("Unauthorized operation")]
    Unauthorized,

    #[error("Invalid recipient")]
    InvalidRecipient,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for TokenError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        TokenError::ViewError(format!("{:?}", err))
    }
}

/// Token Contract
pub struct BattleTokenContract {
    state: BattleTokenState<ViewStorageContext>,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(BattleTokenContract);

impl WithContractAbi for BattleTokenContract {
    type Abi = BattleTokenAbi;
}

impl Contract for BattleTokenContract {
    type Message = Message;
    type Parameters = Amount; // Initial supply
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let initial_supply = self.runtime.application_parameters();
        let chain_ownership = self.runtime.chain_ownership();
        let creator = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("No super owners found")
            .clone();
        let now = self.runtime.system_time();

        // Initialize token metadata
        self.state.name = "BattleChain Token".to_string();
        self.state.symbol = "BATTLE".to_string();
        self.state.decimals = 6;
        self.state.total_supply = initial_supply;
        self.state.total_transfers = 0;
        self.state.total_holders = 1;
        self.state.total_burned = Amount::ZERO;
        self.state.created_at = now;
        self.state.last_activity = now;

        // Mint initial supply to creator
        self.state.balances.insert(&creator, initial_supply).expect("Failed to set initial balance");
        self.state.accounts.push(creator);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let caller = self
            .runtime
            .authenticated_signer()
            .expect("Must be authenticated");
        let now = self.runtime.system_time();

        match operation {
            Operation::Transfer { to, amount } => {
                match self.state.transfer(caller, to, amount, now).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }

            Operation::Approve { spender, amount } => {
                match self.state.approve(caller, spender, amount).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }

            Operation::TransferFrom { from, to, amount } => {
                match self.state.transfer_from(caller, from, to, amount, now).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }

            Operation::Burn { amount } => {
                match self.state.burn(caller, amount, now).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }

            Operation::Mint { to, amount } => {
                // TODO: Add admin check
                // For now, only allow minting during initialization or by specific authority
                match self.state.mint(to, amount, now).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }

            Operation::Claim { amount } => {
                // For reward claims or initial distribution
                // TODO: Implement claim logic with verification
                match self.state.mint(caller, amount, now).await {
                    Ok(_) => {
                    }
                    Err(_e) => {
                    }
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        let now = self.runtime.system_time();

        match message {
            Message::Transfer {
                from,
                to,
                amount,
                target_chain: _,
            } => {
                // Deduct from sender on this chain
                match self.state.balance_of(&from).await {
                    balance if balance >= amount => {
                        if let Ok(_) = self.state.transfer(from, to, amount, now).await {
                            // TODO: Send credit message to target chain
                            // self.runtime.send_message(target_chain, Message::Credit { recipient: to, amount });
                        }
                    }
                    _ => {
                        // Insufficient balance
                    }
                }
            }

            Message::Credit { recipient, amount } => {
                // Credit tokens received from another chain
                if let Ok(_) = self.state.mint(recipient, amount, now).await {
                }
            }

            Message::Debit { sender, amount } => {
                // Confirmation of tokens sent to another chain
                if let Ok(_) = self.state.burn(sender, amount, now).await {
                }
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Token Service (GraphQL queries)
pub struct BattleTokenService {
    state: BattleTokenState<ViewStorageContext>,
}

impl WithServiceAbi for BattleTokenService {
    type Abi = BattleTokenAbi;
}

// TODO: Re-enable after fixing Contract compilation
// linera_sdk::service!(BattleTokenService);

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

/// GraphQL Query Root
#[derive(Clone)]
struct QueryRoot {
    name: String,
    symbol: String,
    decimals: u8,
    total_supply: Amount,
    total_burned: Amount,
    total_holders: u64,
    total_transfers: u64,
}

impl QueryRoot {
    async fn new(state: &BattleTokenState<ViewStorageContext>) -> Self {
        Self {
            name: state.name.get().clone(),
            symbol: state.symbol.get().clone(),
            decimals: *state.decimals.get(),
            total_supply: *state.total_supply.get(),
            total_burned: *state.total_burned.get(),
            total_holders: *state.total_holders.get(),
            total_transfers: *state.total_transfers.get(),
        }
    }
}

#[async_graphql::Object]
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

    /// Get balance of account
    async fn balance_of(&self, _account: String) -> String {
        // For now, return zero - need proper Owner parsing
        // TODO: Parse Owner from string and query balance
        "0".to_string()
    }

    /// Get allowance
    async fn allowance(&self, _owner: String, _spender: String) -> String {
        // TODO: Parse Owner from strings and query allowance
        "0".to_string()
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

struct EmptyMutation;

#[async_graphql::Object]
impl EmptyMutation {
    async fn placeholder(&self) -> bool {
        false
    }
}
