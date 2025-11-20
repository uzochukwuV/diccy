use async_graphql::{Request, Response, Schema, EmptySubscription, SimpleObject, Object};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{AccountOwner, Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
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
#[view(context = ViewStorageContext)]
pub struct BattleTokenState {
    /// Token metadata
    pub name: RegisterView<String>,
    pub symbol: RegisterView<String>,
    pub decimals: RegisterView<u8>,
    pub total_supply: RegisterView<Amount>,

    /// Admin account (can mint tokens)
    pub admin: RegisterView<Option<Owner>>,

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
}

impl BattleTokenState {
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
            let mut accounts = self.accounts.get().clone();
            if !accounts.contains(&to) {
                accounts.push(to);
                self.accounts.set(accounts);
                self.total_holders.set(*self.total_holders.get() + 1);
            }
        }

        // Update stats
        self.total_transfers.set(*self.total_transfers.get() + 1);
        self.last_activity.set(now);

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
        let new_total_supply = self.total_supply.get()
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_supply.set(new_total_supply);

        let new_total_burned = self.total_burned.get()
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_burned.set(new_total_burned);

        self.last_activity.set(now);

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
        let new_total_supply = self.total_supply.get()
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_supply.set(new_total_supply);

        // Track new holder
        if balance == Amount::ZERO {
            let mut accounts = self.accounts.get().clone();
            if !accounts.contains(&to) {
                accounts.push(to);
                self.accounts.set(accounts);
                self.total_holders.set(*self.total_holders.get() + 1);
            }
        }

        self.last_activity.set(now);

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
    state: BattleTokenState,
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
        self.state.name.set("BattleChain Token".to_string());
        self.state.symbol.set("BATTLE".to_string());
        self.state.decimals.set(6);
        self.state.total_supply.set(initial_supply);
        self.state.total_transfers.set(0);
        self.state.total_holders.set(1);
        self.state.total_burned.set(Amount::ZERO);
        self.state.created_at.set(now);
        self.state.last_activity.set(now);

        // Set creator as admin
        self.state.admin.set(Some(creator.clone()));
        log::info!("BattleChain Token initialized with admin: {:?}", creator);

        // Mint initial supply to creator
        self.state.balances.insert(&creator, initial_supply).expect("Failed to set initial balance");
        let mut accounts = self.state.accounts.get().clone();
        accounts.push(creator);
        self.state.accounts.set(accounts);
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
                        log::info!("Transfer successful: {:?} -> {:?}, amount: {}", caller, to, amount);
                    }
                    Err(e) => {
                        log::error!("Transfer failed: {:?} -> {:?}, amount: {}, error: {:?}", caller, to, amount, e);
                        panic!("Transfer failed: {:?}", e);
                    }
                }
            }

            Operation::Approve { spender, amount } => {
                match self.state.approve(caller, spender, amount).await {
                    Ok(_) => {
                        log::info!("Approval successful: owner {:?} approved {:?} to spend {}", caller, spender, amount);
                    }
                    Err(e) => {
                        log::error!("Approval failed: owner {:?}, spender {:?}, amount: {}, error: {:?}", caller, spender, amount, e);
                        panic!("Approval failed: {:?}", e);
                    }
                }
            }

            Operation::TransferFrom { from, to, amount } => {
                match self.state.transfer_from(caller, from, to, amount, now).await {
                    Ok(_) => {
                        log::info!("TransferFrom successful: spender {:?} transferred {} from {:?} to {:?}", caller, amount, from, to);
                    }
                    Err(e) => {
                        log::error!("TransferFrom failed: spender {:?}, from {:?}, to {:?}, amount: {}, error: {:?}", caller, from, to, amount, e);
                        panic!("TransferFrom failed: {:?}", e);
                    }
                }
            }

            Operation::Burn { amount } => {
                match self.state.burn(caller, amount, now).await {
                    Ok(_) => {
                        log::info!("Burn successful: {:?} burned {}", caller, amount);
                    }
                    Err(e) => {
                        log::error!("Burn failed: {:?}, amount: {}, error: {:?}", caller, amount, e);
                        panic!("Burn failed: {:?}", e);
                    }
                }
            }

            Operation::Mint { to, amount } => {
                // SECURITY: Only admin can mint tokens
                let admin = self.state.admin.get().as_ref();
                if admin != Some(&caller) {
                    log::error!("Unauthorized mint attempt: {:?} tried to mint {} to {:?}. Only admin {:?} can mint.", caller, amount, to, admin);
                    panic!("Unauthorized: Only admin can mint tokens");
                }

                match self.state.mint(to, amount, now).await {
                    Ok(_) => {
                        log::info!("Mint successful: admin {:?} minted {} to {:?}", caller, amount, to);
                    }
                    Err(e) => {
                        log::error!("Mint failed: admin {:?}, to {:?}, amount: {}, error: {:?}", caller, to, amount, e);
                        panic!("Mint failed: {:?}", e);
                    }
                }
            }

            Operation::Claim { amount } => {
                // For reward claims or initial distribution
                // SECURITY: Only admin can approve claims
                let admin = self.state.admin.get().as_ref();
                if admin != Some(&caller) {
                    log::error!("Unauthorized claim attempt: {:?} tried to claim {}. Only admin {:?} can process claims.", caller, amount, admin);
                    panic!("Unauthorized: Only admin can process claims");
                }

                match self.state.mint(caller, amount, now).await {
                    Ok(_) => {
                        log::info!("Claim successful: admin {:?} claimed {}", caller, amount);
                    }
                    Err(e) => {
                        log::error!("Claim failed: {:?}, amount: {}, error: {:?}", caller, amount, e);
                        panic!("Claim failed: {:?}", e);
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
                        match self.state.transfer(from, to, amount, now).await {
                            Ok(_) => {
                                log::info!("Cross-chain transfer debit successful: {:?} -> {:?}, amount: {}", from, to, amount);
                                // TODO: Send credit message to target chain
                                // self.runtime.send_message(target_chain, Message::Credit { recipient: to, amount });
                            }
                            Err(e) => {
                                log::error!("Cross-chain transfer debit failed: {:?} -> {:?}, amount: {}, error: {:?}", from, to, amount, e);
                                panic!("Cross-chain transfer debit failed: {:?}", e);
                            }
                        }
                    }
                    balance => {
                        log::error!("Insufficient balance for cross-chain transfer: {:?} has {}, needs {}", from, balance, amount);
                        panic!("Insufficient balance for cross-chain transfer");
                    }
                }
            }

            Message::Credit { recipient, amount } => {
                // Credit tokens received from another chain
                match self.state.mint(recipient, amount, now).await {
                    Ok(_) => {
                        log::info!("Cross-chain credit successful: minted {} to {:?}", amount, recipient);
                    }
                    Err(e) => {
                        log::error!("Cross-chain credit failed: recipient {:?}, amount: {}, error: {:?}", recipient, amount, e);
                        panic!("Cross-chain credit failed: {:?}", e);
                    }
                }
            }

            Message::Debit { sender, amount } => {
                // Confirmation of tokens sent to another chain
                match self.state.burn(sender, amount, now).await {
                    Ok(_) => {
                        log::info!("Cross-chain debit confirmation successful: burned {} from {:?}", amount, sender);
                    }
                    Err(e) => {
                        log::error!("Cross-chain debit confirmation failed: sender {:?}, amount: {}, error: {:?}", sender, amount, e);
                        panic!("Cross-chain debit confirmation failed: {:?}", e);
                    }
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
    state: BattleTokenState,
}

impl WithServiceAbi for BattleTokenService {
    type Abi = BattleTokenAbi;
}

// NOTE: Only one of contract! or service! can be used per library
// The contract! macro includes service functionality
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

struct EmptyMutation;

#[async_graphql::Object]
impl EmptyMutation {
    async fn placeholder(&self) -> bool {
        false
    }
}
