use async_graphql::{Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{AccountOwner, Amount, ChainId, ContractAbi, ServiceAbi},
};
use serde::{Deserialize, Serialize};

/// Character classes with unique abilities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterClass {
    Warrior,
    Assassin,
    Mage,
    Tank,
    Trickster,
}

/// Battle stances with strategic modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Balanced,
    Aggressive,
    Defensive,
    Berserker,
    Counter,
}

/// Character snapshot for battles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    pub nft_id: String,
    pub class: CharacterClass,
    pub level: u16,
    pub hp_max: u32,
    pub min_damage: u16,
    pub max_damage: u16,
    pub crit_chance: u16,
    pub crit_multiplier: u16,
    pub dodge_chance: u16,
    pub defense: u16,
    pub attack_bps: i16,
    pub defense_bps: i16,
    pub crit_bps: i16,
}

/// Turn submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSubmission {
    pub round: u8,
    pub turn: u8,
    pub stance: Stance,
    pub use_special: bool,
}

/// Battle participant data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParticipant {
    pub owner: AccountOwner,
    pub chain: ChainId,
    pub character: CharacterSnapshot,
    pub stake: Amount,
    pub current_hp: u32,
    pub combo_stack: u8,
    pub special_cooldown: u8,
    pub turns_submitted: [Option<TurnSubmission>; 3],
}

/// Combat statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatStats {
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub crits: u64,
    pub dodges: u64,
    pub highest_crit: u64,
}

/// Global player statistics tracked by lobby
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerGlobalStats {
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub elo_rating: u64,
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub total_crits: u64,
    pub total_dodges: u64,
    pub highest_crit: u64,
    pub total_earnings: Amount,
    pub current_streak: u64,
    pub best_streak: u64,
}

impl Default for PlayerGlobalStats {
    fn default() -> Self {
        Self {
            total_battles: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            elo_rating: 1200,
            total_damage_dealt: 0,
            total_damage_taken: 0,
            total_crits: 0,
            total_dodges: 0,
            highest_crit: 0,
            total_earnings: Amount::ZERO,
            current_streak: 0,
            best_streak: 0,
        }
    }
}

/// Initialization argument for different chain types
#[derive(Debug, Deserialize, Serialize)]
pub struct InitializationArgument {
    pub variant: ChainVariant,
    pub treasury_owner: Option<AccountOwner>,
    pub platform_fee_bps: Option<u16>,
}

/// Chain variant type
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum ChainVariant {
    Lobby,
    Battle,
    Player,
    Prediction,
}

pub struct MajorulesAbi;

impl ContractAbi for MajorulesAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for MajorulesAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Operations for all chain types
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    // ========== SHARED OPERATIONS ==========
    /// Legacy increment for testing
    Increment { value: u64 },

    // ========== LOBBY OPERATIONS ==========
    /// Join matchmaking queue with character and stake (auto-matches when 2 players)
    JoinQueue { 
        character_id: String, 
        stake: Amount 
    },
    
    /// Leave matchmaking queue
    LeaveQueue,
    
    /// Create private battle and return battle ID
    CreatePrivateBattle { 
        character_id: String, 
        stake: Amount 
    },
    
    /// Join existing private battle by ID
    JoinPrivateBattle { 
        battle_id: u64,
        character_id: String, 
        stake: Amount 
    },
    
    /// Update global leaderboard for specific player
    UpdateLeaderboard { 
        player: AccountOwner 
    },
    
    /// Create player chain for user
    CreatePlayerChain,
    
    // ========== BATTLE OPERATIONS ==========
    /// Submit turn for current round
    SubmitTurn { 
        round: u8, 
        turn: u8, 
        stance: String, 
        use_special: bool 
    },
    
    /// Execute current round when all turns submitted (auto-executed)
    ExecuteRound,
    
    // ========== PLAYER OPERATIONS ==========
    /// Mint new character NFT
    MintCharacter { 
        character_id: String, 
        class: String 
    },
    
    /// Level up character using XP (with level-up logic)
    LevelUpCharacter { 
        character_id: String,
        xp_to_spend: u64 
    },
    
    /// Set active character for battles
    SetActiveCharacter { 
        character_id: String 
    },
    

    
    // ========== PREDICTION MARKET OPERATIONS ==========
    /// Create prediction market for battle
    CreateMarket { 
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
    },
    
    /// Place bet on battle outcome
    PlaceBet { 
        market_id: u64, 
        predicted_winner: ChainId, 
        amount: Amount 
    },
    
    /// Close market (stop accepting bets)
    CloseMarket { 
        market_id: u64 
    },
    
    /// Settle market and distribute winnings
    SettleMarket { 
        market_id: u64, 
        winner_chain: ChainId 
    },
    
    /// Claim winnings from settled market
    ClaimWinnings { 
        market_id: u64 
    },
    
    // ========== TOKEN OPERATIONS ==========
    /// Transfer battle tokens between accounts
    TransferTokens { 
        to: AccountOwner, 
        amount: Amount 
    },
}

/// Cross-chain messages between different chain types
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    // ===== LOBBY → BATTLE =====
    /// Initialize new battle chain with participants
    InitializeBattle {
        player1: BattleParticipant,
        player2: BattleParticipant,
        lobby_chain_id: ChainId,
        platform_fee_bps: u16,
        treasury_owner: AccountOwner,
    },
    
    // ===== BATTLE → PLAYER =====
    /// Send battle result to player chain
    BattleResult {
        winner: AccountOwner,
        loser: AccountOwner,
        winner_payout: Amount,
        xp_gained: u64,
        battle_stats: CombatStats,
        battle_chain: ChainId,
    },
    
    // ===== BATTLE → LOBBY =====
    /// Notify lobby of battle completion for leaderboard
    BattleCompleted {
        winner: AccountOwner,
        loser: AccountOwner,
        rounds_played: u8,
        total_stake: Amount,
        battle_stats: (CombatStats, CombatStats), // (winner_stats, loser_stats)
    },
    
    /// Battle result with ELO changes for lobby processing
    BattleResultWithElo {
        player: AccountOwner,
        opponent: AccountOwner,
        won: bool,
        payout: Amount,
        xp_gained: u64,
        elo_change: i32,
        battle_stats: CombatStats,
        battle_chain: ChainId,
    },
    
    // ===== PLAYER → LOBBY =====
    /// Request to join matchmaking queue
    RequestJoinQueue {
        player: AccountOwner,
        player_chain: ChainId,
        character_snapshot: CharacterSnapshot,
        stake: Amount,
    },
    
    /// Request to create private battle
    RequestCreatePrivateBattle {
        player: AccountOwner,
        player_chain: ChainId,
        character_snapshot: CharacterSnapshot,
        stake: Amount,
    },
    
    /// Request to join private battle by ID
    RequestJoinPrivateBattle {
        player: AccountOwner,
        player_chain: ChainId,
        battle_id: u64,
        character_snapshot: CharacterSnapshot,
        stake: Amount,
    },
    
    // ===== BATTLE → PREDICTION =====
    /// Notify prediction market that battle started
    BattleStarted {
        battle_chain: ChainId,
    },
    
    /// Notify prediction market of battle result
    BattleEnded {
        battle_chain: ChainId,
        winner_chain: ChainId,
    },
    
    // ===== LOBBY → PREDICTION =====
    /// Create prediction market for new battle
    CreatePredictionMarket {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
    },
    
    // ===== PREDICTION → PLAYER =====
    /// Distribute winnings to bettor
    DistributeWinnings {
        bettor: AccountOwner,
        amount: Amount,
        market_id: u64,
    },
    
    // ===== LOBBY → PLAYER =====
    /// Request player stats from player chain
    RequestPlayerStats {
        player: AccountOwner,
    },
    
    /// Update player stats after battle with ELO
    UpdatePlayerStats {
        player: AccountOwner,
        won: bool,
        xp_gained: u64,
        elo_change: i32,
        battle_chain: ChainId,
    },
    
    // ===== PLAYER → LOBBY =====
    /// Response with player stats
    PlayerStatsResponse {
        player: AccountOwner,
        stats: PlayerGlobalStats,
    },
    
    // ===== LOBBY → PLAYER =====
    /// Notify player that private battle was created
    PrivateBattleCreated {
        battle_id: u64,
    },

    /// Initialize player chain with lobby reference
    InitializePlayerChain {
        lobby_chain_id: ChainId,
        owner: AccountOwner,
    },
    
    /// Instantiate chain with specific variant
    InstantiateChain {
        variant: ChainVariant,
        treasury_owner: Option<AccountOwner>,
        platform_fee_bps: Option<u16>,
    },
}

impl CharacterClass {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "warrior" => Some(CharacterClass::Warrior),
            "assassin" => Some(CharacterClass::Assassin),
            "mage" => Some(CharacterClass::Mage),
            "tank" => Some(CharacterClass::Tank),
            "trickster" => Some(CharacterClass::Trickster),
            _ => None,
        }
    }
}

impl Stance {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "balanced" => Some(Stance::Balanced),
            "aggressive" => Some(Stance::Aggressive),
            "defensive" => Some(Stance::Defensive),
            "berserker" => Some(Stance::Berserker),
            "counter" => Some(Stance::Counter),
            _ => None,
        }
    }
}

impl CharacterClass {
    /// Get base stats (HP, min_dmg, max_dmg, crit_bps)
    pub fn base_stats(&self) -> (u32, u16, u16, u16) {
        match self {
            CharacterClass::Warrior => (120, 8, 15, 1500),   // 15% crit
            CharacterClass::Assassin => (90, 12, 20, 3500),  // 35% crit
            CharacterClass::Mage => (80, 10, 18, 2000),      // 20% crit
            CharacterClass::Tank => (150, 6, 12, 1000),      // 10% crit
            CharacterClass::Trickster => (100, 8, 16, 2500), // 25% crit
        }
    }

    /// Special ability cooldown
    pub fn special_cooldown(&self) -> u8 {
        match self {
            CharacterClass::Warrior => 3,
            CharacterClass::Assassin => 4,
            CharacterClass::Mage => 3,
            CharacterClass::Tank => 4,
            CharacterClass::Trickster => 2,
        }
    }
}

impl BattleParticipant {
    pub fn new(owner: AccountOwner, chain: ChainId, character: CharacterSnapshot, stake: Amount) -> Self {
        Self {
            owner,
            chain,
            character: character.clone(),
            stake,
            current_hp: character.hp_max,
            combo_stack: 0,
            special_cooldown: 0,
            turns_submitted: [None, None, None],
        }
    }
    
    /// Reset turn submissions for new round
    pub fn reset_turns(&mut self) {
        self.turns_submitted = [None, None, None];
    }
    
    /// Check if all turns submitted
    pub fn all_turns_submitted(&self) -> bool {
        self.turns_submitted.iter().all(|t| t.is_some())
    }
    
    /// Take damage and return if defeated
    pub fn take_damage(&mut self, damage: u32) -> bool {
        self.current_hp = self.current_hp.saturating_sub(damage);
        self.current_hp == 0
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
    
    /// Tick cooldown
    pub fn tick_cooldown(&mut self) {
        if self.special_cooldown > 0 {
            self.special_cooldown -= 1;
        }
    }
}

impl CombatStats {
    pub fn new() -> Self {
        Self {
            damage_dealt: 0,
            damage_taken: 0,
            crits: 0,
            dodges: 0,
            highest_crit: 0,
        }
    }
}

/// Fixed-point math constants
pub const FP_SCALE: u128 = 1_000_000; // 1e6 for fixed-point arithmetic
pub const MAX_COMBO_STACK: u8 = 5;

/// Helper: multiply two fixed-point values
pub fn mul_fp(a: u128, b: u128) -> u128 {
    (a * b) / FP_SCALE
}

/// Helper: convert fixed-point to u64
pub fn fp_to_u64(value: u128) -> u64 {
    (value / FP_SCALE) as u64
}

/// Generate random value from seed and tag
pub fn derive_random_u64(seed: &[u8; 32], tag: u8) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    tag.hash(&mut hasher);
    hasher.finish()
}

/// Generate random value in range [min, max]
pub fn random_in_range(seed: &[u8; 32], tag: u8, min: u64, max: u64) -> u64 {
    let raw = derive_random_u64(seed, tag);
    let range = max - min + 1;
    min + (raw % range)
}
