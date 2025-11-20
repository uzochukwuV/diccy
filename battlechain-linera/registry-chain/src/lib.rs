// Module declarations
mod state;
mod contract;
mod service;

// Re-exports
pub use state::{BattleRecord, CharacterStats, RegistryState};
pub use contract::RegistryContract;
pub use service::RegistryService;

use battlechain_shared_events::CombatStats;
use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{Amount, ChainId},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Registry Chain Application ABI
pub struct RegistryAbi;

impl ContractAbi for RegistryAbi {
    type Operation = Operation;
    type Response = Result<(), RegistryError>;
}

impl ServiceAbi for RegistryAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
}

/// Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Register a new character in the global registry
    RegisterCharacter {
        character_id: String,
        nft_id: String,
        owner: Owner,
        owner_chain: ChainId,
        class: CharacterClass,
        level: u16,
    },

    /// Update character statistics after a battle
    UpdateCharacterStats {
        character_id: String,
        won: bool,
        damage_dealt: u64,
        damage_taken: u64,
        crits: u64,
        dodges: u64,
        highest_crit: u64,
        earnings: Amount,
        stake: Amount,
        opponent_elo: u64,
    },

    /// Record a battle in the global history
    RecordBattle {
        battle_chain: ChainId,
        player1_id: String,
        player2_id: String,
        winner_id: String,
        stake: Amount,
        rounds_played: u8,
    },

    /// Update character level
    UpdateCharacterLevel {
        character_id: String,
        new_level: u16,
    },

    /// Mark character as defeated (no lives remaining)
    MarkCharacterDefeated {
        character_id: String,
    },

    /// Subscribe to battle events from a battle chain
    SubscribeToBattleEvents {
        battle_chain_id: ChainId,
        battle_app_id: linera_sdk::linera_base_types::ApplicationId,
    },

    /// SECURITY: Pause contract (admin only)
    Pause,

    /// SECURITY: Unpause contract (admin only)
    Unpause,

    /// SECURITY: Transfer admin rights (admin only)
    TransferAdmin { new_admin: Owner },
}

/// Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Battle completed - update stats
    BattleCompleted {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
        winner_chain: ChainId,
        stake: Amount,
        rounds_played: u8,
        // Combat statistics (now using shared struct)
        player1_stats: CombatStats,
        player2_stats: CombatStats,
    },

    /// Character registered
    CharacterRegistered {
        character_id: String,
        owner_chain: ChainId,
    },
}

/// Errors
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum RegistryError {
    #[error("Character not found: {0}")]
    CharacterNotFound(String),

    #[error("Character already registered")]
    CharacterAlreadyRegistered,

    #[error("Battle not found")]
    BattleNotFound,

    #[error("Unauthorized message sender: {0:?}")]
    UnauthorizedSender(ChainId),

    #[error("Contract is paused")]
    ContractPaused,

    #[error("Not authorized: only admin can perform this operation")]
    NotAuthorized,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for RegistryError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        RegistryError::ViewError(format!("{:?}", err))
    }
}
