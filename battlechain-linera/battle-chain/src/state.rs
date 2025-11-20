use battlechain_shared_types::{
    derive_random_u64, mul_fp, random_in_range,
    FP_SCALE, MAX_COMBO_STACK, Owner, Stance,
};
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{RegisterView, RootView, ViewStorageContext},
};
use serde::{Deserialize, Serialize};

use crate::{BattleError, BattleParticipant, CombatAction, RoundResult, TurnSubmission};

/// Battle chain state
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct BattleState {
    /// Battle participants
    pub player1: RegisterView<Option<BattleParticipant>>,
    pub player2: RegisterView<Option<BattleParticipant>>,

    /// Battle metadata
    pub status: RegisterView<BattleStatus>,
    pub current_round: RegisterView<u8>,
    pub max_rounds: RegisterView<u8>,
    pub winner: RegisterView<Option<Owner>>,

    /// Round results history
    pub round_results: RegisterView<Vec<RoundResult>>,

    /// Battle log for tracking events
    pub battle_log: RegisterView<Vec<String>>,

    /// Randomness counter for generating unique seeds per action
    pub random_counter: RegisterView<u64>,

    /// Application references
    pub battle_token_app: RegisterView<Option<ApplicationId>>,
    pub matchmaking_chain: RegisterView<Option<ChainId>>,
    pub prediction_chain_id: RegisterView<Option<ChainId>>,

    /// Platform fee (basis points, 300 = 3%)
    pub platform_fee_bps: RegisterView<u16>,
    pub treasury_owner: RegisterView<Option<Owner>>,

    /// Timestamps
    pub started_at: RegisterView<Option<Timestamp>>,
    pub completed_at: RegisterView<Option<Timestamp>>,
}

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BattleStatus {
    #[default]
    WaitingForPlayers,
    InProgress,
    Completed,
}

impl BattleState {
    /// Generate random seed from timestamp and counter
    pub fn generate_random_seed(&mut self, timestamp: Timestamp) -> [u8; 32] {
        let counter = *self.random_counter.get();
        self.random_counter.set(counter + 1);

        // Combine timestamp and counter for unique seed
        let time_micros = timestamp.micros();
        let mut seed = [0u8; 32];

        // Fill seed with timestamp and counter bytes
        seed[0..8].copy_from_slice(&time_micros.to_le_bytes());
        seed[8..16].copy_from_slice(&counter.to_le_bytes());

        // Fill rest with combination of both for extra entropy
        let combined = time_micros.wrapping_add(counter);
        seed[16..24].copy_from_slice(&combined.to_le_bytes());
        seed[24..32].copy_from_slice(&(time_micros ^ counter).to_le_bytes());

        seed
    }

    /// Get next random value
    pub fn next_random(&mut self, timestamp: Timestamp) -> u64 {
        let seed = self.generate_random_seed(timestamp);
        let tag = (*self.random_counter.get() % 256) as u8;
        derive_random_u64(&seed, tag)
    }

    /// Get participant by owner (returns cloned participant)
    pub fn get_participant(&self, owner: &Owner) -> Result<BattleParticipant, BattleError> {
        let p1 = self.player1.get().as_ref().ok_or(BattleError::NotInitialized)?;
        let p2 = self.player2.get().as_ref().ok_or(BattleError::NotInitialized)?;

        if p1.owner == *owner {
            Ok(p1.clone())
        } else if p2.owner == *owner {
            Ok(p2.clone())
        } else {
            Err(BattleError::NotParticipant)
        }
    }

    /// Calculate damage for an attack
    pub fn calculate_damage(
        &mut self,
        attacker: &BattleParticipant,
        defender: &BattleParticipant,
        attacker_stance: Stance,
        defender_stance: Stance,
        special_used: bool,
        timestamp: Timestamp,
    ) -> Result<(u32, bool, bool), BattleError> {
        let char = &attacker.character;

        // Base damage (random in range)
        let seed = self.generate_random_seed(timestamp);
        let base_damage = random_in_range(
            &seed,
            0,
            char.min_damage as u64,
            char.max_damage as u64,
        ) as u32;

        let mut damage = base_damage as u128 * FP_SCALE;

        // Apply attack traits (basis points)
        if char.attack_bps != 0 {
            let attack_mod = FP_SCALE as i128 + ((char.attack_bps as i128 * FP_SCALE as i128) / 10000);
            damage = ((damage as i128 * attack_mod) / FP_SCALE as i128) as u128;
        }

        // Apply stance modifiers
        damage = match attacker_stance {
            Stance::Balanced => damage,
            Stance::Aggressive => mul_fp(damage, 13 * FP_SCALE / 10), // 130%
            Stance::Defensive => mul_fp(damage, 7 * FP_SCALE / 10),   // 70%
            Stance::Berserker => mul_fp(damage, 2 * FP_SCALE),         // 200%
            Stance::Counter => mul_fp(damage, 9 * FP_SCALE / 10),     // 90%
        };

        // Apply combo bonus (5% per stack)
        if attacker.combo_stack > 0 {
            let combo_bonus = FP_SCALE + (attacker.combo_stack as u128 * FP_SCALE / 20);
            damage = mul_fp(damage, combo_bonus);
        }

        // Check for critical hit
        let crit_roll = self.next_random(timestamp) % 10000;
        let crit_chance = char.crit_chance + char.crit_bps.max(0) as u16;
        let mut was_crit = false;

        if crit_roll < crit_chance as u64 {
            was_crit = true;
            let crit_mult = char.crit_multiplier as u128 * FP_SCALE / 10000;
            damage = mul_fp(damage, crit_mult);
        }

        // Apply special ability multiplier
        if special_used {
            damage = mul_fp(damage, 15 * FP_SCALE / 10); // 150%
        }

        // Check for dodge
        let dodge_roll = self.next_random(timestamp) % 10000;
        let was_dodged = dodge_roll < defender.character.dodge_chance as u64;

        if was_dodged {
            return Ok((0, was_crit, true));
        }

        // Apply defender's defense
        let def_reduction = defender.character.defense as u128 * FP_SCALE / 100;
        if def_reduction < FP_SCALE {
            damage = mul_fp(damage, FP_SCALE - def_reduction);
        } else {
            damage = FP_SCALE; // Minimum 1 damage
        }

        // Apply defender stance modifiers
        damage = match defender_stance {
            Stance::Balanced => damage,
            Stance::Aggressive => mul_fp(damage, 15 * FP_SCALE / 10), // 150% (take more)
            Stance::Defensive => mul_fp(damage, 5 * FP_SCALE / 10),   // 50% (take less)
            Stance::Berserker => damage,
            Stance::Counter => mul_fp(damage, 6 * FP_SCALE / 10),     // 60% (take less)
        };

        // Apply defender defense traits
        if defender.character.defense_bps != 0 {
            let def_mod = FP_SCALE as i128 - ((defender.character.defense_bps as i128 * FP_SCALE as i128) / 10000);
            if def_mod > 0 {
                damage = ((damage as i128 * def_mod) / FP_SCALE as i128) as u128;
            } else {
                damage = FP_SCALE;
            }
        }

        let final_damage = (damage / FP_SCALE) as u32;
        let final_damage = final_damage.max(1); // Minimum 1 damage

        Ok((final_damage, was_crit, false))
    }

    /// Execute a single turn
    pub fn execute_turn(
        &mut self,
        attacker: &mut BattleParticipant,
        defender: &mut BattleParticipant,
        attacker_turn: &TurnSubmission,
        defender_stance: Stance,
        timestamp: Timestamp,
    ) -> Result<CombatAction, BattleError> {
        let attacker_owner = attacker.owner;
        let defender_owner = defender.owner;

        // Use special ability if requested and available
        let special_used = if attacker_turn.use_special {
            attacker.use_special()
        } else {
            false
        };

        // Calculate damage
        let (damage, was_crit, was_dodged) = self.calculate_damage(
            attacker,
            defender,
            attacker_turn.stance,
            defender_stance,
            special_used,
            timestamp,
        )?;

        let mut was_countered = false;

        // Handle berserker self-damage
        if attacker_turn.stance == Stance::Berserker && !was_dodged {
            let self_damage = damage / 4; // 25% self-damage
            attacker.take_damage(self_damage);
        }

        // Apply damage to defender
        let defeated = if !was_dodged {
            defender.take_damage(damage)
        } else {
            false
        };

        // Handle combo stacks
        if was_crit {
            attacker.add_combo();
        } else if was_dodged {
            attacker.reset_combo();
        }

        // Counter-attack for Counter stance
        if defender_stance == Stance::Counter && !was_dodged && !defeated {
            let counter_roll = self.next_random(timestamp) % 10000;
            if counter_roll < 4000 {
                // 40% counter chance
                was_countered = true;
                let counter_damage = damage * 4 / 10; // 40% of original damage
                attacker.take_damage(counter_damage);
            }
        }

        // Tick cooldowns
        attacker.tick_cooldown();
        defender.tick_cooldown();

        Ok(CombatAction {
            attacker: attacker_owner,
            defender: defender_owner,
            damage,
            was_crit,
            was_dodged,
            was_countered,
            special_used,
            defender_hp_remaining: defender.current_hp,
        })
    }

    /// Execute full round (all 3 turns for both players)
    pub fn execute_full_round(&mut self, timestamp: Timestamp) -> Result<RoundResult, BattleError> {
        let mut p1 = self.player1.get().clone().ok_or(BattleError::NotInitialized)?;
        let mut p2 = self.player2.get().clone().ok_or(BattleError::NotInitialized)?;

        let mut player1_actions = Vec::new();
        let mut player2_actions = Vec::new();

        // Execute 3 turns
        for turn in 0..3 {
            let p1_turn = p1.turns_submitted[turn].clone().unwrap();
            let p2_turn = p2.turns_submitted[turn].clone().unwrap();
            let p1_stance = p1_turn.stance;
            let p2_stance = p2_turn.stance;

            // Player 1 attacks
            if p1.current_hp > 0 && p2.current_hp > 0 {
                let action = self.execute_turn(&mut p1, &mut p2, &p1_turn, p2_stance, timestamp)?;
                player1_actions.push(action);
            }

            // Player 2 attacks
            if p2.current_hp > 0 && p1.current_hp > 0 {
                let action = self.execute_turn(&mut p2, &mut p1, &p2_turn, p1_stance, timestamp)?;
                player2_actions.push(action);
            }

            // Check for KO
            if p1.current_hp == 0 || p2.current_hp == 0 {
                break;
            }
        }

        let round_result = RoundResult {
            round: *self.current_round.get(),
            player1_actions,
            player2_actions,
            player1_hp: p1.current_hp,
            player2_hp: p2.current_hp,
        };

        // Restore players
        self.player1.set(Some(p1));
        self.player2.set(Some(p2));

        Ok(round_result)
    }
}
