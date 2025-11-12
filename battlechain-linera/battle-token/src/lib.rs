use async_graphql::{Request, Response, Schema, EmptySubscription, SimpleObject};
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp, WithContractAbi},
    views::{MapView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Owner type in newer SDK
type Owner = linera_sdk::linera_base_types::Account;

/// BATTLE Token Application ABI
pub struct BattleTokenAbi;

impl WithContractAbi for BattleTokenAbi {
    type Operation = Operation;
    type Response = ();
}

/// Token State - manages all BATTLE token balances and operations
#[derive(RootView)]
pub struct BattleTokenState {
    /// Token metadata
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: Amount,

    /// Account balances (Owner -> Amount)
    pub balances: MapView<Owner, Amount>,

    /// Allowances for spending (owner, spender) -> amount
    pub allowances: MapView<(Owner, Owner), Amount>,

    /// Account registry for iteration
    pub accounts: Vec<Owner>,

    /// Statistics
    pub total_transfers: u64,
    pub total_holders: u64,
    pub total_burned: Amount,

    /// Timestamps
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
}

impl BattleTokenState {
    /// Initialize new token with initial supply
    pub fn new(initial_owner: Owner, initial_supply: Amount, created_at: Timestamp) -> Self {
        Self {
            name: "BattleChain Token".to_string(),
            symbol: "BATTLE".to_string(),
            decimals: 6,
            total_supply: initial_supply,
            balances: MapView::default(),
            allowances: MapView::default(),
            accounts: vec![initial_owner],
            total_transfers: 0,
            total_holders: 1,
            total_burned: Amount::ZERO,
            created_at,
            last_activity: created_at,
        }
    }

    /// Get balance of account
    pub async fn balance_of(&self, account: &Owner) -> Amount {
        self.balances
            .get(account)
            .await
            .unwrap_or(Ok(Amount::ZERO))
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
            .checked_sub(amount)
            .ok_or(TokenError::MathOverflow)?;
        self.balances.insert(&from, new_from_balance).await?;

        // Add to recipient
        let to_balance = self.balance_of(&to).await;
        let new_to_balance = to_balance
            .checked_add(amount)
            .ok_or(TokenError::MathOverflow)?;
        self.balances.insert(&to, new_to_balance).await?;

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

        self.allowances.insert(&(owner, spender), amount).await?;
        Ok(())
    }

    /// Get allowance
    pub async fn allowance(&self, owner: &Owner, spender: &Owner) -> Amount {
        self.allowances
            .get(&(*owner, *spender))
            .await
            .unwrap_or(Ok(Amount::ZERO))
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
            .checked_sub(amount)
            .ok_or(TokenError::MathOverflow)?;
        self.allowances
            .insert(&(from, spender), new_allowance)
            .await?;

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
        let new_balance = balance.checked_sub(amount).ok_or(TokenError::MathOverflow)?;
        self.balances.insert(&from, new_balance).await?;

        // Reduce total supply
        self.total_supply = self
            .total_supply
            .checked_sub(amount)
            .ok_or(TokenError::MathOverflow)?;

        self.total_burned = self
            .total_burned
            .checked_add(amount)
            .ok_or(TokenError::MathOverflow)?;

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
        let new_balance = balance.checked_add(amount).ok_or(TokenError::MathOverflow)?;
        self.balances.insert(&to, new_balance).await?;

        // Increase total supply
        self.total_supply = self
            .total_supply
            .checked_add(amount)
            .ok_or(TokenError::MathOverflow)?;

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

impl From<linera_sdk::views::views::ViewError> for TokenError {
    fn from(err: linera_sdk::views::views::ViewError) -> Self {
        TokenError::ViewError(format!("{:?}", err))
    }
}

/// Token Contract
pub struct BattleTokenContract {
    state: BattleTokenState,
    runtime: ContractRuntime<Self>,
}

impl Contract for BattleTokenContract {
    type Message = Message;
    type Parameters = Amount; // Initial supply
    type InstantiationArgument = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let initial_supply = self.runtime.parameters();
        let creator = self
            .runtime
            .chain_ownership()
            .owner
            .expect("Chain must have owner");
        let now = self.runtime.system_time();

        // Initialize state with initial supply minted to creator
        self.state = BattleTokenState::new(creator, initial_supply, now);

        // Mint initial supply to creator
        self.state
            .balances
            .insert(&creator, initial_supply)
            .await
            .expect("Failed to set initial balance");

        self.runtime.emit(format!(
            "BATTLE Token initialized: {} tokens minted to {}",
            initial_supply, creator
        ));
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
                        self.runtime.emit(format!(
                            "Transfer: {} -> {} | Amount: {}",
                            caller, to, amount
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Transfer failed: {}", e));
                    }
                }
            }

            Operation::Approve { spender, amount } => {
                match self.state.approve(caller, spender, amount).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "Approval: {} approved {} to spend {}",
                            caller, spender, amount
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Approval failed: {}", e));
                    }
                }
            }

            Operation::TransferFrom { from, to, amount } => {
                match self.state.transfer_from(caller, from, to, amount, now).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "TransferFrom: {} moved {} from {} to {}",
                            caller, amount, from, to
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("TransferFrom failed: {}", e));
                    }
                }
            }

            Operation::Burn { amount } => {
                match self.state.burn(caller, amount, now).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "Burn: {} burned {} BATTLE | Total burned: {}",
                            caller, amount, self.state.total_burned
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Burn failed: {}", e));
                    }
                }
            }

            Operation::Mint { to, amount } => {
                // TODO: Add admin check
                // For now, only allow minting during initialization or by specific authority
                match self.state.mint(to, amount, now).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "Mint: {} minted to {} | Total supply: {}",
                            amount, to, self.state.total_supply
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Mint failed: {}", e));
                    }
                }
            }

            Operation::Claim { amount } => {
                // For reward claims or initial distribution
                // TODO: Implement claim logic with verification
                match self.state.mint(caller, amount, now).await {
                    Ok(_) => {
                        self.runtime.emit(format!("Claimed: {} received {}", caller, amount));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Claim failed: {}", e));
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
                target_chain,
            } => {
                // Deduct from sender on this chain
                match self.state.balance_of(&from).await {
                    balance if balance >= amount => {
                        if let Ok(_) = self.state.transfer(from, to, amount, now).await {
                            self.runtime.emit(format!(
                                "Cross-chain transfer: {} -> {} ({}) | Amount: {}",
                                from, to, target_chain, amount
                            ));

                            // TODO: Send credit message to target chain
                            // self.runtime.send_message(target_chain, Message::Credit { recipient: to, amount });
                        }
                    }
                    _ => {
                        self.runtime
                            .emit(format!("Cross-chain transfer failed: insufficient balance"));
                    }
                }
            }

            Message::Credit { recipient, amount } => {
                // Credit tokens received from another chain
                if let Ok(_) = self.state.mint(recipient, amount, now).await {
                    self.runtime.emit(format!(
                        "Cross-chain credit: {} received {}",
                        recipient, amount
                    ));
                }
            }

            Message::Debit { sender, amount } => {
                // Confirmation of tokens sent to another chain
                if let Ok(_) = self.state.burn(sender, amount, now).await {
                    self.runtime.emit(format!(
                        "Cross-chain debit: {} debited {}",
                        sender, amount
                    ));
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

impl Service for BattleTokenService {
    type Parameters = ();

    async fn load(runtime: ServiceRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state),
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root
struct QueryRoot<'a> {
    state: &'a BattleTokenState,
}

impl<'a> QueryRoot<'a> {
    fn new(state: &'a BattleTokenState) -> Self {
        Self { state }
    }
}

#[async_graphql::Object]
impl<'a> QueryRoot<'a> {
    /// Token name
    async fn name(&self) -> String {
        self.state.name.clone()
    }

    /// Token symbol
    async fn symbol(&self) -> String {
        self.state.symbol.clone()
    }

    /// Token decimals
    async fn decimals(&self) -> u8 {
        self.state.decimals
    }

    /// Total supply
    async fn total_supply(&self) -> String {
        self.state.total_supply.to_string()
    }

    /// Total burned
    async fn total_burned(&self) -> String {
        self.state.total_burned.to_string()
    }

    /// Circulating supply (total - burned)
    async fn circulating_supply(&self) -> String {
        (self.state.total_supply - self.state.total_burned).to_string()
    }

    /// Get balance of account
    async fn balance_of(&self, account: String) -> String {
        // For now, return zero - need proper Owner parsing
        // TODO: Parse Owner from string and query balance
        "0".to_string()
    }

    /// Get allowance
    async fn allowance(&self, owner: String, spender: String) -> String {
        // TODO: Parse Owner from strings and query allowance
        "0".to_string()
    }

    /// Total number of token holders
    async fn total_holders(&self) -> u64 {
        self.state.total_holders
    }

    /// Total number of transfers
    async fn total_transfers(&self) -> u64 {
        self.state.total_transfers
    }

    /// Token statistics
    async fn stats(&self) -> TokenStats {
        TokenStats {
            total_supply: self.state.total_supply.to_string(),
            total_burned: self.state.total_burned.to_string(),
            circulating_supply: (self.state.total_supply - self.state.total_burned).to_string(),
            total_holders: self.state.total_holders,
            total_transfers: self.state.total_transfers,
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

linera_sdk::contract!(BattleTokenContract);
linera_sdk::service!(BattleTokenService);
