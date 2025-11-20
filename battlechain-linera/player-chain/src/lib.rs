// Module declarations
mod state;
mod contract;
mod service;

// Re-exports
pub use state::PlayerChainState;
pub use contract::PlayerChainContract;
pub use service::PlayerChainService;

use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{Amount, ApplicationId, ChainId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Player Chain Application ABI
pub struct PlayerChainAbi;

impl ContractAbi for PlayerChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for PlayerChainAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Operations on Player Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Initialize BATTLE token app reference
    Initialize { battle_token_app: ApplicationId },

    /// Create new character
    CreateCharacter { nft_id: String, class: CharacterClass },

    /// Join a battle
    JoinBattle { battle_chain: ChainId, character_nft: String, stake: Amount },

    /// Update stats after battle
    UpdateAfterBattle { battle_chain: ChainId, won: bool, reward: Amount },

    /// SECURITY: Pause contract (admin only)
    Pause,

    /// SECURITY: Unpause contract (admin only)
    Unpause,

    /// SECURITY: Transfer admin rights (admin only)
    TransferAdmin { new_admin: Owner },
}

/// Cross-chain messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle invitation
    BattleInvite {
        battle_chain: ChainId,
        stake_required: Amount,
    },

    /// Battle result notification (from battle chain)
    BattleResult {
        winner: Owner,
        loser: Owner,
        winner_payout: Amount,
        rounds_played: u8,
    },

    /// Matchmaking request to lock stake
    LockStakeRequest {
        matchmaking_chain: ChainId,
        battle_chain: ChainId,
        stake_amount: Amount,
    },
}

/// Player Chain Errors
#[derive(Debug, Error)]
pub enum PlayerChainError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: Amount, required: Amount },

    #[error("Battle not found")]
    BattleNotFound,

    #[error("Character not found: {0}")]
    CharacterNotFound(String),

    #[error("Math overflow")]
    MathOverflow,

    #[error("Unauthorized message sender: {0:?}")]
    UnauthorizedSender(ChainId),

    #[error("Contract is paused")]
    ContractPaused,

    #[error("Not authorized: only admin can perform this operation")]
    NotAuthorized,

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for PlayerChainError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        PlayerChainError::ViewError(format!("{:?}", err))
    }
}
