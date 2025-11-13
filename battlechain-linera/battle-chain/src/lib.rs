use async_trait::async_trait;
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{RootView, ViewStorageContext},
    Contract, ContractRuntime, Service, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use shared_types::{
    derive_random_u64, mul_fp, random_in_range, CharacterClass, CharacterSnapshot, EntropySeed,
    FP_SCALE, MAX_COMBO_STACK, Owner, Stance,
};
use thiserror::Error;

/// Battle chain ABI
pub struct BattleChainAbi;

impl linera_sdk::abi::ContractAbi for BattleChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl linera_sdk::abi::ServiceAbi for BattleChainAbi {
    type Query = ();
    type QueryResponse = ();
}

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleStatus {
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

/// Battle chain state (multi-owner chain)
#[derive(RootView)]
pub struct BattleState {
    /// Battle participants
    pub player1: Option<BattleParticipant>,
    pub player2: Option<BattleParticipant>,

    /// Battle metadata
    pub status: BattleStatus,
    pub current_round: u8,
    pub max_rounds: u8,
    pub winner: Option<Owner>,

    /// Round results history
    pub round_results: Vec<RoundResult>,

    /// Entropy for randomness
    pub entropy_seed: Option<EntropySeed>,
    pub entropy_index: u64,

    /// Application references
    pub battle_token_app: ApplicationId,
    pub matchmaking_chain: ChainId,

    /// Platform fee (basis points, 300 = 3%)
    pub platform_fee_bps: u16,
    pub treasury_owner: Owner,

    /// Timestamps
    pub started_at: Option<Timestamp>,
    pub completed_at: Option<Timestamp>,
}

/// Operations for Battle chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Initialize battle with both players
    Initialize {
        player1_owner: Owner,
        player1_chain: ChainId,
        player1_character: CharacterSnapshot,
        player1_stake: Amount,
        player2_owner: Owner,
        player2_chain: ChainId,
        player2_character: CharacterSnapshot,
        player2_stake: Amount,
        battle_token_app: ApplicationId,
        matchmaking_chain: ChainId,
        entropy_seed: [u8; 32],
    },

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

/// Messages sent from Battle chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
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
}

/// Battle errors
#[derive(Debug, Error, Serialize, Deserialize)]
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

    #[error("Entropy not initialized")]
    EntropyNotInitialized,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for BattleError {
    fn from(error: linera_sdk::views::ViewError) -> Self {
        BattleError::ViewError(error.to_string())
    }
}

pub struct BattleContract {
    state: BattleState,
    runtime: ContractRuntime,
}

impl BattleState {
    /// Get next entropy value
    pub fn next_random(&mut self) -> Result<u64, BattleError> {
        let entropy = self.entropy_seed.as_ref().ok_or(BattleError::EntropyNotInitialized)?;
        let value = derive_random_u64(&entropy.seed, (self.entropy_index % 256) as u8);
        self.entropy_index += 1;
        Ok(value)
    }

    /// Get participant by owner
    pub fn get_participant(&self, owner: &Owner) -> Result<&BattleParticipant, BattleError> {
        let p1 = self.player1.as_ref().ok_or(BattleError::NotInitialized)?;
        let p2 = self.player2.as_ref().ok_or(BattleError::NotInitialized)?;

        if p1.owner == *owner {
            Ok(p1)
        } else if p2.owner == *owner {
            Ok(p2)
        } else {
            Err(BattleError::NotParticipant)
        }
    }

    /// Get mutable participant by owner
    pub fn get_participant_mut(&mut self, owner: &Owner) -> Result<&mut BattleParticipant, BattleError> {
        let is_p1 = self.player1.as_ref()
            .map(|p| p.owner == *owner)
            .unwrap_or(false);
        let is_p2 = self.player2.as_ref()
            .map(|p| p.owner == *owner)
            .unwrap_or(false);

        if is_p1 {
            Ok(self.player1.as_mut().unwrap())
        } else if is_p2 {
            Ok(self.player2.as_mut().unwrap())
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
    ) -> Result<(u32, bool, bool), BattleError> {
        let char = &attacker.character;

        // Base damage (random in range)
        let base_damage = random_in_range(
            &self.entropy_seed.as_ref().unwrap().seed,
            (self.entropy_index % 256) as u8,
            char.min_damage as u64,
            char.max_damage as u64,
        ) as u32;
        self.entropy_index += 1;

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
        let crit_roll = self.next_random()? % 10000;
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
        let dodge_roll = self.next_random()? % 10000;
        let was_dodged = dodge_roll < defender.character.dodge_chance as u64;

        if was_dodged {
            return Ok((0, was_crit, true));
        }

        // Apply defender's defense
        let def_reduction = defender.character.defense as u128 * FP_SCALE / 100; // defense / 100
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
            Stance::Berserker => damage, // No change to incoming damage
            Stance::Counter => mul_fp(damage, 6 * FP_SCALE / 10),     // 60% (take less)
        };

        // Apply defender defense traits
        if defender.character.defense_bps != 0 {
            let def_mod = FP_SCALE as i128 - ((defender.character.defense_bps as i128 * FP_SCALE as i128) / 10000);
            if def_mod > 0 {
                damage = ((damage as i128 * def_mod) / FP_SCALE as i128) as u128;
            } else {
                damage = FP_SCALE; // Minimum 1 damage
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
            let counter_roll = self.next_random()? % 10000;
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
    pub fn execute_full_round(&mut self) -> Result<RoundResult, BattleError> {
        let mut p1 = self.player1.take().ok_or(BattleError::NotInitialized)?;
        let mut p2 = self.player2.take().ok_or(BattleError::NotInitialized)?;

        let mut player1_actions = Vec::new();
        let mut player2_actions = Vec::new();

        // Execute 3 turns
        for turn in 0..3 {
            let p1_turn = p1.turns_submitted[turn].as_ref().unwrap();
            let p2_turn = p2.turns_submitted[turn].as_ref().unwrap();

            // Player 1 attacks
            if p1.current_hp > 0 && p2.current_hp > 0 {
                let action = self.execute_turn(&mut p1, &mut p2, p1_turn, p2_turn.stance)?;
                player1_actions.push(action);
            }

            // Player 2 attacks
            if p2.current_hp > 0 && p1.current_hp > 0 {
                let action = self.execute_turn(&mut p2, &mut p1, p2_turn, p1_turn.stance)?;
                player2_actions.push(action);
            }

            // Check for KO
            if p1.current_hp == 0 || p2.current_hp == 0 {
                break;
            }
        }

        let round_result = RoundResult {
            round: self.current_round,
            player1_actions,
            player2_actions,
            player1_hp: p1.current_hp,
            player2_hp: p2.current_hp,
        };

        // Restore players
        self.player1 = Some(p1);
        self.player2 = Some(p2);

        Ok(round_result)
    }
}

#[async_trait]
impl Contract for BattleContract {
    type Error = BattleError;
    type Storage = BattleState;
    type State = BattleState;
    type Message = Message;

    async fn new(state: Self::State, runtime: ContractRuntime) -> Result<Self, Self::Error> {
        Ok(BattleContract { state, runtime })
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    async fn initialize(
        &mut self,
        context: &linera_sdk::OperationContext,
        argument: Self::InitializationArgument,
    ) -> Result<(), Self::Error> {
        if let Operation::Initialize {
            player1_owner,
            player1_chain,
            player1_character,
            player1_stake,
            player2_owner,
            player2_chain,
            player2_character,
            player2_stake,
            battle_token_app,
            matchmaking_chain,
            entropy_seed,
        } = argument
        {
            self.state.player1 = Some(BattleParticipant::new(
                player1_owner,
                player1_chain,
                player1_character,
                player1_stake,
            ));

            self.state.player2 = Some(BattleParticipant::new(
                player2_owner,
                player2_chain,
                player2_character,
                player2_stake,
            ));

            self.state.status = BattleStatus::InProgress;
            self.state.current_round = 1;
            self.state.max_rounds = 3;
            self.state.battle_token_app = battle_token_app;
            self.state.matchmaking_chain = matchmaking_chain;
            self.state.platform_fee_bps = 300; // 3%
            self.state.started_at = Some(context.system.timestamp);

            // Initialize entropy
            self.state.entropy_seed = Some(EntropySeed {
                seed: entropy_seed,
                index: 0,
                timestamp: context.system.timestamp,
            });
            self.state.entropy_index = 0;

            // TODO: Set treasury owner
            self.state.treasury_owner = player1_owner; // Placeholder

            Ok(())
        } else {
            Err(BattleError::ViewError("Invalid initialization".to_string()))
        }
    }

    async fn execute_operation(
        &mut self,
        context: &linera_sdk::OperationContext,
        operation: Self::Operation,
    ) -> Result<(), Self::Error> {
        let caller = context
            .authenticated_signer
            .ok_or(BattleError::NotParticipant)?;

        match operation {
            Operation::Initialize { .. } => {
                // Handled in initialize()
                Ok(())
            }

            Operation::SubmitTurn {
                round,
                turn,
                stance,
                use_special,
            } => {
                if self.state.status != BattleStatus::InProgress {
                    return Err(BattleError::AlreadyCompleted);
                }

                if round != self.state.current_round {
                    return Err(BattleError::InvalidRound(round));
                }

                if turn >= 3 {
                    return Err(BattleError::InvalidTurn(turn));
                }

                let participant = self.state.get_participant_mut(&caller)?;

                if participant.current_hp == 0 {
                    return Err(BattleError::PlayerDefeated);
                }

                if participant.turns_submitted[turn as usize].is_some() {
                    return Err(BattleError::TurnAlreadySubmitted);
                }

                let submission = TurnSubmission {
                    round,
                    turn,
                    stance,
                    use_special,
                };

                participant.turns_submitted[turn as usize] = Some(submission);

                Ok(())
            }

            Operation::ExecuteRound => {
                if self.state.status != BattleStatus::InProgress {
                    return Err(BattleError::AlreadyCompleted);
                }

                // Check if both players submitted all turns
                let p1 = self.state.player1.as_ref().ok_or(BattleError::NotInitialized)?;
                let p2 = self.state.player2.as_ref().ok_or(BattleError::NotInitialized)?;

                if !p1.all_turns_submitted() || !p2.all_turns_submitted() {
                    return Err(BattleError::NotAllTurnsSubmitted);
                }

                // Execute the round
                let round_result = self.state.execute_full_round()?;
                self.state.round_results.push(round_result);

                // Check for winner
                let p1 = self.state.player1.as_ref().unwrap();
                let p2 = self.state.player2.as_ref().unwrap();

                if p1.current_hp == 0 {
                    self.state.winner = Some(p2.owner);
                    self.state.status = BattleStatus::Completed;
                    self.state.completed_at = Some(context.system.timestamp);
                } else if p2.current_hp == 0 {
                    self.state.winner = Some(p1.owner);
                    self.state.status = BattleStatus::Completed;
                    self.state.completed_at = Some(context.system.timestamp);
                } else if self.state.current_round >= self.state.max_rounds {
                    // Max rounds reached, winner is player with more HP
                    self.state.winner = if p1.current_hp > p2.current_hp {
                        Some(p1.owner)
                    } else {
                        Some(p2.owner)
                    };
                    self.state.status = BattleStatus::Completed;
                    self.state.completed_at = Some(context.system.timestamp);
                } else {
                    // Continue to next round
                    self.state.current_round += 1;
                    self.state.player1.as_mut().unwrap().reset_turns();
                    self.state.player2.as_mut().unwrap().reset_turns();
                }

                Ok(())
            }

            Operation::FinalizeBattle => {
                if self.state.status != BattleStatus::Completed {
                    return Err(BattleError::ViewError("Battle not completed".to_string()));
                }

                let p1 = self.state.player1.as_ref().ok_or(BattleError::NotInitialized)?;
                let p2 = self.state.player2.as_ref().ok_or(BattleError::NotInitialized)?;
                let winner_owner = self.state.winner.ok_or(BattleError::ViewError("No winner".to_string()))?;

                let (winner, loser) = if winner_owner == p1.owner {
                    (p1, p2)
                } else {
                    (p2, p1)
                };

                // Calculate payouts
                let total_stakes = p1.stake.saturating_add(p2.stake);
                let platform_fee = Amount::from_attos(
                    (total_stakes.to_attos() * self.state.platform_fee_bps as u128) / 10000
                );
                let winner_payout = total_stakes.saturating_sub(platform_fee);

                // TODO: Transfer tokens
                // - Send platform_fee to treasury
                // - Send winner_payout to winner's player chain

                // Send result messages to player chains
                // TODO: Implement cross-chain messaging

                // Notify matchmaking
                // TODO: Send message to matchmaking chain

                Ok(())
            }
        }
    }

    async fn execute_message(
        &mut self,
        _context: &linera_sdk::MessageContext,
        _message: Self::Message,
    ) -> Result<(), Self::Error> {
        // Battle chain primarily sends messages, doesn't receive many
        Ok(())
    }

    async fn store(mut self) -> Result<Self::State, Self::Error> {
        self.state.save().await?;
        Ok(self.state)
    }
}

pub struct BattleService {
    state: BattleState,
    runtime: ServiceRuntime,
}

#[async_trait]
impl Service for BattleService {
    type Error = BattleError;
    type Storage = BattleState;
    type State = BattleState;

    async fn new(state: Self::State, runtime: ServiceRuntime) -> Result<Self, Self::Error> {
        Ok(BattleService { state, runtime })
    }

    async fn handle_query(
        &mut self,
        _context: &linera_sdk::QueryContext,
        _query: Self::Query,
    ) -> Result<Self::QueryResponse, Self::Error> {
        // TODO: Implement GraphQL queries for:
        // - Current battle state
        // - Round results
        // - Player HP and stats
        // - Turn submissions status
        // - Winner and payouts
        Ok(())
    }
}

linera_sdk::contract!(BattleContract);
linera_sdk::service!(BattleService);
