use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use battlechain_shared_types::Owner;
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Prediction Market Chain Application ABI
pub struct PredictionAbi;

impl ContractAbi for PredictionAbi {
    type Operation = Operation;
    type Response = Result<(), PredictionError>;
}

impl ServiceAbi for PredictionAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Prediction market status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Open,       // Accepting bets
    Closed,     // Battle started, no more bets
    Settled,    // Battle ended, winnings distributed
    Cancelled,  // Battle cancelled, refunds issued
}

/// Which player a bet is on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum BetSide {
    Player1,
    Player2,
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
    pub fn calculate_winnings(&self, bet: &Bet) -> Amount {
        if self.winner != Some(bet.side) {
            return Amount::ZERO;
        }

        // Winnings = bet_amount * odds / 10000
        // Since Amount doesn't support direct arithmetic, we work with smaller multipliers
        // For simplicity, just return the bet amount for now
        // TODO: Implement proper fixed-point arithmetic for odds-based payouts

        // For now, return 2x the bet for winners (simpler than complex odds calculation)
        bet.amount.saturating_add(bet.amount)
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

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
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

    /// Update configuration
    UpdateConfig {
        platform_fee_bps: u16,
        treasury_owner: Owner,
    },
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

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for PredictionError {
    fn from(err: linera_sdk::views::ViewError) -> Self {
        PredictionError::ViewError(format!("{:?}", err))
    }
}

/// Prediction Market Contract
pub struct PredictionContract {
    state: PredictionState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(PredictionContract);

impl WithContractAbi for PredictionContract {
    type Abi = PredictionAbi;
}

impl Contract for PredictionContract {
    type Message = Message;
    type Parameters = u16; // Platform fee basis points
    type InstantiationArgument = Owner; // Treasury owner
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = PredictionState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, treasury_owner: Owner) {
        let platform_fee_bps = self.runtime.application_parameters();
        let now = self.runtime.system_time();

        self.state.next_market_id.set(0);
        self.state.total_markets.set(0);
        self.state.total_bets.set(0);
        self.state.total_volume.set(Amount::ZERO);
        self.state.platform_fee_bps.set(platform_fee_bps);
        self.state.treasury_owner.set(Some(treasury_owner));
        self.state.created_at.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::CreateMarket { battle_chain, player1_chain, player2_chain } => {
                let market_id = *self.state.next_market_id.get();
                self.state.next_market_id.set(market_id + 1);

                let now = self.runtime.system_time();
                let platform_fee_bps = *self.state.platform_fee_bps.get();

                let market = Market {
                    market_id,
                    battle_chain,
                    player1_chain,
                    player2_chain,
                    status: MarketStatus::Open,
                    total_player1_bets: Amount::ZERO,
                    total_player2_bets: Amount::ZERO,
                    total_pool: Amount::ZERO,
                    created_at: now,
                    closed_at: None,
                    settled_at: None,
                    winner: None,
                    platform_fee_bps,
                };

                self.state.markets.insert(&market_id, market)
                    .expect("Failed to create market");

                self.state.battle_to_market.insert(&battle_chain, market_id)
                    .expect("Failed to map battle to market");

                let total = *self.state.total_markets.get();
                self.state.total_markets.set(total + 1);
            }

            Operation::PlaceBet { market_id, side, amount, bettor_chain, bettor } => {
                let mut market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                if market.status != MarketStatus::Open {
                    return Err(PredictionError::MarketNotOpen.into());
                }

                if amount.is_zero() {
                    return Err(PredictionError::BetTooSmall.into());
                }

                // Calculate odds at placement
                let odds = market.calculate_odds(side);

                // Update market pools
                match side {
                    BetSide::Player1 => {
                        market.total_player1_bets = market.total_player1_bets.saturating_add(amount);
                    }
                    BetSide::Player2 => {
                        market.total_player2_bets = market.total_player2_bets.saturating_add(amount);
                    }
                }
                market.total_pool = market.total_pool.saturating_add(amount);

                // Store updated market
                self.state.markets.insert(&market_id, market)?;

                // Create bet record
                let bet = Bet {
                    bettor,
                    bettor_chain,
                    side,
                    amount,
                    placed_at: self.runtime.system_time(),
                    odds_at_placement: odds,
                };

                self.state.bets.insert(&(market_id, bettor_chain), bet)?;

                // Update stats
                let total_bets = *self.state.total_bets.get();
                self.state.total_bets.set(total_bets + 1);

                let total_volume = *self.state.total_volume.get();
                self.state.total_volume.set(total_volume.saturating_add(amount));
            }

            Operation::CloseMarket { market_id } => {
                let mut market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                if market.status != MarketStatus::Open {
                    return Err(PredictionError::MarketNotOpen.into());
                }

                market.status = MarketStatus::Closed;
                market.closed_at = Some(self.runtime.system_time());

                self.state.markets.insert(&market_id, market)?;
            }

            Operation::SettleMarket { market_id, winner } => {
                let mut market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                if market.status != MarketStatus::Closed {
                    return Err(PredictionError::MarketNotClosed.into());
                }

                market.status = MarketStatus::Settled;
                market.settled_at = Some(self.runtime.system_time());
                market.winner = Some(winner);

                self.state.markets.insert(&market_id, market)?;
            }

            Operation::CancelMarket { market_id } => {
                let mut market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                market.status = MarketStatus::Cancelled;

                self.state.markets.insert(&market_id, market)?;

                // TODO: Issue refunds to all bettors
            }

            Operation::ClaimWinnings { market_id, bettor_chain } => {
                let market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                if market.status != MarketStatus::Settled {
                    return Err(PredictionError::MarketNotClosed.into());
                }

                let bet = self.state.bets.get(&(market_id, bettor_chain)).await?
                    .ok_or(PredictionError::BetNotFound)?;

                let winnings = market.calculate_winnings(&bet);

                if winnings.is_zero() {
                    return Err(PredictionError::NoWinnings.into());
                }

                // Send winnings payout message
                self.runtime
                    .prepare_message(Message::WinningsPayout {
                        market_id,
                        bettor: bet.bettor,
                        amount: winnings,
                    })
                    .with_authentication()
                    .send_to(bettor_chain);

                // Remove bet after claiming (prevent double-claim)
                self.state.bets.remove(&(market_id, bettor_chain))?;
            }

            Operation::UpdateConfig { platform_fee_bps, treasury_owner } => {
                self.state.platform_fee_bps.set(platform_fee_bps);
                self.state.treasury_owner.set(Some(treasury_owner));
            }
        }

        Ok(())
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleStarted { battle_chain } => {
                // Find market for this battle and close it
                if let Some(market_id) = self.state.battle_to_market.get(&battle_chain).await.ok().flatten() {
                    let _ = self.execute_operation(Operation::CloseMarket { market_id }).await;
                }
            }

            Message::BattleEnded { battle_chain, winner_chain } => {
                // Find market and determine winner
                if let Some(market_id) = self.state.battle_to_market.get(&battle_chain).await.ok().flatten() {
                    if let Some(market) = self.state.markets.get(&market_id).await.ok().flatten() {
                        let winner = if winner_chain == market.player1_chain {
                            BetSide::Player1
                        } else {
                            BetSide::Player2
                        };

                        let _ = self.execute_operation(Operation::SettleMarket { market_id, winner }).await;
                    }
                }
            }

            Message::WinningsPayout { .. } => {
                // Handled by recipient chain
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Prediction Service
pub struct PredictionService {
    state: PredictionState,
}

impl WithServiceAbi for PredictionService {
    type Abi = PredictionAbi;
}

impl Service for PredictionService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = PredictionState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state).await,
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root
#[derive(Clone)]
struct QueryRoot {
    total_markets: u64,
    total_bets: u64,
    total_volume: String,
    platform_fee_bps: u16,
}

impl QueryRoot {
    async fn new(state: &PredictionState) -> Self {
        Self {
            total_markets: *state.total_markets.get(),
            total_bets: *state.total_bets.get(),
            total_volume: state.total_volume.get().to_string(),
            platform_fee_bps: *state.platform_fee_bps.get(),
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get total number of prediction markets created
    async fn total_markets(&self) -> i64 {
        self.total_markets as i64
    }

    /// Get total number of bets placed
    async fn total_bets(&self) -> i64 {
        self.total_bets as i64
    }

    /// Get total volume wagered
    async fn total_volume(&self) -> &str {
        &self.total_volume
    }

    /// Get platform fee in basis points
    async fn platform_fee_bps(&self) -> i32 {
        self.platform_fee_bps as i32
    }

    /// Get prediction market stats
    async fn stats(&self) -> PredictionStats {
        PredictionStats {
            total_markets: self.total_markets,
            total_bets: self.total_bets,
            total_volume: self.total_volume.clone(),
            platform_fee_bps: self.platform_fee_bps,
        }
    }
}

#[derive(SimpleObject)]
struct PredictionStats {
    total_markets: u64,
    total_bets: u64,
    total_volume: String,
    platform_fee_bps: u16,
}
