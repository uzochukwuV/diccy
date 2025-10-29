

// state.rs
// Minimal persistent state for the Dice realtime match app.
// Keeps player profiles and match records (simple, no NFTs).


use std::vec::Vec;

use async_graphql::SimpleObject;
use linera_sdk::{
    linera_base_types::AccountOwner,
    views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext},
};
use serde::{Deserialize, Serialize};

/// Player profile stored on chain.
#[derive(Clone, Debug, Serialize, Deserialize, SimpleObject)]
pub struct PlayerProfile {
    pub owner: AccountOwner,
    pub xp: u64,
    pub level: u32,
    pub hp_max: u32,
    pub min_damage: u32,
    pub max_damage: u32,
    pub wins: u64,
    pub losses: u64,
}

impl Default for PlayerProfile {
    fn default() -> Self {
        PlayerProfile {
            owner: AccountOwner::Reserved(9),
            xp: 0,
            level: 1,
            hp_max: 30,
            min_damage: 1,
            max_damage: 6,
            wins: 0,
            losses: 0,
        }
    }
}

/// A unique match identifier (simple counter).
pub type MatchId = u64;

/// Minimal match states. Hits arrays are optional and can be provided at settlement.
#[derive(Clone, Debug, Serialize, Deserialize, SimpleObject)]
pub struct MatchRecord {
    pub match_id: MatchId,
    pub players: [AccountOwner; 2],
    pub rounds: u8,
    /// Optional revealed hits per player (length == rounds when present).
    pub hits_player0: Vec<u32>,
    pub hits_player1: Vec<u32>,
    pub winner: Option<AccountOwner>,
    pub settled: bool,
}

impl Default for MatchRecord {
    fn default() -> Self {
        MatchRecord {
            match_id: 0,
            players: [AccountOwner::Reserved(8), AccountOwner::Reserved(7)],
            rounds: 0,
            hits_player0: Vec::new(),
            hits_player1: Vec::new(),
            winner: None,
            settled: false,
        }
    }
}

/// Root application state.
///
/// - `profiles` map: AccountOwner -> PlayerProfile
/// - `matches` map: MatchId -> MatchRecord
/// - `next_match_id`: incremental counter for assigning match ids
#[derive(RootView, SimpleObject)]
#[view(context = ViewStorageContext)]
pub struct DiceState {
    /// Player profiles keyed by owner.
    pub profiles: MapView<AccountOwner, PlayerProfile>,
    /// Matches keyed by id.
    pub matches: MapView<MatchId, MatchRecord>,
    /// Next match id counter.
    pub next_match_id: RegisterView<MatchId>,
}




