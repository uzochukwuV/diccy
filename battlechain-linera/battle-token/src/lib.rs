use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{AccountOwner, Amount, ChainId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Type alias for consistency
type Owner = AccountOwner;

/// BATTLE Token Application ABI
pub struct BattleTokenAbi;

impl ContractAbi for BattleTokenAbi {
    type Operation = Operation;
    type Response = TokenResponse;
}

impl ServiceAbi for BattleTokenAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Token Response enum for inter-contract call returns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenResponse {
    /// Operation completed successfully
    Ok,
    /// Balance query response
    Balance(Amount),
    /// Transfer completed successfully
    TransferSuccess,
    /// Allowance query response
    Allowance(Amount),
}

/// Token Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Query balance of an account (for inter-contract calls)
    Balance { owner: Owner },

    /// Query allowance between owner and spender
    GetAllowance { owner: Owner, spender: Owner },

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
