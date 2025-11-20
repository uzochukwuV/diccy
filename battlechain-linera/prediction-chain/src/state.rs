use battlechain_shared_types::Owner;
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, ViewStorageContext},
};
use serde::{Deserialize, Serialize};

use crate::{BattleTokenAbi, BetSide, PredictionError};

/// Prediction market status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Open,       // Accepting bets
    Closed,     // Battle started, no more bets
    Settled,    // Battle ended, winnings distributed
    Cancelled,  // Battle cancelled, refunds issued
}

/// A bet placed by a spectator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub bettor: Owner,
    pub bettor_chain: ChainId,
    pub side: BetSide,
    pub amount: Amount,
    pub placed_at: Timestamp,
    pub odds_at_placement: u64, // basis points (10000 = 1.0x)
}

/// Prediction market for a battle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub market_id: u64,
    pub battle_chain: ChainId,
    pub player1_chain: ChainId,
    pub player2_chain: ChainId,
    pub status: MarketStatus,

    // Betting pools
    pub total_player1_bets: Amount,
    pub total_player2_bets: Amount,
    pub total_pool: Amount,

    // Market metadata
    pub created_at: Timestamp,
    pub closed_at: Option<Timestamp>,
    pub settled_at: Option<Timestamp>,
    pub winner: Option<BetSide>,

    // Platform fee
    pub platform_fee_bps: u16, // basis points (100 = 1%)
}

impl Market {
    /// Calculate current odds for a bet side (in basis points, 10000 = 1.0x)
    pub fn calculate_odds(&self, side: BetSide) -> u64 {
        if self.total_pool.is_zero() {
            return 20000; // 2.0x default odds when no bets placed
        }

        let side_pool = match side {
            BetSide::Player1 => self.total_player1_bets,
            BetSide::Player2 => self.total_player2_bets,
        };

        if side_pool.is_zero() {
            return 50000; // 5.0x if no one has bet on this side yet
        }

        // Odds = total_pool / side_pool
        // Convert to u128 for calculation to avoid overflow
        let total = self.total_pool.try_into().unwrap_or(0u128);
        let side = side_pool.try_into().unwrap_or(1u128);

        if side == 0 {
            return 50000;
        }

        // Calculate with basis points: (total * 10000) / side
        let odds = (total * 10000) / side;
        odds.min(100000) as u64 // Cap at 10x odds
    }

    /// Calculate winnings for a bet
    /// Formula: (bet_amount / total_winning_side_bets) * total_pool * (1 - platform_fee)
    pub fn calculate_winnings(&self, bet: &Bet) -> Amount {
        if self.winner != Some(bet.side) {
            return Amount::ZERO;
        }

        // Get total bets for the winning side
        let winning_side_total = match bet.side {
            BetSide::Player1 => self.total_player1_bets,
            BetSide::Player2 => self.total_player2_bets,
        };

        // If no bets on winning side (shouldn't happen), return zero
        if winning_side_total.is_zero() {
            return Amount::ZERO;
        }

        // Convert to u128 for fixed-point arithmetic
        let total_pool_u128: u128 = self.total_pool.try_into().unwrap_or(0);
        let bet_amount_u128: u128 = bet.amount.try_into().unwrap_or(0);
        let winning_total_u128: u128 = winning_side_total.try_into().unwrap_or(1);

        // Calculate platform fee: (total_pool * fee_bps) / 10000
        let fee_amount = (total_pool_u128 * self.platform_fee_bps as u128) / 10000;
        let pool_after_fee = total_pool_u128.saturating_sub(fee_amount);

        // Calculate proportional winnings: (bet_amount * pool_after_fee) / winning_total
        let winnings_u128 = (bet_amount_u128 * pool_after_fee) / winning_total_u128;

        Amount::from_attos(winnings_u128)
    }
}

/// Prediction Market State
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PredictionState {
    /// Markets indexed by market ID
    pub markets: MapView<u64, Market>,

    /// Bets indexed by (market_id, bettor_chain) -> Bet
    pub bets: MapView<(u64, ChainId), Bet>,

    /// Battle chain to market ID mapping
    pub battle_to_market: MapView<ChainId, u64>,

    /// Next market ID
    pub next_market_id: RegisterView<u64>,

    /// Total markets created
    pub total_markets: RegisterView<u64>,

    /// Total bets placed
    pub total_bets: RegisterView<u64>,

    /// Total volume wagered
    pub total_volume: RegisterView<Amount>,

    /// Platform fee basis points (default 100 = 1%)
    pub platform_fee_bps: RegisterView<u16>,

    /// Treasury owner for platform fees
    pub treasury_owner: RegisterView<Option<Owner>>,

    /// Battle token application ID (for transferring winnings)
    pub battle_token_app: RegisterView<Option<ApplicationId<BattleTokenAbi>>>,

    /// SECURITY: Track known battle chains (for message authentication)
    pub known_battle_chains: MapView<ChainId, bool>,

    /// SECURITY: Admin owner (for pause functionality)
    pub admin: RegisterView<Option<Owner>>,

    /// SECURITY: Paused state
    pub paused: RegisterView<bool>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}
