// Copyright (c) Fighter Game
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::ComplexObject;
use fighter_game::{
    Battle, BattleConfig, BattleStatus, Fighter, FighterGameAbi, FighterOutcome, 
    MatchmakingTier, Operation, Tournament, TournamentStatus,
};
use linera_sdk::{
    linera_base_types::{
        AccountOwner, Amount, ApplicationPermissions, ChainId, ChainOwnership, 
        TimeoutConfig, WithContractAbi,
    },
    views::{RootView, View},
    Contract, ContractRuntime,
};
use serde::{Deserialize, Serialize};
use state::FighterGameState;

pub struct FighterGameContract {
    state: FighterGameState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(FighterGameContract);

impl WithContractAbi for FighterGameContract {
    type Abi = FighterGameAbi;
}

impl Contract for FighterGameContract {
    type Message = Message;
    type InstantiationArgument = BattleConfig;
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = FighterGameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        FighterGameContract { state, runtime }
    }

    async fn instantiate(&mut self, config: BattleConfig) {
        log::trace!("Instantiating Fighter Game");
        self.runtime.application_parameters();
        self.state.config.set(config);
        self.state.battle_counter.set(0);
        self.state.tournament_counter.set(0);
        self.state.platform_balance.set(0);
        self.state.total_battles.set(0);
        self.state.total_xp_distributed.set(0);
    }

    async fn execute_operation(&mut self, operation: Operation) -> FighterOutcome {
        log::trace!("Handling operation {:?}", operation);
        
        match operation {
            Operation::RegisterFighter { name } => {
                self.execute_register_fighter(name).await
            }
            Operation::StartFreeBattle { opponent } => {
                self.execute_start_battle(opponent, Amount::ZERO).await
            }
            Operation::StartStakedBattle { opponent, stake_amount } => {
                self.execute_start_battle(opponent, stake_amount).await
            }
            Operation::Strike { battle_id } => {
                self.execute_strike(battle_id).await
            }
            Operation::ClaimTimeout { battle_id } => {
                self.execute_claim_timeout(battle_id).await
            }
            Operation::CreateTournament { 
                name, 
                entry_fee, 
                start_time, 
                max_participants,
                prize_pool_distribution 
            } => {
                self.execute_create_tournament(
                    name, 
                    entry_fee, 
                    start_time, 
                    max_participants,
                    prize_pool_distribution
                ).await
            }
            Operation::JoinTournament { tournament_id } => {
                self.execute_join_tournament(tournament_id).await
            }
            Operation::PlacePrediction { 
                battle_id, 
                predicted_winner, 
                bet_amount 
            } => {
                self.execute_place_prediction(battle_id, predicted_winner, bet_amount).await
            }
            Operation::ClaimPredictionWinnings { battle_id } => {
                self.execute_claim_prediction_winnings(battle_id).await
            }
            Operation::UpgradeFighter => {
                self.execute_upgrade_fighter().await
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        log::trace!("Handling message {:?}", message);
        match message {
            Message::BattleResult { 
                battle_id, 
                winner, 
                loser,
                xp_winner,
                xp_loser,
            } => {
                // Update fighter stats
                if let Ok(Some(mut winner_fighter)) = self.state.fighters.get_mut(&winner).await {
                    winner_fighter.total_wins += 1;
                    winner_fighter.current_streak += 1;
                    winner_fighter.highest_streak = winner_fighter.highest_streak.max(winner_fighter.current_streak);
                    
                    let leveled_up = winner_fighter.add_xp(xp_winner);
                    winner_fighter.last_battle = Some(self.runtime.system_time());
                    
                    if leveled_up {
                        log::info!("Fighter {} leveled up to {}", winner_fighter.name, winner_fighter.level);
                    }
                }
                
                if let Ok(Some(mut loser_fighter)) = self.state.fighters.get_mut(&loser).await {
                    loser_fighter.total_losses += 1;
                    loser_fighter.current_streak = 0;
                    loser_fighter.add_xp(xp_loser);
                    loser_fighter.last_battle = Some(self.runtime.system_time());
                }
                
                // Update leaderboard
                self.update_leaderboard(winner).await;
                self.update_leaderboard(loser).await;
                
                // Increment total battles
                let total = self.state.total_battles.get() + 1;
                self.state.total_battles.set(total);
            }
            Message::TournamentStarted { tournament_id } => {
                // Tournament logic would go here
                log::info!("Tournament {} started", tournament_id);
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl FighterGameContract {
    /// Register a new fighter
    async fn execute_register_fighter(&mut self, name: String) -> FighterOutcome {
        let owner = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated to register".to_string() 
            },
        };
        
        // Check if fighter already exists
        if self.state.fighters.get(&owner).await.unwrap().is_some() {
            return FighterOutcome::Error { 
                message: "Fighter already registered".to_string() 
            };
        }
        
        // Validate name
        if name.is_empty() || name.len() > 32 {
            return FighterOutcome::Error { 
                message: "Invalid fighter name (1-32 characters)".to_string() 
            };
        }
        
        let timestamp = self.runtime.system_time();
        let fighter = Fighter::new(owner, name, timestamp);
        
        self.state.fighters.insert(&owner, fighter.clone()).unwrap();
        self.state.leaderboard.insert(&owner, 0).unwrap();
        
        log::info!("Fighter registered: {} for {}", fighter.name, owner);
        
        FighterOutcome::FighterRegistered { fighter }
    }
    
    /// Start a battle between two fighters
    async fn execute_start_battle(&mut self, opponent: AccountOwner, stake_amount: Amount) -> FighterOutcome {
        let initiator = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        if initiator == opponent {
            return FighterOutcome::Error { 
                message: "Cannot battle yourself".to_string() 
            };
        }
        
        // Get both fighters
        let fighter1 = match self.state.fighters.get(&initiator).await.unwrap() {
            Some(f) => f,
            None => return FighterOutcome::Error { 
                message: "Initiator not registered".to_string() 
            },
        };
        
        let fighter2 = match self.state.fighters.get(&opponent).await.unwrap() {
            Some(f) => f,
            None => return FighterOutcome::Error { 
                message: "Opponent not registered".to_string() 
            },
        };
        
        // Check tier compatibility
        if !fighter1.get_tier().can_match(&fighter2.get_tier()) {
            return FighterOutcome::Error { 
                message: "Fighters are not in compatible tiers".to_string() 
            };
        }
        
        // Handle staking if required
        if stake_amount > Amount::ZERO {
            // In a real implementation, you would:
            // 1. Transfer stake_amount from both players
            // 2. Hold it in escrow
            // For this example, we'll just validate they have sufficient balance
            // The actual token transfer would be handled by the Linera runtime
        }
        
        // Create new battle
        let battle_id = self.state.battle_counter.get() + 1;
        self.state.battle_counter.set(battle_id);
        
        let timestamp = self.runtime.system_time();
        let battle = Battle::new(battle_id, &fighter1, &fighter2, stake_amount, timestamp);
        
        self.state.battles.insert(&battle_id, battle).unwrap();
        
        log::info!("Battle {} started: {} vs {}", battle_id, initiator, opponent);
        
        FighterOutcome::BattleStarted { battle_id }
    }
    
    /// Execute a strike in an active battle
    async fn execute_strike(&mut self, battle_id: u64) -> FighterOutcome {
        let attacker = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        let mut battle = match self.state.battles.get(&battle_id).await.unwrap() {
            Some(b) => b,
            None => return FighterOutcome::Error { 
                message: "Battle not found".to_string() 
            },
        };
        
        if battle.status != BattleStatus::Active {
            return FighterOutcome::Error { 
                message: "Battle is not active".to_string() 
            };
        }
        
        if battle.current_turn != attacker {
            return FighterOutcome::Error { 
                message: "Not your turn".to_string() 
            };
        }
        
        // Get fighters
        let attacker_fighter = self.state.fighters.get(&attacker).await.unwrap().unwrap();
        let defender = if attacker == battle.fighter1 {
            battle.fighter2
        } else {
            battle.fighter1
        };
        let defender_fighter = self.state.fighters.get(&defender).await.unwrap().unwrap();
        
        // Generate randomness using block timestamp and chain ID
        // In production, use Linera's VRF or oracle
        let timestamp = self.runtime.system_time();
        let random_seed = self.generate_random_seed(battle_id, timestamp);
        
        // Execute the strike
        let damage = match battle.execute_strike(
            attacker,
            &attacker_fighter,
            &defender_fighter,
            random_seed,
            timestamp,
        ) {
            Ok(dmg) => dmg,
            Err(e) => return FighterOutcome::Error { message: e },
        };
        
        // Check if battle ended
        let winner = battle.winner;
        
        // Update battle state
        self.state.battles.insert(&battle_id, battle.clone()).unwrap();
        
        // If battle ended, distribute rewards
        if let Some(winner_owner) = winner {
            let loser = if winner_owner == battle.fighter1 {
                battle.fighter2
            } else {
                battle.fighter1
            };
            
            // Calculate XP rewards
            let winner_xp = self.calculate_xp_reward(true, &battle);
            let loser_xp = self.calculate_xp_reward(false, &battle);
            
            // Update total XP distributed
            let total_xp = self.state.total_xp_distributed.get() + winner_xp + loser_xp;
            self.state.total_xp_distributed.set(total_xp);
            
            // Handle stake distribution
            if battle.stake_amount > Amount::ZERO {
                self.distribute_stake_winnings(&battle, winner_owner).await;
            }
            
            // Distribute prediction pool
            self.distribute_prediction_winnings(&battle, winner_owner).await;
            
            // Send battle result message
            self.runtime.send_message(
                self.runtime.application_creator_chain_id(),
                Message::BattleResult {
                    battle_id,
                    winner: winner_owner,
                    loser,
                    xp_winner: winner_xp,
                    xp_loser: loser_xp,
                },
            );
            
            log::info!("Battle {} ended. Winner: {}", battle_id, winner_owner);
        }
        
        FighterOutcome::StrikeExecuted { damage, winner }
    }
    
    /// Claim victory if opponent times out
    async fn execute_claim_timeout(&mut self, battle_id: u64) -> FighterOutcome {
        let claimer = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        let mut battle = match self.state.battles.get(&battle_id).await.unwrap() {
            Some(b) => b,
            None => return FighterOutcome::Error { 
                message: "Battle not found".to_string() 
            },
        };
        
        if battle.status != BattleStatus::Active {
            return FighterOutcome::Error { 
                message: "Battle is not active".to_string() 
            };
        }
        
        let config = self.state.config.get();
        let current_time = self.runtime.system_time();
        
        if !battle.is_timed_out(current_time, config) {
            return FighterOutcome::Error { 
                message: "Opponent has not timed out".to_string() 
            };
        }
        
        // The current turn player timed out, so the claimer wins
        if battle.current_turn == claimer {
            return FighterOutcome::Error { 
                message: "You are the one who timed out".to_string() 
            };
        }
        
        battle.status = BattleStatus::TimedOut;
        battle.winner = Some(claimer);
        
        self.state.battles.insert(&battle_id, battle.clone()).unwrap();
        
        // Award XP and distribute winnings
        let loser = battle.current_turn;
        let winner_xp = self.calculate_xp_reward(true, &battle);
        let loser_xp = self.calculate_xp_reward(false, &battle) / 2; // Reduced XP for timeout loss
        
        if battle.stake_amount > Amount::ZERO {
            self.distribute_stake_winnings(&battle, claimer).await;
        }
        
        self.runtime.send_message(
            self.runtime.application_creator_chain_id(),
            Message::BattleResult {
                battle_id,
                winner: claimer,
                loser,
                xp_winner: winner_xp,
                xp_loser: loser_xp,
            },
        );
        
        FighterOutcome::Success
    }
    
    /// Create a new tournament
    async fn execute_create_tournament(
        &mut self,
        name: String,
        entry_fee: Amount,
        start_time: linera_sdk::linera_base_types::Timestamp,
        max_participants: u32,
        prize_pool_distribution: Vec<u8>,
    ) -> FighterOutcome {
        let creator = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        // Validate tournament parameters
        if name.is_empty() || name.len() > 64 {
            return FighterOutcome::Error { 
                message: "Invalid tournament name".to_string() 
            };
        }
        
        if max_participants < 4 || max_participants > 128 {
            return FighterOutcome::Error { 
                message: "Participants must be between 4 and 128".to_string() 
            };
        }
        
        let prize_sum: u32 = prize_pool_distribution.iter().map(|&x| x as u32).sum();
        if prize_sum != 100 {
            return FighterOutcome::Error { 
                message: "Prize distribution must sum to 100%".to_string() 
            };
        }
        
        let tournament_id = self.state.tournament_counter.get() + 1;
        self.state.tournament_counter.set(tournament_id);
        
        let tournament = Tournament {
            tournament_id,
            name,
            entry_fee,
            prize_pool: Amount::ZERO,
            start_time,
            max_participants,
            participants: vec![],
            brackets: vec![],
            status: TournamentStatus::Registration,
            winner: None,
            prize_distribution: prize_pool_distribution,
        };
        
        self.state.tournaments.insert(&tournament_id, tournament).unwrap();
        
        FighterOutcome::TournamentCreated { tournament_id }
    }
    
    /// Join a tournament
    async fn execute_join_tournament(&mut self, tournament_id: u64) -> FighterOutcome {
        let participant = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        // Check if fighter is registered
        if self.state.fighters.get(&participant).await.unwrap().is_none() {
            return FighterOutcome::Error { 
                message: "Must register fighter first".to_string() 
            };
        }
        
        let mut tournament = match self.state.tournaments.get(&tournament_id).await.unwrap() {
            Some(t) => t,
            None => return FighterOutcome::Error { 
                message: "Tournament not found".to_string() 
            },
        };
        
        if tournament.status != TournamentStatus::Registration {
            return FighterOutcome::Error { 
                message: "Tournament registration closed".to_string() 
            };
        }
        
        if tournament.participants.len() >= tournament.max_participants as usize {
            return FighterOutcome::Error { 
                message: "Tournament full".to_string() 
            };
        }
        
        if tournament.participants.contains(&participant) {
            return FighterOutcome::Error { 
                message: "Already registered".to_string() 
            };
        }
        
        // Add entry fee to prize pool
        tournament.prize_pool = tournament.prize_pool.saturating_add(tournament.entry_fee);
        tournament.participants.push(participant);
        
        self.state.tournaments.insert(&tournament_id, tournament).unwrap();
        
        FighterOutcome::Success
    }
    
    /// Place a prediction on battle outcome
    async fn execute_place_prediction(
        &mut self,
        battle_id: u64,
        predicted_winner: AccountOwner,
        bet_amount: Amount,
    ) -> FighterOutcome {
        let predictor = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        let mut battle = match self.state.battles.get(&battle_id).await.unwrap() {
            Some(b) => b,
            None => return FighterOutcome::Error { 
                message: "Battle not found".to_string() 
            },
        };
        
        if battle.status != BattleStatus::Active {
            return FighterOutcome::Error { 
                message: "Battle not active".to_string() 
            };
        }
        
        if predicted_winner != battle.fighter1 && predicted_winner != battle.fighter2 {
            return FighterOutcome::Error { 
                message: "Invalid fighter".to_string() 
            };
        }
        
        if bet_amount == Amount::ZERO {
            return FighterOutcome::Error { 
                message: "Bet amount must be greater than zero".to_string() 
            };
        }
        
        battle.prediction_pool.add_prediction(predictor, predicted_winner, bet_amount, battle.fighter1);
        
        self.state.battles.insert(&battle_id, battle).unwrap();
        
        FighterOutcome::Success
    }
    
    /// Claim prediction winnings
    async fn execute_claim_prediction_winnings(&mut self, battle_id: u64) -> FighterOutcome {
        let claimer = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        let mut battle = match self.state.battles.get(&battle_id).await.unwrap() {
            Some(b) => b,
            None => return FighterOutcome::Error { 
                message: "Battle not found".to_string() 
            },
        };
        
        if battle.status != BattleStatus::Finished {
            return FighterOutcome::Error { 
                message: "Battle not finished".to_string() 
            };
        }
        
        let winner = match battle.winner {
            Some(w) => w,
            None => return FighterOutcome::Error { 
                message: "No winner determined".to_string() 
            },
        };
        
        let config = self.state.config.get();
        let winnings = battle.prediction_pool.calculate_winnings(&claimer, winner, battle.fighter1, config.platform_fee);
        
        if winnings == Amount::ZERO {
            return FighterOutcome::Error { 
                message: "No winnings to claim".to_string() 
            };
        }
        
        // Mark as claimed
        if let Some(prediction) = battle.prediction_pool.predictions.get_mut(&claimer) {
            if prediction.claimed {
                return FighterOutcome::Error { 
                    message: "Already claimed".to_string() 
                };
            }
            prediction.claimed = true;
        }
        
        self.state.battles.insert(&battle_id, battle).unwrap();
        
        // In production, transfer winnings to claimer
        
        FighterOutcome::Success
    }
    
    /// Upgrade fighter with accumulated XP
    async fn execute_upgrade_fighter(&mut self) -> FighterOutcome {
        let owner = match self.runtime.authenticated_owner() {
            Some(owner) => owner,
            None => return FighterOutcome::Error { 
                message: "Must be authenticated".to_string() 
            },
        };
        
        let mut fighter = match self.state.fighters.get(&owner).await.unwrap() {
            Some(f) => f,
            None => return FighterOutcome::Error { 
                message: "Fighter not found".to_string() 
            },
        };
        
        let old_level = fighter.level;
        
        // Try to level up as many times as possible
        let mut total_levels = 0;
        while fighter.xp >= fighter.xp_for_next_level() {
            fighter.add_xp(0); // This will trigger level up if XP is sufficient
            total_levels += 1;
            if total_levels > 10 {
                break; // Safety limit
            }
        }
        
        if fighter.level == old_level {
            return FighterOutcome::Error { 
                message: "Not enough XP to level up".to_string() 
            };
        }
        
        self.state.fighters.insert(&owner, fighter.clone()).unwrap();
        self.update_leaderboard(owner).await;
        
        FighterOutcome::LevelUp { new_level: fighter.level }
    }
    
    // Helper functions
    
    fn generate_random_seed(&self, battle_id: u64, timestamp: linera_sdk::linera_base_types::Timestamp) -> u64 {
        // Simple pseudo-random generation
        // In production, use Linera's VRF or Chainlink VRF
        let chain_id_bytes = self.runtime.chain_id().to_string();
        let mut seed = battle_id;
        seed ^= timestamp.micros();
        for byte in chain_id_bytes.as_bytes() {
            seed = seed.wrapping_mul(31).wrapping_add(*byte as u64);
        }
        seed
    }
    
    fn calculate_xp_reward(&self, is_winner: bool, battle: &Battle) -> u64 {
        let base_xp = if is_winner { 100 } else { 20 };
        
        let multiplier = if battle.stake_amount > Amount::ZERO {
            150 // 1.5x for staked battles
        } else {
            100
        };
        
        (base_xp * multiplier) / 100
    }
    
    async fn distribute_stake_winnings(&mut self, battle: &Battle, winner: AccountOwner) {
        let config = self.state.config.get();
        let total_stake = battle.stake_amount.saturating_mul(2);
        
        // Calculate platform fee
        let fee_amount = Amount::from_tokens(
            (total_stake.saturating_mul(config.platform_fee as u128)) / 100
        );
        
        let winner_amount = total_stake.saturating_sub(fee_amount);
        
        // Update platform balance
        let platform_bal = self.state.platform_balance.get() + fee_amount.into();
        self.state.platform_balance.set(platform_bal);
        
        // In production, transfer winner_amount to winner
        log::info!("Distributing {} to winner {}", winner_amount, winner);
    }
    
    async fn distribute_prediction_winnings(&mut self, battle: &Battle, winner: AccountOwner) {
        let config = self.state.config.get();
        let total_pool = battle.prediction_pool.total_pool;
        
        let fee_amount = Amount::from_tokens(
            (total_pool.saturating_mul(config.platform_fee as u128)) / 100
        );
        
        let platform_bal = self.state.platform_balance.get() + fee_amount.into();
        self.state.platform_balance.set(platform_bal);
        
        log::info!("Prediction pool resolved for battle {}", battle.battle_id);
    }
    
    async fn update_leaderboard(&mut self, owner: AccountOwner) {
        if let Ok(Some(fighter)) = self.state.fighters.get(&owner).await {
            self.state.leaderboard.insert(&owner, fighter.xp).unwrap();
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    BattleResult {
        battle_id: u64,
        winner: AccountOwner,
        loser: AccountOwner,
        xp_winner: u64,
        xp_loser: u64,
    },
    TournamentStarted {
        tournament_id: u64,
    },
}

#[ComplexObject]
impl FighterGameState {}
