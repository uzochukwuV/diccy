/*!
# BattleChain Shared Events

Centralized event definitions used across all BattleChain microchains.

## Purpose
This crate consolidates all cross-chain event types to eliminate code duplication
and ensure consistency across battle-chain, prediction-chain, and registry-chain.

## Events
- `BattleEvent`: Events emitted by battle-chain for cross-chain notifications
- `CombatStats`: Combat statistics for battle participants

## Usage
```rust
use battlechain_shared_events::{BattleEvent, CombatStats};

// Subscribe to battle events
runtime.subscribe(battle_chain_id, battle_app_id, "battle_events");

// Handle events
match event {
    BattleEvent::BattleStarted { .. } => {
        // Create prediction market
    }
    BattleEvent::BattleCompleted { .. } => {
        // Settle market, update registry
    }
}
```
*/

use serde::{Deserialize, Serialize};
use linera_sdk::linera_base_types::{Amount, ChainId};

/// Events emitted by battle-chain for cross-chain notifications
///
/// These events are published to the "battle_events" stream and can be subscribed to by:
/// - prediction-chain (to create/settle markets)
/// - registry-chain (to update character statistics)
/// - Any other chain that wants to track battles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BattleEvent {
    /// Battle started - emitted when battle begins
    ///
    /// Prediction market should:
    /// - Create a new market for this battle
    /// - Open betting
    BattleStarted {
        /// Chain ID where the battle is happening
        battle_chain: ChainId,
        /// Player 1's chain ID
        player1_chain: ChainId,
        /// Player 2's chain ID
        player2_chain: ChainId,
        /// Total stake (sum of both players' stakes)
        total_stake: Amount,
    },

    /// Battle completed - emitted when battle finishes
    ///
    /// Prediction market should:
    /// - Close betting
    /// - Settle the market with winner
    /// - Distribute winnings
    ///
    /// Registry should:
    /// - Update character statistics
    /// - Update ELO ratings
    /// - Update leaderboard
    BattleCompleted {
        /// Chain ID where the battle happened
        battle_chain: ChainId,
        /// Player 1's chain ID
        player1_chain: ChainId,
        /// Player 2's chain ID
        player2_chain: ChainId,
        /// Winner's chain ID
        winner_chain: ChainId,
        /// Loser's chain ID
        loser_chain: ChainId,
        /// Total stake (winner receives this minus platform fee)
        stake: Amount,
        /// Number of rounds played
        rounds_played: u8,
        /// Combat statistics for player 1
        player1_stats: CombatStats,
        /// Combat statistics for player 2
        player2_stats: CombatStats,
    },
}

/// Combat statistics for a battle participant
///
/// Tracks detailed combat metrics for:
/// - Registry chain (character statistics)
/// - Leaderboard rankings
/// - Achievement tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatStats {
    /// Total damage dealt to opponent
    pub damage_dealt: u64,
    /// Total damage taken from opponent
    pub damage_taken: u64,
    /// Number of critical hits landed
    pub crits: u64,
    /// Number of successful dodges
    pub dodges: u64,
    /// Highest single damage hit (for records)
    pub highest_crit: u64,
}

impl CombatStats {
    /// Create empty combat stats
    pub fn new() -> Self {
        Self {
            damage_dealt: 0,
            damage_taken: 0,
            crits: 0,
            dodges: 0,
            highest_crit: 0,
        }
    }

    /// Create combat stats from battle actions
    pub fn from_actions(
        damage_dealt: u64,
        damage_taken: u64,
        crits: u64,
        dodges: u64,
        highest_crit: u64,
    ) -> Self {
        Self {
            damage_dealt,
            damage_taken,
            crits,
            dodges,
            highest_crit,
        }
    }
}

impl Default for CombatStats {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_combat_stats_creation() {
        let stats = CombatStats::new();
        assert_eq!(stats.damage_dealt, 0);
        assert_eq!(stats.damage_taken, 0);
        assert_eq!(stats.crits, 0);
        assert_eq!(stats.dodges, 0);
        assert_eq!(stats.highest_crit, 0);
    }

    #[test]
    fn test_combat_stats_from_actions() {
        let stats = CombatStats::from_actions(100, 50, 3, 2, 45);
        assert_eq!(stats.damage_dealt, 100);
        assert_eq!(stats.damage_taken, 50);
        assert_eq!(stats.crits, 3);
        assert_eq!(stats.dodges, 2);
        assert_eq!(stats.highest_crit, 45);
    }

    #[test]
    fn test_combat_stats_default() {
        let stats = CombatStats::default();
        assert_eq!(stats.damage_dealt, 0);
    }
}
