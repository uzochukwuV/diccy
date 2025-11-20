use async_graphql::{Request, Response, Schema, EmptyMutation, EmptySubscription, SimpleObject};
use battlechain_shared_events::{BattleEvent, CombatStats};
use battlechain_shared_types::{
    derive_random_u64, mul_fp, random_in_range, CharacterSnapshot,
    FP_SCALE, MAX_COMBO_STACK, Owner, Stance,
};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{RegisterView, RootView, View, ViewStorageContext},
    Contract, Service, ContractRuntime, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Battle Token ABI for cross-application calls
// Defined inline to avoid circular dependencies (battle-token is cdylib)
pub struct BattleTokenAbi;

impl ContractAbi for BattleTokenAbi {
    type Operation = BattleTokenOperation;
    type Response = ();
}

/// Battle token operations (subset needed for calls)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BattleTokenOperation {
    Transfer { to: Owner, amount: Amount },
}

/// Battle Chain Application ABI
pub struct BattleChainAbi;

impl ContractAbi for BattleChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for BattleChainAbi {
    type Query = Request;
    type QueryResponse = Response;
}

// BattleEvent is now imported from battlechain-shared-events

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BattleStatus {
    #[default]
    WaitingForPlayers,
    InProgress,
    Completed,
}

/// Turn submission by a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSubmission {
    pub round: u8,
    pub turn: u8,
    pub stance: Stance,
    pub use_special: bool,
}

/// Combat action result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatAction {
    pub attacker: Owner,
    pub defender: Owner,
    pub damage: u32,
    pub was_crit: bool,
    pub was_dodged: bool,
    pub was_countered: bool,
    pub special_used: bool,
    pub defender_hp_remaining: u32,
}

/// Round result with all actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResult {
    pub round: u8,
    pub player1_actions: Vec<CombatAction>,
    pub player2_actions: Vec<CombatAction>,
    pub player1_hp: u32,
    pub player2_hp: u32,
}

/// Battle participant state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParticipant {
    pub owner: Owner,
    pub chain: ChainId,
    pub character: CharacterSnapshot,
    pub stake: Amount,

    // Combat state
    pub current_hp: u32,
    pub combo_stack: u8,
    pub special_cooldown: u8,

    // Turn submissions per round
    pub turns_submitted: [Option<TurnSubmission>; 3], // 3 turns per round
}

impl BattleParticipant {
    pub fn new(owner: Owner, chain: ChainId, character: CharacterSnapshot, stake: Amount) -> Self {
        let current_hp = character.hp_max;

        Self {
            owner,
            chain,
            character,
            stake,
            current_hp,
            combo_stack: 0,
            special_cooldown: 0,
            turns_submitted: [None, None, None],
        }
    }

    /// Reset turn submissions for new round
    pub fn reset_turns(&mut self) {
        self.turns_submitted = [None, None, None];
    }

    /// Check if all turns submitted for current round
    pub fn all_turns_submitted(&self) -> bool {
        self.turns_submitted[0].is_some()
            && self.turns_submitted[1].is_some()
            && self.turns_submitted[2].is_some()
    }

    /// Decrease special ability cooldown
    pub fn tick_cooldown(&mut self) {
        if self.special_cooldown > 0 {
            self.special_cooldown -= 1;
        }
    }

    /// Use special ability
    pub fn use_special(&mut self) -> bool {
        if self.special_cooldown == 0 {
            self.special_cooldown = self.character.class.special_cooldown();
            true
        } else {
            false
        }
    }

    /// Take damage and return if defeated
    pub fn take_damage(&mut self, damage: u32) -> bool {
        self.current_hp = self.current_hp.saturating_sub(damage);
        self.current_hp == 0
    }

    /// Increase combo stack
    pub fn add_combo(&mut self) {
        if self.combo_stack < MAX_COMBO_STACK {
            self.combo_stack += 1;
        }
    }

    /// Reset combo stack
    pub fn reset_combo(&mut self) {
        self.combo_stack = 0;
    }
}

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

/// Operations for Battle chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Submit turn for current round
    SubmitTurn {
        round: u8,
        turn: u8,
        stance: Stance,
        use_special: bool,
    },

    /// Execute current round (when all turns submitted)
    ExecuteRound,

    /// Finalize battle and distribute rewards
    FinalizeBattle,
}

/// Messages sent TO and FROM Battle chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Initialize battle after auto-deployment (FIRST message received)
    /// This triggers automatic deployment of battle application to new chain
    Initialize {
        player1: BattleParticipant,
        player2: BattleParticipant,
        matchmaking_chain: ChainId,
        battle_token_app: ApplicationId,
        prediction_chain_id: Option<ChainId>,
        platform_fee_bps: u16,
        treasury_owner: Owner,
    },

    /// Notify player of battle result
    BattleResult {
        winner: Owner,
        loser: Owner,
        winner_payout: Amount,
        rounds_played: u8,
    },

    /// Notify matchmaking of completion
    BattleCompleted {
        winner: Owner,
        loser: Owner,
    },

    /// Notify prediction market that battle has started (close betting)
    BattleStarted {
        battle_chain: ChainId,
    },

    /// Notify prediction market of battle result (settle market)
    BattleEnded {
        battle_chain: ChainId,
        winner_chain: ChainId,
    },
}

/// Battle initialization parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParameters {
    pub player1_owner: Owner,
    pub player1_chain: ChainId,
    pub player1_character: CharacterSnapshot,
    pub player1_stake: Amount,
    pub player2_owner: Owner,
    pub player2_chain: ChainId,
    pub player2_character: CharacterSnapshot,
    pub player2_stake: Amount,
    pub battle_token_app: ApplicationId,
    pub matchmaking_chain: ChainId,
    pub platform_fee_bps: u16,
    pub treasury_owner: Owner,
}

/// Battle errors
#[derive(Debug, Error)]
pub enum BattleError {
    #[error("Battle not initialized")]
    NotInitialized,

    #[error("Battle already in progress")]
    AlreadyStarted,

    #[error("Battle already completed")]
    AlreadyCompleted,

    #[error("Not a participant")]
    NotParticipant,

    #[error("Invalid round: {0}")]
    InvalidRound(u8),

    #[error("Invalid turn: {0}")]
    InvalidTurn(u8),

    #[error("Turn already submitted")]
    TurnAlreadySubmitted,

    #[error("Not all turns submitted")]
    NotAllTurnsSubmitted,

    #[error("Player defeated")]
    PlayerDefeated,

    #[error("Invalid stake: {0}")]
    InvalidStake(String),

    #[error("Invalid platform fee: {0} bps (must be 0-10000)")]
    InvalidPlatformFee(u16),

    #[error("Invalid max rounds: {0} (must be 1-100)")]
    InvalidMaxRounds(u8),

    #[error("Unauthorized message sender")]
    UnauthorizedSender,

    #[error("Contract is paused")]
    ContractPaused,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for BattleError {
    fn from(error: linera_sdk::views::ViewError) -> Self {
        BattleError::ViewError(error.to_string())
    }
}

/// Battle Contract
pub struct BattleContract {
    state: BattleState,
    runtime: ContractRuntime<Self>,
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
                        self.runtime.call_application::<BattleTokenAbi>(
                            true, // authenticated call
                            battle_token_app.with_abi(),
                            &BattleTokenOperation::Transfer {
                                to: *treasury_owner,
                                amount: platform_fee,
                            },
                        );

                        log::info!(
                            "Transferred platform fee {} to treasury {:?}",
                            platform_fee,
                            treasury_owner
                        );
                    }

                    // Transfer winner payout to winner
                    if winner_payout > Amount::ZERO {
                        self.runtime.call_application::<BattleTokenAbi>(
                            true, // authenticated call
                            battle_token_app.with_abi(),
                            &BattleTokenOperation::Transfer {
                                to: winner_owner,
                                amount: winner_payout,
                            },
                        );

                        log::info!(
                            "Transferred winner payout {} to {:?}",
                            winner_payout,
                            winner_owner
                        );
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

/// Battle Service
pub struct BattleService {
    state: BattleState,
}

impl WithServiceAbi for BattleService {
    type Abi = BattleChainAbi;
}

impl Service for BattleService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = BattleState::load(runtime.root_view_storage_context())
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
    status: BattleStatus,
    current_round: u8,
    max_rounds: u8,
    round_count: usize,
    player1_hp: u32,
    player2_hp: u32,
}

impl QueryRoot {
    async fn new(state: &BattleState) -> Self {
        let (p1_hp, p2_hp) = if let (Some(p1), Some(p2)) = (state.player1.get(), state.player2.get()) {
            (p1.current_hp, p2.current_hp)
        } else {
            (0, 0)
        };

        Self {
            status: *state.status.get(),
            current_round: *state.current_round.get(),
            max_rounds: *state.max_rounds.get(),
            round_count: state.round_results.get().len(),
            player1_hp: p1_hp,
            player2_hp: p2_hp,
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    /// Get battle status
    async fn status(&self) -> String {
        format!("{:?}", self.status)
    }

    /// Get current round number
    async fn current_round(&self) -> i32 {
        self.current_round as i32
    }

    /// Get maximum rounds
    async fn max_rounds(&self) -> i32 {
        self.max_rounds as i32
    }

    /// Get completed round count
    async fn completed_rounds(&self) -> i32 {
        self.round_count as i32
    }

    /// Get player 1 HP
    async fn player1_hp(&self) -> i32 {
        self.player1_hp as i32
    }

    /// Get player 2 HP
    async fn player2_hp(&self) -> i32 {
        self.player2_hp as i32
    }

    /// Get battle info
    async fn battle_info(&self) -> BattleInfo {
        BattleInfo {
            status: format!("{:?}", self.status),
            current_round: self.current_round as i32,
            completed_rounds: self.round_count as i32,
            player1_hp: self.player1_hp as i32,
            player2_hp: self.player2_hp as i32,
        }
    }
}

#[derive(SimpleObject)]
struct BattleInfo {
    status: String,
    current_round: i32,
    completed_rounds: i32,
    player1_hp: i32,
    player2_hp: i32,
}
