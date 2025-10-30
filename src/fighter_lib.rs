// Copyright (c) Fighter Game
// SPDX-License-Identifier: Apache-2.0

/*! ABI and core logic for the Fighter Game */

use async_graphql::{Enum, InputObject, Request, Response, SimpleObject};
use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{AccountOwner, Amount, ContractAbi, ServiceAbi, TimeDelta, Timestamp},
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct FighterGameAbi;

/// Main operations that can be performed on the contract
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    /// Register a new fighter with base stats
    RegisterFighter {
        name: String,
    },
    
    /// Start a new free-play battle (no stakes)
    StartFreeBattle {
        opponent: AccountOwner,
    },
    
    /// Start a staked battle with bet amount
    StartStakedBattle {
        opponent: AccountOwner,
        stake_amount: Amount,
    },
    
    /// Execute a strike in an active battle
    Strike {
        battle_id: u64,
    },
    
    /// Claim victory if opponent times out
    ClaimTimeout {
        battle_id: u64,
    },
    
    /// Create a scheduled tournament
    CreateTournament {
        name: String,
        entry_fee: Amount,
        start_time: Timestamp,
        max_participants: u32,
        prize_pool_distribution: Vec<u8>, // Percentages for 1st, 2nd, 3rd place
    },
    
    /// Join a tournament
    JoinTournament {
        tournament_id: u64,
    },
    
    /// Place a prediction on a battle outcome
    PlacePrediction {
        battle_id: u64,
        predicted_winner: AccountOwner,
        bet_amount: Amount,
    },
    
    /// Claim prediction winnings
    ClaimPredictionWinnings {
        battle_id: u64,
    },
    
    /// Upgrade fighter NFT with earned XP
    UpgradeFighter,
}

impl ContractAbi for FighterGameAbi {
    type Operation = Operation;
    type Response = FighterOutcome;
}

impl ServiceAbi for FighterGameAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Configuration for battle mechanics and timing
#[derive(Clone, Debug, Deserialize, Serialize, SimpleObject, InputObject)]
#[graphql(input_name = "BattleConfigInput")]
#[serde(rename_all = "camelCase")]
pub struct BattleConfig {
    /// Time each player has per turn
    pub turn_timeout: TimeDelta,
    /// Maximum time allowed between block proposal and validation
    pub block_delay: TimeDelta,
    /// Platform fee percentage (0-100)
    pub platform_fee: u8,
}

impl Default for BattleConfig {
    fn default() -> Self {
        BattleConfig {
            turn_timeout: TimeDelta::from_secs(30),
            block_delay: TimeDelta::from_secs(5),
            platform_fee: 10, // 10%
        }
    }
}

/// Represents a fighter's statistics and progression
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject, PartialEq)]
pub struct Fighter {
    pub owner: AccountOwner,
    pub name: String,
    pub level: u32,
    pub xp: u64,
    pub total_wins: u32,
    pub total_losses: u32,
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub current_streak: u32,
    pub highest_streak: u32,
    
    // Combat stats
    pub max_hp: u32,
    pub base_attack: u32,
    pub defense: u32,
    pub critical_chance: u32, // 0-100
    pub critical_multiplier: u32, // 150 = 1.5x
    
    // NFT metadata
    pub nft_tier: FighterTier,
    pub special_abilities: Vec<String>,
    pub visual_traits: Vec<String>,
    
    // Timestamps
    pub created_at: Timestamp,
    pub last_battle: Option<Timestamp>,
}

impl Fighter {
    pub fn new(owner: AccountOwner, name: String, created_at: Timestamp) -> Self {
        Self {
            owner,
            name,
            level: 1,
            xp: 0,
            total_wins: 0,
            total_losses: 0,
            total_damage_dealt: 0,
            total_damage_taken: 0,
            current_streak: 0,
            highest_streak: 0,
            max_hp: 100,
            base_attack: 10,
            defense: 5,
            critical_chance: 5,
            critical_multiplier: 150,
            nft_tier: FighterTier::Bronze,
            special_abilities: vec![],
            visual_traits: vec![],
            created_at,
            last_battle: None,
        }
    }
    
    /// Calculate damage range based on level and stats
    pub fn calculate_damage_range(&self) -> (u32, u32) {
        let min_damage = self.base_attack + (self.level * 2);
        let max_damage = self.base_attack + (self.level * 5);
        (min_damage, max_damage)
    }
    
    /// Calculate effective damage after defense
    pub fn apply_defense(&self, incoming_damage: u32) -> u32 {
        let defense_reduction = (incoming_damage * self.defense) / 100;
        incoming_damage.saturating_sub(defense_reduction)
    }
    
    /// Check if attack is critical hit
    pub fn is_critical_hit(&self, random_value: u64) -> bool {
        (random_value % 100) < self.critical_chance as u64
    }
    
    /// Apply critical multiplier to damage
    pub fn apply_critical(&self, base_damage: u32) -> u32 {
        (base_damage * self.critical_multiplier) / 100
    }
    
    /// Calculate XP required for next level
    pub fn xp_for_next_level(&self) -> u64 {
        100 * (self.level as u64).pow(2)
    }
    
    /// Add XP and check for level up
    pub fn add_xp(&mut self, xp_gain: u64) -> bool {
        self.xp += xp_gain;
        let required_xp = self.xp_for_next_level();
        
        if self.xp >= required_xp {
            self.level_up();
            true
        } else {
            false
        }
    }
    
    /// Level up and improve stats
    fn level_up(&mut self) {
        self.level += 1;
        self.max_hp += 10;
        self.base_attack += 3;
        self.defense += 1;
        
        // Every 5 levels, increase crit chance
        if self.level % 5 == 0 {
            self.critical_chance = (self.critical_chance + 2).min(30);
        }
        
        // Update tier based on level
        self.nft_tier = FighterTier::from_level(self.level);
        
        // Unlock abilities at milestones
        if self.level == 10 {
            self.special_abilities.push("Power Strike".to_string());
        } else if self.level == 25 {
            self.special_abilities.push("Defensive Stance".to_string());
        } else if self.level == 50 {
            self.special_abilities.push("Berserker Mode".to_string());
        }
    }
    
    /// Get matchmaking tier
    pub fn get_tier(&self) -> MatchmakingTier {
        MatchmakingTier::from_level(self.level)
    }
}

/// Fighter NFT tiers that evolve with level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum FighterTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Legendary,
}

impl FighterTier {
    pub fn from_level(level: u32) -> Self {
        match level {
            1..=10 => FighterTier::Bronze,
            11..=25 => FighterTier::Silver,
            26..=50 => FighterTier::Gold,
            51..=75 => FighterTier::Platinum,
            76..=99 => FighterTier::Diamond,
            _ => FighterTier::Legendary,
        }
    }
}

/// Matchmaking tiers for fair battles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum MatchmakingTier {
    Novice,      // Level 1-10
    Intermediate, // Level 11-25
    Advanced,     // Level 26-50
    Expert,       // Level 51-100
    Master,       // Level 100+
}

impl MatchmakingTier {
    pub fn from_level(level: u32) -> Self {
        match level {
            1..=10 => MatchmakingTier::Novice,
            11..=25 => MatchmakingTier::Intermediate,
            26..=50 => MatchmakingTier::Advanced,
            51..=100 => MatchmakingTier::Expert,
            _ => MatchmakingTier::Master,
        }
    }
    
    pub fn can_match(&self, other: &Self) -> bool {
        // Allow matching within same tier or adjacent tiers
        let diff = (*self as i32 - *other as i32).abs();
        diff <= 1
    }
}

/// Current state of a battle
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Battle {
    pub battle_id: u64,
    pub fighter1: AccountOwner,
    pub fighter2: AccountOwner,
    pub fighter1_hp: u32,
    pub fighter2_hp: u32,
    pub fighter1_max_hp: u32,
    pub fighter2_max_hp: u32,
    pub current_turn: AccountOwner,
    pub turn_number: u32,
    pub stake_amount: Amount,
    pub is_free_play: bool,
    pub battle_type: BattleType,
    pub status: BattleStatus,
    pub winner: Option<AccountOwner>,
    pub combo_tracker: ComboTracker,
    pub battle_log: Vec<BattleAction>,
    pub started_at: Timestamp,
    pub last_action_at: Timestamp,
    pub prediction_pool: PredictionPool,
}

impl Battle {
    pub fn new(
        battle_id: u64,
        fighter1: &Fighter,
        fighter2: &Fighter,
        stake_amount: Amount,
        started_at: Timestamp,
    ) -> Self {
        Self {
            battle_id,
            fighter1: fighter1.owner,
            fighter2: fighter2.owner,
            fighter1_hp: fighter1.max_hp,
            fighter2_hp: fighter2.max_hp,
            fighter1_max_hp: fighter1.max_hp,
            fighter2_max_hp: fighter2.max_hp,
            current_turn: fighter1.owner,
            turn_number: 1,
            stake_amount,
            is_free_play: stake_amount == Amount::ZERO,
            battle_type: if stake_amount == Amount::ZERO {
                BattleType::FreePlay
            } else {
                BattleType::Staked
            },
            status: BattleStatus::Active,
            winner: None,
            combo_tracker: ComboTracker::new(),
            battle_log: vec![],
            started_at,
            last_action_at: started_at,
            prediction_pool: PredictionPool::new(battle_id),
        }
    }
    
    /// Execute a strike and return damage dealt
    pub fn execute_strike(
        &mut self,
        attacker: AccountOwner,
        attacker_fighter: &Fighter,
        defender_fighter: &Fighter,
        random_seed: u64,
        timestamp: Timestamp,
    ) -> Result<u32, String> {
        if self.status != BattleStatus::Active {
            return Err("Battle is not active".to_string());
        }
        
        if self.current_turn != attacker {
            return Err("Not attacker's turn".to_string());
        }
        
        // Calculate base damage with randomness
        let (min_dmg, max_dmg) = attacker_fighter.calculate_damage_range();
        let damage_range = max_dmg - min_dmg;
        let base_damage = min_dmg + ((random_seed % damage_range as u64) as u32);
        
        // Check for critical hit
        let mut final_damage = if attacker_fighter.is_critical_hit(random_seed) {
            let crit_damage = attacker_fighter.apply_critical(base_damage);
            self.battle_log.push(BattleAction {
                turn: self.turn_number,
                attacker,
                action_type: ActionType::CriticalStrike,
                damage: crit_damage,
                timestamp,
            });
            crit_damage
        } else {
            base_damage
        };
        
        // Check for combo bonus
        if let Some(combo_bonus) = self.combo_tracker.check_combo(base_damage) {
            final_damage = (final_damage * combo_bonus) / 100;
            self.battle_log.push(BattleAction {
                turn: self.turn_number,
                attacker,
                action_type: ActionType::ComboBonus,
                damage: final_damage,
                timestamp,
            });
        }
        
        // Apply defender's defense
        let effective_damage = defender_fighter.apply_defense(final_damage);
        
        // Apply damage to defender
        if attacker == self.fighter1 {
            self.fighter2_hp = self.fighter2_hp.saturating_sub(effective_damage);
        } else {
            self.fighter1_hp = self.fighter1_hp.saturating_sub(effective_damage);
        }
        
        // Log the action
        self.battle_log.push(BattleAction {
            turn: self.turn_number,
            attacker,
            action_type: ActionType::Strike,
            damage: effective_damage,
            timestamp,
        });
        
        // Check for winner
        if self.fighter1_hp == 0 {
            self.status = BattleStatus::Finished;
            self.winner = Some(self.fighter2);
        } else if self.fighter2_hp == 0 {
            self.status = BattleStatus::Finished;
            self.winner = Some(self.fighter1);
        } else {
            // Switch turns
            self.current_turn = if self.current_turn == self.fighter1 {
                self.fighter2
            } else {
                self.fighter1
            };
            self.turn_number += 1;
        }
        
        self.last_action_at = timestamp;
        Ok(effective_damage)
    }
    
    pub fn is_timed_out(&self, current_time: Timestamp, config: &BattleConfig) -> bool {
        current_time.delta_since(self.last_action_at) > config.turn_timeout
    }
}

/// Types of battles
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum BattleType {
    FreePlay,
    Staked,
    Tournament,
    Ranked,
}

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum BattleStatus {
    Active,
    Finished,
    Cancelled,
    TimedOut,
}

/// Tracks combos for bonus damage
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct ComboTracker {
    pub last_damage_range: Option<u32>,
    pub combo_count: u32,
}

impl ComboTracker {
    pub fn new() -> Self {
        Self {
            last_damage_range: None,
            combo_count: 0,
        }
    }
    
    /// Check if attack continues combo and return bonus multiplier
    pub fn check_combo(&mut self, damage: u32) -> Option<u32> {
        let damage_range = (damage / 10) * 10; // Round to nearest 10
        
        if let Some(last_range) = self.last_damage_range {
            if last_range == damage_range {
                self.combo_count += 1;
                let bonus = match self.combo_count {
                    2 => 150,  // 1.5x on second hit
                    3 => 200,  // 2x on third hit
                    _ => 200,  // Cap at 2x
                };
                self.last_damage_range = Some(damage_range);
                return Some(bonus);
            }
        }
        
        // Reset combo
        self.combo_count = 1;
        self.last_damage_range = Some(damage_range);
        None
    }
}

/// Individual action in battle log
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct BattleAction {
    pub turn: u32,
    pub attacker: AccountOwner,
    pub action_type: ActionType,
    pub damage: u32,
    pub timestamp: Timestamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum ActionType {
    Strike,
    CriticalStrike,
    ComboBonus,
    SpecialAbility,
}

/// Prediction pool for a battle
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct PredictionPool {
    pub battle_id: u64,
    pub predictions: HashMap<AccountOwner, Prediction>,
    pub total_pool: Amount,
    pub fighter1_pool: Amount,
    pub fighter2_pool: Amount,
    pub is_locked: bool,
}

impl PredictionPool {
    pub fn new(battle_id: u64) -> Self {
        Self {
            battle_id,
            predictions: HashMap::new(),
            total_pool: Amount::ZERO,
            fighter1_pool: Amount::ZERO,
            fighter2_pool: Amount::ZERO,
            is_locked: false,
        }
    }
    
    pub fn add_prediction(&mut self, predictor: AccountOwner, predicted_winner: AccountOwner, amount: Amount, fighter1: AccountOwner) {
        if self.is_locked {
            return;
        }
        
        self.predictions.insert(predictor, Prediction {
            predictor,
            predicted_winner,
            amount,
            claimed: false,
        });
        
        self.total_pool = self.total_pool.saturating_add(amount);
        
        if predicted_winner == fighter1 {
            self.fighter1_pool = self.fighter1_pool.saturating_add(amount);
        } else {
            self.fighter2_pool = self.fighter2_pool.saturating_add(amount);
        }
    }
    
    pub fn calculate_winnings(&self, predictor: &AccountOwner, winner: AccountOwner, fighter1: AccountOwner, platform_fee: u8) -> Amount {
        if let Some(prediction) = self.predictions.get(predictor) {
            if prediction.predicted_winner == winner && !prediction.claimed {
                let winning_pool = if winner == fighter1 {
                    self.fighter1_pool
                } else {
                    self.fighter2_pool
                };
                
                if winning_pool > Amount::ZERO {
                    let total_after_fee = self.total_pool.saturating_sub(
                        Amount::from_tokens((self.total_pool.saturating_mul(platform_fee as u128)) / 100)
                    );
                    
                    let share = (prediction.amount.saturating_mul(total_after_fee.into())) / winning_pool.into();
                    Amount::from_tokens(share)
                } else {
                    Amount::ZERO
                }
            } else {
                Amount::ZERO
            }
        } else {
            Amount::ZERO
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Prediction {
    pub predictor: AccountOwner,
    pub predicted_winner: AccountOwner,
    pub amount: Amount,
    pub claimed: bool,
}

/// Tournament structure
#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct Tournament {
    pub tournament_id: u64,
    pub name: String,
    pub entry_fee: Amount,
    pub prize_pool: Amount,
    pub start_time: Timestamp,
    pub max_participants: u32,
    pub participants: Vec<AccountOwner>,
    pub brackets: Vec<TournamentBracket>,
    pub status: TournamentStatus,
    pub winner: Option<AccountOwner>,
    pub prize_distribution: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
pub struct TournamentBracket {
    pub round: u32,
    pub matches: Vec<u64>, // Battle IDs
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Enum)]
pub enum TournamentStatus {
    Registration,
    InProgress,
    Finished,
    Cancelled,
}

/// Outcome of operations
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FighterOutcome {
    Success,
    BattleStarted { battle_id: u64 },
    StrikeExecuted { damage: u32, winner: Option<AccountOwner> },
    FighterRegistered { fighter: Fighter },
    LevelUp { new_level: u32 },
    TournamentCreated { tournament_id: u64 },
    Error { message: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    use linera_sdk::linera_base_types::Timestamp;

    #[test]
    fn test_fighter_creation_and_leveling() {
        let owner = AccountOwner::from([1u8; 32]);
        let timestamp = Timestamp::from(0);
        let mut fighter = Fighter::new(owner, "TestFighter".to_string(), timestamp);
        
        assert_eq!(fighter.level, 1);
        assert_eq!(fighter.xp, 0);
        assert_eq!(fighter.max_hp, 100);
        
        // Add enough XP to level up
        let leveled_up = fighter.add_xp(100);
        assert!(leveled_up);
        assert_eq!(fighter.level, 2);
        assert_eq!(fighter.max_hp, 110);
    }
    
    #[test]
    fn test_damage_calculation() {
        let owner = AccountOwner::from([1u8; 32]);
        let timestamp = Timestamp::from(0);
        let fighter = Fighter::new(owner, "TestFighter".to_string(), timestamp);
        
        let (min, max) = fighter.calculate_damage_range();
        assert_eq!(min, 12); // base_attack(10) + level(1) * 2
        assert_eq!(max, 15); // base_attack(10) + level(1) * 5
    }
    
    #[test]
    fn test_combo_tracker() {
        let mut tracker = ComboTracker::new();
        
        // First hit, no combo
        assert_eq!(tracker.check_combo(45), None);
        
        // Second hit in same range (40-49), combo!
        assert_eq!(tracker.check_combo(48), Some(150));
        
        // Third hit, bigger combo!
        assert_eq!(tracker.check_combo(42), Some(200));
        
        // Different range, combo breaks
        assert_eq!(tracker.check_combo(55), None);
    }
}
