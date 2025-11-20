use battlechain_shared_types::Owner;
use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::AccountOwner,
    views::{RootView, View},
    Contract, ContractRuntime,
};

use crate::{
    BattleTokenAbi, BattleTokenOperation, BetSide, Market, MarketStatus, Message, Operation,
    PredictionAbi, PredictionError, PredictionState,
};

/// Prediction Market Contract
pub struct PredictionContract {
    pub state: PredictionState,
    pub runtime: ContractRuntime<Self>,
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
        self.state.total_volume.set(linera_sdk::linera_base_types::Amount::ZERO);
        self.state.platform_fee_bps.set(platform_fee_bps);
        self.state.treasury_owner.set(Some(treasury_owner.clone()));
        self.state.battle_token_app.set(None);
        self.state.created_at.set(now);

        // SECURITY: Initialize admin as treasury owner
        self.state.admin.set(Some(treasury_owner));
        self.state.paused.set(false);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        // SECURITY: Handle admin operations first (they work even when paused)
        match &operation {
            Operation::Pause { reason } => {
                // Only admin can pause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(PredictionError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(PredictionError::NotAuthorized)?;

                if &caller != admin {
                    return Err(PredictionError::NotAuthorized);
                }

                self.state.paused.set(true);
                log::info!("SECURITY: Contract paused by admin. Reason: {}", reason);
                return Ok(());
            }

            Operation::Unpause => {
                // Only admin can unpause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(PredictionError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(PredictionError::NotAuthorized)?;

                if &caller != admin {
                    return Err(PredictionError::NotAuthorized);
                }

                self.state.paused.set(false);
                log::info!("SECURITY: Contract unpaused by admin");
                return Ok(());
            }

            Operation::TransferAdmin { new_admin } => {
                // Only current admin can transfer
                let caller = self.runtime.authenticated_signer()
                    .ok_or(PredictionError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(PredictionError::NotAuthorized)?.clone();

                if caller != admin {
                    return Err(PredictionError::NotAuthorized);
                }

                self.state.admin.set(Some(*new_admin));
                log::info!("SECURITY: Admin transferred to {:?}", new_admin);
                return Ok(());
            }

            _ => {
                // For all other operations, check if paused
                if *self.state.paused.get() {
                    return Err(PredictionError::ContractPaused);
                }
            }
        }

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
                    total_player1_bets: linera_sdk::linera_base_types::Amount::ZERO,
                    total_player2_bets: linera_sdk::linera_base_types::Amount::ZERO,
                    total_pool: linera_sdk::linera_base_types::Amount::ZERO,
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
                let bet = crate::state::Bet {
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

                log::info!("Market {} cancelled. Bettors can claim refunds via ClaimRefund operation.", market_id);
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

                // Transfer winnings to bettor via battle token application
                if let Some(battle_token_app) = self.state.battle_token_app.get().as_ref() {
                    let bettor_owner = AccountOwner::from(bet.bettor);
                    let transfer_op = BattleTokenOperation::Transfer {
                        to: bettor_owner,
                        amount: winnings,
                    };

                    self.runtime.call_application(
                        true,  // authenticated call
                        battle_token_app.clone(),
                        &transfer_op,
                    );

                    log::info!(
                        "Transferred {} BATTLE tokens to bettor {:?} for market {}",
                        winnings, bet.bettor, market_id
                    );
                } else {
                    log::warn!("Battle token app not configured - cannot transfer winnings");
                    return Err(PredictionError::InvalidConfiguration("Battle token app not set".to_string()));
                }

                // Send winnings payout message for notification
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

            Operation::ClaimRefund { market_id, bettor_chain } => {
                let market = self.state.markets.get(&market_id).await?
                    .ok_or(PredictionError::MarketNotFound)?;

                if market.status != MarketStatus::Cancelled {
                    return Err(PredictionError::MarketNotCancelled.into());
                }

                let bet = self.state.bets.get(&(market_id, bettor_chain)).await?
                    .ok_or(PredictionError::BetNotFound)?;

                // Refund the full bet amount
                let refund_amount = bet.amount;

                if refund_amount.is_zero() {
                    return Err(PredictionError::NoRefund.into());
                }

                // Send refund message
                self.runtime
                    .prepare_message(Message::WinningsPayout {
                        market_id,
                        bettor: bet.bettor,
                        amount: refund_amount,
                    })
                    .with_authentication()
                    .send_to(bettor_chain);

                // Remove bet after claiming (prevent double-claim)
                self.state.bets.remove(&(market_id, bettor_chain))?;

                log::info!(
                    "Refund of {:?} issued to {:?} for cancelled market {}",
                    refund_amount,
                    bettor_chain,
                    market_id
                );
            }

            Operation::UpdateConfig { platform_fee_bps, treasury_owner } => {
                self.state.platform_fee_bps.set(platform_fee_bps);
                self.state.treasury_owner.set(Some(treasury_owner));
            }

            Operation::SubscribeToBattleEvents { battle_chain_id, battle_app_id } => {
                // Subscribe to battle events from the specified battle chain
                self.runtime.subscribe_to_events(
                    battle_chain_id,
                    battle_app_id,
                    "battle_events".into(),
                );

                // SECURITY: Track this battle chain as known for message authentication
                self.state.known_battle_chains.insert(&battle_chain_id, true)?;

                log::info!(
                    "Subscribed to battle events from chain {:?}, app {:?}",
                    battle_chain_id,
                    battle_app_id
                );
            }

            // Admin operations already handled above
            Operation::Pause { .. } | Operation::Unpause | Operation::TransferAdmin { .. } => {
                unreachable!("Admin operations handled at start of execute_operation")
            }
        }

        Ok(())
    }

    async fn execute_message(&mut self, message: Message) {
        // NOTE: This handler processes both direct messages and subscribed events
        // Events from battle-chain arrive here after subscription via SubscribeToBattleEvents
        match message {
            Message::BattleStarted { battle_chain } => {
                // SECURITY: Validate message sender is a known battle chain
                let sender_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        log::error!("BattleStarted message has no origin chain");
                        return;
                    }
                };

                // Check if this is a known battle chain
                match self.state.known_battle_chains.get(&sender_chain).await {
                    Ok(Some(true)) => {
                        // Valid battle chain, continue processing
                    }
                    _ => {
                        log::error!(
                            "SECURITY: Unauthorized BattleStarted from unknown chain: {:?}",
                            sender_chain
                        );
                        return; // Reject message from unknown battle chain
                    }
                }

                // Event: BattleEvent::BattleStarted received
                // Close betting for this battle's market
                if let Some(market_id) = self.state.battle_to_market.get(&battle_chain).await.ok().flatten() {
                    let _ = self.execute_operation(Operation::CloseMarket { market_id }).await;
                    log::info!("Closed market {} for battle {:?}", market_id, battle_chain);
                }
            }

            Message::BattleEnded { battle_chain, winner_chain } => {
                // SECURITY: Validate message sender is a known battle chain
                let sender_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        log::error!("BattleEnded message has no origin chain");
                        return;
                    }
                };

                // Check if this is a known battle chain
                match self.state.known_battle_chains.get(&sender_chain).await {
                    Ok(Some(true)) => {
                        // Valid battle chain, continue processing
                    }
                    _ => {
                        log::error!(
                            "SECURITY: Unauthorized BattleEnded from unknown chain: {:?}",
                            sender_chain
                        );
                        return; // Reject message from unknown battle chain
                    }
                }

                // Event: BattleEvent::BattleCompleted received
                // Settle the market with the battle result
                if let Some(market_id) = self.state.battle_to_market.get(&battle_chain).await.ok().flatten() {
                    if let Some(market) = self.state.markets.get(&market_id).await.ok().flatten() {
                        let winner = if winner_chain == market.player1_chain {
                            BetSide::Player1
                        } else {
                            BetSide::Player2
                        };

                        let _ = self.execute_operation(Operation::SettleMarket { market_id, winner }).await;
                        log::info!("Settled market {} for battle {:?}, winner: {:?}", market_id, battle_chain, winner);
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
