use crate::state::{BattleState, BattleStatus, BattleParticipant, CombatStats, Stance, TurnSubmission, RoundResult, CombatAction};
use crate::{Message, Operation};
use crate::random::random_value;
use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ChainId, Timestamp},
    ContractRuntime,
};

const FP_SCALE: u128 = 1_000_000;

fn mul_fp(a: u128, b: u128) -> u128 {
    (a * b) / FP_SCALE
}

pub async fn handle_battle_operation(
    operation: Operation,
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
) {
    match operation {
        Operation::SubmitTurn { round, turn, stance, use_special } => {
            submit_turn(state, runtime, round, turn, stance, use_special).await;
        }
        Operation::ExecuteRound => {
            execute_3_rounds(state, runtime).await;
        }
        _ => {}
    }
}

pub async fn handle_battle_message(
    message: Message,
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
) {
    match message {
        Message::InitializeBattle { player1, player2, lobby_chain_id, platform_fee_bps, treasury_owner } => {
            initialize_battle(state, runtime, player1, player2, lobby_chain_id, platform_fee_bps, treasury_owner).await;
        }
        _ => {}
    }
}

async fn initialize_battle(
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
    player1: majorules::BattleParticipant,
    player2: majorules::BattleParticipant,
    lobby_chain_id: ChainId,
    platform_fee_bps: u16,
    treasury_owner: AccountOwner,
) {
    let sender_chain = runtime.message_origin_chain_id().expect("Message must have origin");
    assert_eq!(sender_chain, lobby_chain_id, "Only lobby can initialize battles");

    if state.player1.get().is_some() || state.player2.get().is_some() {
        return;
    }

    let convert_participant = |p: majorules::BattleParticipant| BattleParticipant {
        owner: p.owner,
        chain: p.chain,
        character: crate::state::CharacterSnapshot {
            nft_id: p.character.nft_id,
            class: match p.character.class {
                majorules::CharacterClass::Warrior => crate::state::CharacterClass::Warrior,
                majorules::CharacterClass::Assassin => crate::state::CharacterClass::Assassin,
                majorules::CharacterClass::Mage => crate::state::CharacterClass::Mage,
                majorules::CharacterClass::Tank => crate::state::CharacterClass::Tank,
                majorules::CharacterClass::Trickster => crate::state::CharacterClass::Trickster,
            },
            level: p.character.level,
            hp_max: p.character.hp_max,
            min_damage: p.character.min_damage,
            max_damage: p.character.max_damage,
            crit_chance: p.character.crit_chance,
            crit_multiplier: p.character.crit_multiplier,
            dodge_chance: p.character.dodge_chance,
            defense: p.character.defense,
            attack_bps: p.character.attack_bps,
            defense_bps: p.character.defense_bps,
            crit_bps: p.character.crit_bps,
        },
        stake: p.stake,
        current_hp: p.character.hp_max,
        combo_stack: 0,
        special_cooldown: 0,
        turns_submitted: [None, None, None],
    };

    state.player1.set(Some(convert_participant(player1)));
    state.player2.set(Some(convert_participant(player2)));
    state.status.set(BattleStatus::InProgress);
    state.current_round.set(1);
    state.max_rounds.set(10);
    state.winner.set(None);
    state.round_results.set(Vec::new());
    state.lobby_chain_id.set(Some(lobby_chain_id));
    state.platform_fee_bps.set(platform_fee_bps);
    state.treasury_owner.set(Some(treasury_owner));
    state.random_counter.set(0);
    state.started_at.set(Some(runtime.system_time()));
    state.completed_at.set(None);
}

async fn submit_turn(
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
    round: u8,
    turn: u8,
    stance: String,
    use_special: bool,
) {
    if *state.status.get() != BattleStatus::InProgress || round != *state.current_round.get() || turn >= 3 {
        return;
    }

    let caller = runtime.authenticated_signer().expect("Operation must be authenticated");
    let stance = match stance.as_str() {
        "Balanced" => Stance::Balanced,
        "Aggressive" => Stance::Aggressive,
        "Defensive" => Stance::Defensive,
        "Berserker" => Stance::Berserker,
        "Counter" => Stance::Counter,
        _ => return,
    };

    let turn_key = (caller, turn);
    
    // Prevent double submission
    if state.turn_submissions.contains_key(&turn_key).await.unwrap_or(false) {
        return;
    }

    // Store turn submission
    state.turn_submissions.insert(&turn_key, TurnSubmission { round, turn, stance, use_special })
        .expect("Failed to store turn submission");

    // Check if both players submitted this turn
    let (p1, p2) = (state.player1.get().clone(), state.player2.get().clone());
    if let (Some(player1), Some(player2)) = (p1, p2) {
        let p1_key = (player1.owner, turn);
        let p2_key = (player2.owner, turn);
        
        let p1_submitted = state.turn_submissions.contains_key(&p1_key).await.unwrap_or(false);
        let p2_submitted = state.turn_submissions.contains_key(&p2_key).await.unwrap_or(false);
        
        // Auto-execute turn when both players submit
        if p1_submitted && p2_submitted {
            execute_single_turn(state, runtime, turn).await;
        }
    }
}

async fn execute_single_turn(
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
    turn: u8,
) {
    if *state.status.get() != BattleStatus::InProgress {
        return;
    }

    let (p1, p2) = (state.player1.get().clone(), state.player2.get().clone());
    if let (Some(player1), Some(player2)) = (p1, p2) {
        let p1_key = (player1.owner, turn);
        let p2_key = (player2.owner, turn);
        
        let p1_turn = state.turn_submissions.get(&p1_key).await.ok().flatten();
        let p2_turn = state.turn_submissions.get(&p2_key).await.ok().flatten();
        
        if let (Some(p1_submission), Some(p2_submission)) = (p1_turn, p2_turn) {
            let mut p1_mut = player1.clone();
            let mut p2_mut = player2.clone();
            
            // Execute combat for this turn
            if p1_mut.current_hp > 0 && p2_mut.current_hp > 0 {
                execute_attack(state, &mut p1_mut, &mut p2_mut, &p1_submission, p2_submission.stance).ok();
            }
            if p2_mut.current_hp > 0 && p1_mut.current_hp > 0 {
                execute_attack(state, &mut p2_mut, &mut p1_mut, &p2_submission, p1_submission.stance).ok();
            }

            // Update player states
            state.player1.set(Some(p1_mut.clone()));
            state.player2.set(Some(p2_mut.clone()));

            // Check if battle ends
            if p1_mut.current_hp == 0 || p2_mut.current_hp == 0 {
                let winner = if p1_mut.current_hp > 0 { p1_mut.owner } else { p2_mut.owner };
                let loser = if winner == p1_mut.owner { p2_mut.owner } else { p1_mut.owner };
                finalize_battle(state, runtime, winner, loser).await;
            }
        }
    }
}

async fn execute_3_rounds(
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
) {
    if *state.status.get() != BattleStatus::InProgress {
        return;
    }

    let caller = runtime.authenticated_signer().expect("Operation must be authenticated");
    let (p1, p2) = (state.player1.get().clone(), state.player2.get().clone());
    
    let is_participant = if let (Some(ref player1), Some(ref player2)) = (p1, p2) {
        caller == player1.owner || caller == player2.owner
    } else {
        false
    };

    if !is_participant {
        return;
    }

    let current_round = *state.current_round.get();
    let execute_key = format!("execute_3_rounds_{}_{}", current_round, caller);
    let mut log = state.battle_log.get().clone();
    
    // Prevent double execution
    if log.iter().any(|entry| entry.contains(&execute_key)) {
        return;
    }
    
    log.push(execute_key.clone());
    state.battle_log.set(log.clone());

    // Check if both players called execute
    let p1 = state.player1.get().clone().unwrap();
    let p2 = state.player2.get().clone().unwrap();
    let p1_execute_key = format!("execute_3_rounds_{}_{}", current_round, p1.owner);
    let p2_execute_key = format!("execute_3_rounds_{}_{}", current_round, p2.owner);
    
    let p1_wants_execute = log.iter().any(|entry| entry.contains(&p1_execute_key));
    let p2_wants_execute = log.iter().any(|entry| entry.contains(&p2_execute_key));
    
    // Only execute when both players call it
    if p1_wants_execute && p2_wants_execute {
        // Store round result
        let round_result = RoundResult {
            round: current_round,
            player1_actions: Vec::new(),
            player2_actions: Vec::new(),
            player1_hp: p1.current_hp,
            player2_hp: p2.current_hp,
        };
        
        let mut results = state.round_results.get().clone();
        results.push(round_result);
        state.round_results.set(results);

        // Clear turn submissions
        for turn in 0..3 {
            state.turn_submissions.remove(&(p1.owner, turn)).ok();
            state.turn_submissions.remove(&(p2.owner, turn)).ok();
        }

        // Check battle completion or advance round
        if p1.current_hp == 0 || p2.current_hp == 0 {
            let winner = if p1.current_hp > 0 { p1.owner } else { p2.owner };
            let loser = if winner == p1.owner { p2.owner } else { p1.owner };
            finalize_battle(state, runtime, winner, loser).await;
        } else if current_round >= *state.max_rounds.get() {
            let winner = if p1.current_hp > p2.current_hp { p1.owner } else { p2.owner };
            let loser = if winner == p1.owner { p2.owner } else { p1.owner };
            finalize_battle(state, runtime, winner, loser).await;
        } else {
            state.current_round.set(current_round + 1);
        }
    }
}

fn execute_attack(
    state: &mut BattleState,
    attacker: &mut BattleParticipant,
    defender: &mut BattleParticipant,
    attacker_turn: &TurnSubmission,
    defender_stance: Stance,
) -> Result<CombatAction, String> {
    let attacker_owner = attacker.owner;
    let defender_owner = defender.owner;

    // Use special ability
    let special_used = if attacker_turn.use_special && attacker.special_cooldown == 0 {
        attacker.special_cooldown = 3;
        true
    } else {
        false
    };

    // Calculate damage
    let (damage, was_crit, was_dodged) = calculate_damage(attacker, defender, attacker_turn.stance, defender_stance, special_used)?;

    let mut was_countered = false;

    // Berserker self-damage
    if attacker_turn.stance == Stance::Berserker && !was_dodged {
        attacker.current_hp = attacker.current_hp.saturating_sub(damage / 4);
    }

    // Apply damage
    if !was_dodged {
        defender.current_hp = defender.current_hp.saturating_sub(damage);
    }

    // Handle combos
    if was_crit && attacker.combo_stack < 5 {
        attacker.combo_stack += 1;
    } else if was_dodged {
        attacker.combo_stack = 0;
    }

    // Counter-attack
    if defender_stance == Stance::Counter && !was_dodged && defender.current_hp > 0 {
        if random_value(0, 9999) < 4000 {
            was_countered = true;
            attacker.current_hp = attacker.current_hp.saturating_sub(damage * 4 / 10);
        }
    }

    // Tick cooldowns
    if attacker.special_cooldown > 0 { attacker.special_cooldown -= 1; }
    if defender.special_cooldown > 0 { defender.special_cooldown -= 1; }

    state.random_counter.set(state.random_counter.get() + 1);

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

fn calculate_damage(
    attacker: &BattleParticipant,
    defender: &BattleParticipant,
    attacker_stance: Stance,
    defender_stance: Stance,
    special_used: bool,
) -> Result<(u32, bool, bool), String> {
    let char = &attacker.character;
    let base_damage = random_value(char.min_damage as u64, char.max_damage as u64) as u32;
    let mut damage = base_damage as u128 * FP_SCALE;

    // Apply attack traits
    if char.attack_bps != 0 {
        let attack_mod = FP_SCALE as i128 + ((char.attack_bps as i128 * FP_SCALE as i128) / 10000);
        damage = ((damage as i128 * attack_mod) / FP_SCALE as i128) as u128;
    }

    // Stance modifiers
    damage = match attacker_stance {
        Stance::Balanced => damage,
        Stance::Aggressive => mul_fp(damage, 13 * FP_SCALE / 10),
        Stance::Defensive => mul_fp(damage, 7 * FP_SCALE / 10),
        Stance::Berserker => mul_fp(damage, 2 * FP_SCALE),
        Stance::Counter => mul_fp(damage, 9 * FP_SCALE / 10),
    };

    // Combo bonus
    if attacker.combo_stack > 0 {
        let combo_bonus = FP_SCALE + (attacker.combo_stack as u128 * FP_SCALE / 20);
        damage = mul_fp(damage, combo_bonus);
    }

    // Critical hit
    let crit_roll = random_value(0, 9999);
    let crit_chance = char.crit_chance + char.crit_bps.max(0) as u16;
    let was_crit = crit_roll < crit_chance as u64;
    if was_crit {
        let crit_mult = char.crit_multiplier as u128 * FP_SCALE / 10000;
        damage = mul_fp(damage, crit_mult);
    }

    // Special ability
    if special_used {
        damage = mul_fp(damage, 15 * FP_SCALE / 10);
    }

    // Dodge check
    let dodge_roll = random_value(0, 9999);
    let was_dodged = dodge_roll < defender.character.dodge_chance as u64;
    if was_dodged {
        return Ok((0, was_crit, true));
    }

    // Defense
    let def_reduction = defender.character.defense as u128 * FP_SCALE / 100;
    if def_reduction < FP_SCALE {
        damage = mul_fp(damage, FP_SCALE - def_reduction);
    } else {
        damage = FP_SCALE;
    }

    // Defender stance
    damage = match defender_stance {
        Stance::Balanced => damage,
        Stance::Aggressive => mul_fp(damage, 15 * FP_SCALE / 10),
        Stance::Defensive => mul_fp(damage, 5 * FP_SCALE / 10),
        Stance::Berserker => damage,
        Stance::Counter => mul_fp(damage, 6 * FP_SCALE / 10),
    };

    // Defense traits
    if defender.character.defense_bps != 0 {
        let def_mod = FP_SCALE as i128 - ((defender.character.defense_bps as i128 * FP_SCALE as i128) / 10000);
        if def_mod > 0 {
            damage = ((damage as i128 * def_mod) / FP_SCALE as i128) as u128;
        } else {
            damage = FP_SCALE;
        }
    }

    let final_damage = ((damage / FP_SCALE) as u32).max(1);
    Ok((final_damage, was_crit, false))
}

async fn finalize_battle(
    state: &mut BattleState,
    runtime: &mut ContractRuntime<crate::MajorulesContract>,
    winner: AccountOwner,
    loser: AccountOwner,
) {
    state.winner.set(Some(winner));
    state.status.set(BattleStatus::Completed);
    state.completed_at.set(Some(runtime.system_time()));

    let (p1, p2) = (state.player1.get().clone().unwrap(), state.player2.get().clone().unwrap());
    let total_stake = p1.stake.saturating_add(p2.stake);
    let platform_fee_bps = *state.platform_fee_bps.get();
    let platform_fee_amount = (u128::from(total_stake) * platform_fee_bps as u128) / 10000;
    let platform_fee = Amount::from_attos(platform_fee_amount);
    let winner_payout = total_stake.saturating_sub(platform_fee);

    // Calculate stats
    let round_results = state.round_results.get().clone();
    let (winner_stats, loser_stats) = calculate_combat_stats(&round_results, &winner);

    // Send results to lobby
    if let Some(lobby_chain) = state.lobby_chain_id.get().as_ref() {
        let convert_stats = |stats: &CombatStats| majorules::CombatStats {
            damage_dealt: stats.damage_dealt,
            damage_taken: stats.damage_taken,
            crits: stats.crits,
            dodges: stats.dodges,
            highest_crit: stats.highest_crit,
        };

        let battle_chain = runtime.chain_id();

        // Winner result
        runtime.prepare_message(Message::BattleResult {
            winner, loser, winner_payout, xp_gained: 150,
            battle_stats: convert_stats(&winner_stats),
            battle_chain,
        }).with_authentication().send_to(*lobby_chain);

        // Loser result
        runtime.prepare_message(Message::BattleResult {
            winner, loser, winner_payout: Amount::ZERO, xp_gained: 50,
            battle_stats: convert_stats(&loser_stats),
            battle_chain,
        }).with_authentication().send_to(*lobby_chain);

        // Completion notification
        runtime.prepare_message(Message::BattleCompleted {
            winner, loser, rounds_played: *state.current_round.get(), total_stake,
            battle_stats: (convert_stats(&winner_stats), convert_stats(&loser_stats)),
        }).with_authentication().send_to(*lobby_chain);
    }
}

fn calculate_combat_stats(round_results: &[RoundResult], winner: &AccountOwner) -> (CombatStats, CombatStats) {
    let mut winner_stats = CombatStats { damage_dealt: 0, damage_taken: 0, crits: 0, dodges: 0, highest_crit: 0 };
    let mut loser_stats = CombatStats { damage_dealt: 0, damage_taken: 0, crits: 0, dodges: 0, highest_crit: 0 };

    for round in round_results {
        for actions in [&round.player1_actions, &round.player2_actions] {
            for action in actions {
                let (attacker_stats, defender_stats) = if &action.attacker == winner {
                    (&mut winner_stats, &mut loser_stats)
                } else {
                    (&mut loser_stats, &mut winner_stats)
                };

                if !action.was_dodged {
                    attacker_stats.damage_dealt += action.damage as u64;
                    defender_stats.damage_taken += action.damage as u64;
                }
                if action.was_crit {
                    attacker_stats.crits += 1;
                    if action.damage as u64 > attacker_stats.highest_crit {
                        attacker_stats.highest_crit = action.damage as u64;
                    }
                }
                if action.was_dodged {
                    defender_stats.dodges += 1;
                }
            }
        }
    }

    (winner_stats, loser_stats)
}