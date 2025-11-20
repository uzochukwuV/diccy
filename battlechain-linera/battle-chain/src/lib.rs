use battlechain_shared_types::{CharacterSnapshot, Owner, Stance};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi},
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

// Import battle-token ABI and types for inter-contract calls
use battle_token::{BattleTokenAbi, Operation as BattleTokenOperation, TokenResponse};

/// Battle Chain Application ABI
pub struct BattleChainAbi;

impl ContractAbi for BattleChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for BattleChainAbi {
    type Query = async_graphql::Request;
    type QueryResponse = async_graphql::Response;
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
        if self.combo_stack < battlechain_shared_types::MAX_COMBO_STACK {
            self.combo_stack += 1;
        }
    }

    /// Reset combo stack
    pub fn reset_combo(&mut self) {
        self.combo_stack = 0;
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
        battle_token_app: ApplicationId<BattleTokenAbi>,
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
    pub battle_token_app: ApplicationId<BattleTokenAbi>,
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
