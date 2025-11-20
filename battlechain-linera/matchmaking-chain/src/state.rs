use battlechain_shared_types::Owner;
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, ViewStorageContext},
};
use matchmaking_chain::{BattleMetadata, MatchmakingError, QueueEntry};
use prediction_chain::PredictionAbi;
use serde::{Deserialize, Serialize};

/// Matchmaking State - coordinates battle matchmaking
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct MatchmakingState {
    /// Players waiting for matches (player_chain -> queue entry)
    pub waiting_players: MapView<ChainId, QueueEntry>,

    /// Pending battle offers (offer_id -> pending battle)
    pub pending_battles: MapView<u64, PendingBattle>,

    /// Next offer ID
    pub next_offer_id: RegisterView<u64>,

    /// Active battles (battle_chain -> metadata)
    pub active_battles: MapView<ChainId, BattleMetadata>,

    /// Completed battles
    pub completed_battles: RegisterView<Vec<ChainId>>,

    /// Total battles created
    pub total_battles: RegisterView<u64>,

    /// Minimum stake required
    pub min_stake: RegisterView<Amount>,

    /// Battle chain application ID
    pub battle_app_id: RegisterView<Option<ApplicationId>>,

    /// Battle token application ID
    pub battle_token_app: RegisterView<Option<ApplicationId>>,

    /// Prediction market application ID (typed for cross-application calls)
    pub prediction_app_id: RegisterView<Option<ApplicationId<PredictionAbi>>>,

    /// Platform fee basis points (300 = 3%)
    pub platform_fee_bps: RegisterView<u16>,

    /// Treasury owner
    pub treasury_owner: RegisterView<Option<Owner>>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}

/// Battle offer waiting for confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingBattle {
    pub offer_id: u64,
    pub player1: QueueEntry,
    pub player2: QueueEntry,
    pub created_at: Timestamp,
    pub player1_confirmed: bool,
    pub player2_confirmed: bool,
}

impl MatchmakingState {
    /// Add player to waiting queue
    pub fn add_waiting_player(&mut self, entry: QueueEntry) -> Result<(), MatchmakingError> {
        if entry.stake < *self.min_stake.get() {
            return Err(MatchmakingError::InsufficientStake {
                provided: entry.stake,
                required: *self.min_stake.get(),
            });
        }

        let player_chain = entry.player_chain;
        self.waiting_players.insert(&player_chain, entry)?;
        Ok(())
    }

    /// Remove player from waiting queue
    pub async fn remove_waiting_player(&mut self, player_chain: &ChainId) -> Result<QueueEntry, MatchmakingError> {
        let entry = self.waiting_players
            .get(player_chain)
            .await?
            .ok_or(MatchmakingError::PlayerNotWaiting)?;

        self.waiting_players.remove(player_chain)?;
        Ok(entry)
    }

    /// Find a match for a player (simple FIFO matching)
    pub async fn find_match(&self, player_chain: &ChainId) -> Option<(ChainId, QueueEntry)> {
        // Get all waiting player chain IDs
        let waiting_keys = self.waiting_players.indices().await.ok()?;

        // Find first opponent that's not the player themselves
        for opponent_chain in waiting_keys {
            if &opponent_chain != player_chain {
                // Found an opponent! Get their queue entry
                if let Ok(Some(entry)) = self.waiting_players.get(&opponent_chain).await {
                    return Some((opponent_chain, entry));
                }
            }
        }

        // No opponents found
        None
    }
}
