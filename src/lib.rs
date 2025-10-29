
// use serde::{Deserialize, Serialize};

// pub struct EsportAbi;

// impl ContractAbi for EsportAbi {
//     type Operation = Operation;
//     type Response = ();
// }

// impl ServiceAbi for EsportAbi {
//     type Query = Request;
//     type QueryResponse = Response;
// }

// #[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
// pub enum Operation {
//     Increment { value: u64 },
// }


use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{ContractAbi, ServiceAbi},
};
use async_graphql::{Request, Response, SimpleObject};
use linera_sdk::{linera_base_types::AccountOwner};
use serde::{Deserialize, Serialize};

pub struct DiceAbi;

/// Operations the contract accepts.
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    /// Register or refresh a player profile. This is a helpful convenience to set initial stats.
    RegisterPlayer { owner: AccountOwner },

    /// Start a match with two players and a number of rounds.
    StartMatch {
        players: [AccountOwner; 2],
        rounds: u8,
    },

    /// Settle a match by providing revealed seeds/hits (produced off-chain by deterministic RNG).
    /// One submitted SettleMatch operation will verify the seeds / hits and finalize the match.
    SettleMatch {
        match_id: u64,
        /// hex or raw bytes encoded seed for player 0 (revealed)
        seed0: Vec<u8>,
        /// seed for player 1 (revealed)
        seed1: Vec<u8>,
        /// hits arrays calculated by the frontend (should be reproducible on-chain from seeds)
        hits0: Vec<u32>,
        hits1: Vec<u32>,
    },
}

impl ContractAbi for DiceAbi {
    type Operation = Operation;
    type Response = SettleOutcome;
}

impl ServiceAbi for DiceAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Outcome after a settlement attempt.
#[derive(Debug, Deserialize, Serialize, SimpleObject)]
pub struct SettleOutcome {
    /// Whether settlement succeeded.
    pub success: bool,
    /// If success is true, winner owner is set.
    pub winner: Option<AccountOwner>,
    /// Message for human-readable explanation or error.
    pub message: String,
}
