#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use battle_chain::{
    BattleChainAbi, BattleError, BattleParticipant, BattleParameters,
    Message, Operation, RoundResult, TurnSubmission,
};
use battle_token::{BattleTokenAbi, Operation as BattleTokenOperation, TokenResponse};
use battlechain_shared_events::{BattleEvent, CombatStats};
use battlechain_shared_types::Owner;
use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::{Amount, Timestamp},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use self::state::{BattleState, BattleStatus};

/// Battle Contract
pub struct BattleContract {
    pub state: BattleState,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(BattleContract);

impl WithContractAbi for BattleContract {
    type Abi = BattleChainAbi;
}

/// Calculate combat statistics for a player from round results
fn calculate_combat_stats(
    round_results: &[RoundResult],
    player_owner: &Owner,
) -> (u64, u64, u64, u64, u64) {
    let mut damage_dealt = 0u64;
    let mut damage_taken = 0u64;
    let mut crits = 0u64;
    let mut dodges = 0u64;
    let mut highest_crit = 0u64;

    for round in round_results {
        // Check all actions in the round
        for actions in [&round.player1_actions, &round.player2_actions] {
            for action in actions {
                // Count stats where this player is the attacker
                if &action.attacker == player_owner {
                    if !action.was_dodged && !action.was_countered {
                        damage_dealt += action.damage as u64;
                    }
                    if action.was_crit {
                        crits += 1;
                        if action.damage as u64 > highest_crit {
                            highest_crit = action.damage as u64;
                        }
                    }
                }
                // Count stats where this player is the defender
                else if &action.defender == player_owner {
                    if !action.was_dodged && !action.was_countered {
                        damage_taken += action.damage as u64;
                    }
                    if action.was_dodged {
                        dodges += 1;
                    }
                }
            }
        }
    }

    (damage_dealt, damage_taken, crits, dodges, highest_crit)
}

/// Validation functions for security

/// Validate stake amount
fn validate_stake(amount: Amount) -> Result<(), BattleError> {
    const MIN_STAKE: u128 = 1_000_000; // 0.001 BATTLE tokens (1e6 attos)
    const MAX_STAKE: u128 = 1_000_000_000_000_000_000; // 1000 BATTLE tokens

    let attos: u128 = amount.try_into().unwrap_or(0);

    if attos < MIN_STAKE {
        return Err(BattleError::InvalidStake(
            format!("Stake too low: {} (minimum {})", attos, MIN_STAKE)
        ));
    }

    if attos > MAX_STAKE {
        return Err(BattleError::InvalidStake(
            format!("Stake too high: {} (maximum {})", attos, MAX_STAKE)
        ));
    }

    Ok(())
}

/// Validate platform fee basis points
fn validate_platform_fee(fee_bps: u16) -> Result<(), BattleError> {
    if fee_bps > 10000 {
        return Err(BattleError::InvalidPlatformFee(fee_bps));
    }
    Ok(())
}

/// Validate max rounds
fn validate_max_rounds(max_rounds: u8) -> Result<(), BattleError> {
    if max_rounds == 0 || max_rounds > 100 {
        return Err(BattleError::InvalidMaxRounds(max_rounds));
    }
    Ok(())
}

impl Contract for BattleContract {
    type Message = Message;
    type Parameters = BattleParameters;
    type InstantiationArgument = (); // No arguments needed
    type EventValue = BattleEvent;

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = BattleState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        // With auto-deployment, this method just initializes empty state
        // Actual battle initialization happens via Initialize message

        // Initialize empty player slots
        self.state.player1.set(None);
        self.state.player2.set(None);

        // Initialize battle metadata with defaults
        self.state.status.set(BattleStatus::WaitingForPlayers);
        self.state.current_round.set(0);
        self.state.max_rounds.set(10);
        self.state.winner.set(None);
        self.state.round_results.set(Vec::new());
        self.state.battle_log.set(Vec::new());

        // Initialize randomness counter
        self.state.random_counter.set(0);

        // Initialize empty references (will be set via Initialize message)
        self.state.battle_token_app.set(None);
        self.state.matchmaking_chain.set(None);
        self.state.prediction_chain_id.set(None);
        self.state.platform_fee_bps.set(300); // Default 3%
        self.state.treasury_owner.set(None);

        // Initialize empty timestamps
        self.state.started_at.set(None);
        self.state.completed_at.set(None);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::SubmitTurn {
                round,
                turn,
                stance,
                use_special,
            } => {
                if *self.state.status.get() != BattleStatus::InProgress {
                    return; // Battle not in progress
                }

                if round != *self.state.current_round.get() {
                    return; // Invalid round
                }

                if turn >= 3 {
                    return; // Invalid turn
                }

                // Get caller from authenticated signer
                let caller = self
                    .runtime
                    .authenticated_signer()
                    .expect("Operation must be authenticated");

                // Get participant and update turn
                let mut p1 = self.state.player1.get().clone();
                let mut p2 = self.state.player2.get().clone();

                if let Some(ref mut participant) = p1 {
                    if participant.owner == caller {
                        if participant.current_hp > 0 && participant.turns_submitted[turn as usize].is_none() {
                            participant.turns_submitted[turn as usize] = Some(TurnSubmission {
                                round,
                                turn,
                                stance,
                                use_special,
                            });
                            self.state.player1.set(p1);
                        }
                        return;
                    }
                }

                if let Some(ref mut participant) = p2 {
                    if participant.owner == caller {
                        if participant.current_hp > 0 && participant.turns_submitted[turn as usize].is_none() {
                            participant.turns_submitted[turn as usize] = Some(TurnSubmission {
                                round,
                                turn,
                                stance,
                                use_special,
                            });
                            self.state.player2.set(p2);
                        }
                    }
                }
            }

            Operation::ExecuteRound => {
                // NOTE: No authentication check - anyone can execute rounds
                // This prevents griefing where a player refuses to trigger round execution
                // The operation is safe because it only processes submitted turns
                if *self.state.status.get() != BattleStatus::InProgress {
                    return;
                }

                // Check if both players submitted all turns
                let p1 = self.state.player1.get().clone();
                let p2 = self.state.player2.get().clone();

                if let (Some(ref player1), Some(ref player2)) = (p1, p2) {
                    if !player1.all_turns_submitted() || !player2.all_turns_submitted() {
                        return; // Not all turns submitted
                    }

                    // Execute the round
                    let now = self.runtime.system_time();
                    let current_round = *self.state.current_round.get();

                    if let Ok(round_result) = self.state.execute_full_round(now) {
                        let mut results = self.state.round_results.get().clone();
                        results.push(round_result.clone());
                        self.state.round_results.set(results);

                        // If this is round 1, notify prediction market to close betting
                        if current_round == 1 {
                            if let Some(prediction_chain) = self.state.prediction_chain_id.get().as_ref() {
                                let battle_chain = self.runtime.chain_id();
                                self.runtime
                                    .prepare_message(Message::BattleStarted {
                                        battle_chain,
                                    })
                                    .with_authentication()
                                    .send_to(*prediction_chain);

                                log::info!("Sent BattleStarted message to prediction market");
                            }
                        }

                        // Check for winner
                        let p1 = self.state.player1.get().clone().unwrap();
                        let p2 = self.state.player2.get().clone().unwrap();
                        let now = self.runtime.system_time();

                        if p1.current_hp == 0 {
                            self.state.winner.set(Some(p2.owner));
                            self.state.status.set(BattleStatus::Completed);
                            self.state.completed_at.set(Some(now));
                        } else if p2.current_hp == 0 {
                            self.state.winner.set(Some(p1.owner));
                            self.state.status.set(BattleStatus::Completed);
                            self.state.completed_at.set(Some(now));
                        } else if *self.state.current_round.get() >= *self.state.max_rounds.get() {
                            // Max rounds reached, winner is player with more HP
                            let winner_owner = if p1.current_hp > p2.current_hp {
                                p1.owner
                            } else {
                                p2.owner
                            };
                            self.state.winner.set(Some(winner_owner));
                            self.state.status.set(BattleStatus::Completed);
                            self.state.completed_at.set(Some(now));
                        } else {
                            // Continue to next round
                            self.state.current_round.set(*self.state.current_round.get() + 1);
                            let mut p1 = self.state.player1.get().clone().unwrap();
                            let mut p2 = self.state.player2.get().clone().unwrap();
                            p1.reset_turns();
                            p2.reset_turns();
                            self.state.player1.set(Some(p1));
                            self.state.player2.set(Some(p2));
                        }
                    }
                }
            }

            Operation::FinalizeBattle => {
                // NOTE: No authentication check - anyone can finalize completed battles
                // This prevents battles from being stuck if players don't finalize
                // The operation is safe because it only executes after battle is completed
                if *self.state.status.get() != BattleStatus::Completed {
                    return;
                }

                let p1 = self.state.player1.get().clone().unwrap();
                let p2 = self.state.player2.get().clone().unwrap();
                let winner_owner = self.state.winner.get().clone().unwrap();
                let loser_owner = if winner_owner == p1.owner {
                    p2.owner
                } else {
                    p1.owner
                };

                // Calculate payouts
                let total_stake = p1.stake.saturating_add(p2.stake);
                let platform_fee_bps = *self.state.platform_fee_bps.get();

                // Calculate platform fee: (total * fee_bps) / 10000
                // Amount is u128 internally, work with it as u128
                let total_attos = u128::from(total_stake);
                let fee_numerator = total_attos.saturating_mul(platform_fee_bps as u128);
                let platform_fee_attos = fee_numerator / 10000;
                // Construct Amount from attos (smallest unit)
                let platform_fee = Amount::from_attos(platform_fee_attos);

                let winner_payout = total_stake.saturating_sub(platform_fee);

                // Transfer platform fee and winner payout via battle token application
                if let (Some(battle_token_app), Some(treasury_owner)) = (
                    self.state.battle_token_app.get().as_ref(),
                    self.state.treasury_owner.get().as_ref(),
                ) {
                    // Transfer platform fee to treasury
                    if platform_fee > Amount::ZERO {
                        let response: TokenResponse = self.runtime.call_application(
                            true, // authenticated call
                            *battle_token_app,
                            &BattleTokenOperation::Transfer {
                                to: *treasury_owner,
                                amount: platform_fee,
                            },
                        );

                        match response {
                            TokenResponse::TransferSuccess => {
                                log::info!(
                                    "Transferred platform fee {} to treasury {:?}",
                                    platform_fee,
                                    treasury_owner
                                );
                            }
                            response => panic!("Platform fee transfer failed with response: {:?}", response),
                        }
                    }

                    // Transfer winner payout to winner
                    if winner_payout > Amount::ZERO {
                        let response: TokenResponse = self.runtime.call_application(
                            true, // authenticated call
                            *battle_token_app,
                            &BattleTokenOperation::Transfer {
                                to: winner_owner,
                                amount: winner_payout,
                            },
                        );

                        match response {
                            TokenResponse::TransferSuccess => {
                                log::info!(
                                    "Transferred winner payout {} to {:?}",
                                    winner_payout,
                                    winner_owner
                                );
                            }
                            response => panic!("Winner payout transfer failed with response: {:?}", response),
                        }
                    }
                }

                // Send battle result messages to both player chains
                let result_message = Message::BattleResult {
                    winner: winner_owner,
                    loser: loser_owner,
                    winner_payout,
                    rounds_played: *self.state.current_round.get(),
                };

                self.runtime
                    .prepare_message(result_message.clone())
                    .with_authentication()
                    .send_to(p1.chain);

                self.runtime
                    .prepare_message(result_message)
                    .with_authentication()
                    .send_to(p2.chain);

                // Notify matchmaking chain of completion
                if let Some(matchmaking_chain) = self.state.matchmaking_chain.get().as_ref() {
                    let completion_message = Message::BattleCompleted {
                        winner: winner_owner,
                        loser: loser_owner,
                    };

                    self.runtime
                        .prepare_message(completion_message)
                        .with_authentication()
                        .send_to(*matchmaking_chain);
                }

                // Determine winner and loser chains for prediction market
                let winner_chain = if winner_owner == p1.owner {
                    p1.chain
                } else {
                    p2.chain
                };

                // Notify prediction market of battle result
                if let Some(prediction_chain) = self.state.prediction_chain_id.get().as_ref() {
                    let battle_ended_message = Message::BattleEnded {
                        battle_chain: self.runtime.chain_id(),
                        winner_chain,
                    };

                    self.runtime
                        .prepare_message(battle_ended_message)
                        .with_authentication()
                        .send_to(*prediction_chain);

                    log::info!("Sent BattleEnded message to prediction market");
                }

                // Emit BattleCompleted event for cross-application subscriptions
                // This allows prediction-chain and registry-chain to listen for battle results

                let loser_chain = if winner_owner == p1.owner {
                    p2.chain
                } else {
                    p1.chain
                };

                // Calculate combat statistics for both players
                let round_results = self.state.round_results.get().clone();
                let (p1_damage_dealt, p1_damage_taken, p1_crits, p1_dodges, p1_highest_crit) =
                    calculate_combat_stats(&round_results, &p1.owner);
                let (p2_damage_dealt, p2_damage_taken, p2_crits, p2_dodges, p2_highest_crit) =
                    calculate_combat_stats(&round_results, &p2.owner);

                let player1_stats = CombatStats::from_actions(
                    p1_damage_dealt,
                    p1_damage_taken,
                    p1_crits,
                    p1_dodges,
                    p1_highest_crit,
                );

                let player2_stats = CombatStats::from_actions(
                    p2_damage_dealt,
                    p2_damage_taken,
                    p2_crits,
                    p2_dodges,
                    p2_highest_crit,
                );

                let battle_chain_id = self.runtime.chain_id();
                self.runtime.emit(
                    "battle_events".into(),
                    &BattleEvent::BattleCompleted {
                        battle_chain: battle_chain_id,
                        player1_chain: p1.chain,
                        player2_chain: p2.chain,
                        winner_chain,
                        loser_chain,
                        stake: total_stake,
                        rounds_played: *self.state.current_round.get(),
                        player1_stats,
                        player2_stats,
                    },
                );

                log::info!(
                    "Battle completed on chain {:?}: winner {:?}",
                    self.runtime.chain_id(),
                    winner_owner
                );
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::Initialize {
                player1,
                player2,
                matchmaking_chain,
                battle_token_app,
                prediction_chain_id,
                platform_fee_bps,
                treasury_owner,
            } => {
                // This message triggers automatic deployment!
                // Verify sender is the matchmaking chain (security check)
                let sender_chain = self.runtime.message_origin_chain_id()
                    .expect("Message must have origin");

                assert_eq!(
                    sender_chain, matchmaking_chain,
                    "Only matchmaking chain can initialize battles"
                );

                // Check battle not already initialized
                if self.state.player1.get().is_some() || self.state.player2.get().is_some() {
                    panic!("Battle already initialized");
                }

                // SECURITY: Validate input parameters
                validate_stake(player1.stake).expect("Invalid player1 stake");
                validate_stake(player2.stake).expect("Invalid player2 stake");
                validate_platform_fee(platform_fee_bps).expect("Invalid platform fee");
                validate_max_rounds(10).expect("Invalid max rounds");  // Using default of 10

                // SECURITY: Validate players are different
                assert_ne!(
                    player1.owner, player2.owner,
                    "Players must be different"
                );

                let now = self.runtime.system_time();

                // Initialize battle participants
                self.state.player1.set(Some(player1.clone()));
                self.state.player2.set(Some(player2.clone()));

                // Initialize battle metadata
                self.state.status.set(BattleStatus::InProgress);
                self.state.current_round.set(0);
                self.state.max_rounds.set(10); // Default max rounds
                self.state.winner.set(None);
                self.state.round_results.set(Vec::new());

                // Initialize randomness counter
                self.state.random_counter.set(0);

                // Initialize configuration
                self.state.battle_token_app.set(Some(battle_token_app));
                self.state.matchmaking_chain.set(Some(matchmaking_chain));
                self.state.prediction_chain_id.set(prediction_chain_id);
                self.state.platform_fee_bps.set(platform_fee_bps);
                self.state.treasury_owner.set(Some(treasury_owner));

                // Initialize timestamps
                self.state.started_at.set(Some(now));
                self.state.completed_at.set(None);

                // Initialize combat log
                let mut battle_log = Vec::new();
                battle_log.push(format!(
                    "Battle initialized: {:?} vs {:?}",
                    player1.owner, player2.owner
                ));
                self.state.battle_log.set(battle_log);

                // Emit BattleStarted event for cross-chain subscriptions
                let total_stake = player1.stake.saturating_add(player2.stake);
                let battle_chain_id = self.runtime.chain_id();
                self.runtime.emit(
                    "battle_events".into(),
                    &BattleEvent::BattleStarted {
                        battle_chain: battle_chain_id,
                        player1_chain: player1.chain,
                        player2_chain: player2.chain,
                        total_stake,
                    },
                );

                log::info!(
                    "Battle initialized on chain {:?}: {:?} vs {:?}",
                    self.runtime.chain_id(),
                    player1.owner,
                    player2.owner
                );
            }

            Message::BattleResult { .. }
            | Message::BattleCompleted { .. }
            | Message::BattleStarted { .. }
            | Message::BattleEnded { .. } => {
                // These are outgoing messages, not handled here
                log::warn!("Received outgoing message type - ignoring");
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
