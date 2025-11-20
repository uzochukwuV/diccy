// Module declarations
mod state;
mod contract;
mod service;

// Re-exports
pub use state::{MatchmakingState, PendingBattle};
pub use contract::MatchmakingContract;
pub use service::MatchmakingService;

use battlechain_shared_types::{CharacterSnapshot, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Import prediction-chain ABI and operations for inter-contract calls
use prediction_chain::{PredictionAbi, Operation as PredictionOperation};

// Battle chain message types (defined inline to avoid circular dependencies)
/// Battle participant state for initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParticipant {
    pub owner: Owner,
    pub chain: ChainId,
    pub character: CharacterSnapshot,
    pub stake: Amount,
    pub current_hp: u32,
    pub combo_stack: u8,
    pub special_cooldown: u8,
    pub turns_submitted: [Option<TurnSubmission>; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSubmission {
    pub round: u8,
    pub turn: u8,
    pub stance: battlechain_shared_types::Stance,
    pub use_special: bool,
}

impl BattleParticipant {
    pub fn new(owner: Owner, chain: ChainId, character: CharacterSnapshot, stake: Amount) -> Self {
        let current_hp = character.hp_max;
        Self {
            owner,
            chain,
            character,
            stake,
            current_hp,
            combo_stack: 0,
            special_cooldown: 0,
            turns_submitted: [None, None, None],
        }
    }
}

/// Matchmaking Chain Application ABI
pub struct MatchmakingAbi;

impl ContractAbi for MatchmakingAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for MatchmakingAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Player queue entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub player_chain: ChainId,
    pub player_owner: Owner,
    pub character: CharacterSnapshot,
    pub stake: Amount,
    pub joined_at: Timestamp,
}

/// Battle metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMetadata {
    pub player1: ChainId,
    pub player2: ChainId,
    pub stake: Amount,
    pub started_at: Timestamp,
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Join matchmaking queue
    JoinQueue {
        player_chain: ChainId,
        player_owner: Owner,
        character: CharacterSnapshot,
        stake: Amount,
    },

    /// Leave matchmaking queue
    LeaveQueue { player_chain: ChainId },

    /// Create battle offer (matchmaker only - called after 2 players join)
    CreateBattleOffer {
        player1_chain: ChainId,
        player2_chain: ChainId,
    },

    /// Confirm battle offer (player accepts match)
    ConfirmBattleOffer {
        offer_id: u64,
        player_chain: ChainId,
    },

    /// Record battle completion
    RecordBattleCompletion { battle_chain: ChainId },

    /// Update application references
    UpdateReferences {
        battle_app_id: ApplicationId,
        battle_token_app: ApplicationId,
        treasury_owner: Owner,
    },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle offer notification - sent to both players
    BattleOffer {
        offer_id: u64,
        opponent_chain: ChainId,
        stake: Amount,
    },

    /// Battle created - sent after both confirm
    BattleCreated {
        battle_chain: ChainId,
        opponent: ChainId,
    },

    /// Battle completion notification (from battle chain)
    BattleCompleted {
        winner: Owner,
        loser: Owner,
    },
}

/// Errors
#[derive(Debug, Error)]
pub enum MatchmakingError {
    #[error("Insufficient stake: provided {provided}, required {required}")]
    InsufficientStake { provided: Amount, required: Amount },

    #[error("Player not in waiting queue")]
    PlayerNotWaiting,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for MatchmakingError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        MatchmakingError::ViewError(format!("{:?}", err))
    }
}
