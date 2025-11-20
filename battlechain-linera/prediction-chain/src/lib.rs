use battlechain_shared_types::Owner;
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{AccountOwner, Amount, ChainId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Import battle-token ABI and types for inter-contract calls
use battle_token::{BattleTokenAbi, Operation as BattleTokenOperation, TokenResponse};

/// Prediction Market Chain Application ABI
pub struct PredictionAbi;

impl ContractAbi for PredictionAbi {
    type Operation = Operation;
    type Response = Result<(), PredictionError>;
}

impl ServiceAbi for PredictionAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Which player a bet is on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum BetSide {
    Player1,
    Player2,
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Create prediction market for a battle
    CreateMarket {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
    },

    /// Place a bet on a battle outcome
    PlaceBet {
        market_id: u64,
        side: BetSide,
        amount: Amount,
        bettor_chain: ChainId,
        bettor: Owner,
    },

    /// Close market (no more bets allowed)
    CloseMarket { market_id: u64 },

    /// Settle market with battle result
    SettleMarket {
        market_id: u64,
        winner: BetSide,
    },

    /// Cancel market and issue refunds
    CancelMarket { market_id: u64 },

    /// Claim winnings (called by bettor)
    ClaimWinnings {
        market_id: u64,
        bettor_chain: ChainId,
    },

    /// Claim refund for cancelled market (called by bettor)
    ClaimRefund {
        market_id: u64,
        bettor_chain: ChainId,
    },

    /// Update configuration
    UpdateConfig {
        platform_fee_bps: u16,
        treasury_owner: Owner,
    },

    /// Subscribe to battle events from a battle chain
    SubscribeToBattleEvents {
        battle_chain_id: ChainId,
        battle_app_id: linera_sdk::linera_base_types::ApplicationId,
    },

    /// SECURITY: Pause contract (admin only)
    Pause { reason: String },

    /// SECURITY: Unpause contract (admin only)
    Unpause,

    /// SECURITY: Transfer admin rights (admin only)
    TransferAdmin { new_admin: Owner },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle started - close betting
    BattleStarted { battle_chain: ChainId },

    /// Battle ended - settle market
    BattleEnded {
        battle_chain: ChainId,
        winner_chain: ChainId, // Winner's player chain
    },

    /// Winnings payout notification
    WinningsPayout {
        market_id: u64,
        bettor: Owner,
        amount: Amount,
    },
}

/// Errors
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum PredictionError {
    #[error("Market not found")]
    MarketNotFound,

    #[error("Market is not open for betting")]
    MarketNotOpen,

    #[error("Market is not closed yet")]
    MarketNotClosed,

    #[error("Market is already settled")]
    MarketAlreadySettled,

    #[error("Bet amount too small")]
    BetTooSmall,

    #[error("Bet not found")]
    BetNotFound,

    #[error("No winnings to claim")]
    NoWinnings,

    #[error("Market is not cancelled")]
    MarketNotCancelled,

    #[error("No refund available")]
    NoRefund,

    #[error("Unauthorized message sender: {0:?}")]
    UnauthorizedSender(ChainId),

    #[error("Contract is paused")]
    ContractPaused,

    #[error("Not authorized: only admin can perform this operation")]
    NotAuthorized,

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for PredictionError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        PredictionError::ViewError(format!("{:?}", err))
    }
}
