use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ChainId, Timestamp},
    views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext},
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

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BattleStatus {
    #[default]
    WaitingForPlayers,
    InProgress,
    Completed,
    Cancelled,
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

impl BattleParticipant {
    /// Create new battle participant
    pub fn new(owner: AccountOwner, chain: ChainId, character: CharacterSnapshot, stake: Amount) -> Self {
        Self {
            owner,
            chain,
            current_hp: character.hp_max,
            character,
            stake,
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
            self.special_cooldown = 3; // Default cooldown
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
        const MAX_COMBO_STACK: u8 = 5;
        if self.combo_stack < MAX_COMBO_STACK {
            self.combo_stack += 1;
        }
    }

    /// Reset combo stack
    pub fn reset_combo(&mut self) {
        self.combo_stack = 0;
    }
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

/// Queue entry for matchmaking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerQueueEntry {
    pub player: AccountOwner,
    pub player_chain: ChainId,
    pub character_id: String,
    pub character_snapshot: CharacterSnapshot,
    pub stake: Amount,
    pub joined_at: Timestamp,
}

/// Individual combat action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatAction {
    pub attacker: AccountOwner,
    pub defender: AccountOwner,
    pub damage: u32,
    pub was_crit: bool,
    pub was_dodged: bool,
    pub was_countered: bool,
    pub special_used: bool,
    pub defender_hp_remaining: u32,
}

/// Round result with all combat actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResult {
    pub round: u8,
    pub player1_actions: Vec<CombatAction>,
    pub player2_actions: Vec<CombatAction>,
    pub player1_hp: u32,
    pub player2_hp: u32,
}

/// Battle metadata for lobby tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleMetadata {
    pub battle_chain: ChainId,
    pub player1: AccountOwner,
    pub player2: AccountOwner,
    pub total_stake: Amount,
    pub created_at: Timestamp,
    pub status: BattleStatus,
}

/// Global player statistics
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

/// Character registry entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRegistryEntry {
    pub character_id: String,
    pub owner: AccountOwner,
    pub owner_chain: ChainId,
    pub class: CharacterClass,
    pub level: u16,
    pub created_at: Timestamp,
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub is_alive: bool,
    pub lives_remaining: u8,
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u64,
    pub player: AccountOwner,
    pub elo_rating: u64,
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub total_earnings: Amount,
}

/// Character NFT data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterNFT {
    pub nft_id: String,
    pub class: CharacterClass,
    pub level: u16,
    pub xp: u64,
    pub lives: u8,
    pub hp_max: u32,
    pub min_damage: u16,
    pub max_damage: u16,
    pub crit_chance: u16,
    pub crit_multiplier: u16,
    pub dodge_chance: u16,
    pub defense: u16,
    pub rarity: u8,
    pub attack_bps: i16,
    pub defense_bps: i16,
    pub crit_bps: i16,
    pub in_battle: bool,
    pub current_hp: u32,
    pub created_at: Timestamp,
}

/// Battle record for player history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleRecord {
    pub battle_chain: ChainId,
    pub opponent: AccountOwner,
    pub character_used: String,
    pub stake: Amount,
    pub result: BattleResult,
    pub rounds_played: u8,
    pub xp_gained: u64,
    pub payout: Amount,
    pub combat_stats: CombatStats,
    pub completed_at: Timestamp,
}

/// Battle result
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleResult {
    Won,
    Lost,
    Draw,
}

/// Prediction market
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Market {
    pub market_id: u64,
    pub battle_chain: ChainId,
    pub player1_chain: ChainId,
    pub player2_chain: ChainId,
    pub status: MarketStatus,
    pub total_pool: Amount,
    pub player1_pool: Amount,
    pub player2_pool: Amount,
    pub winner_chain: Option<ChainId>,
    pub created_at: Timestamp,
    pub closed_at: Option<Timestamp>,
    pub settled_at: Option<Timestamp>,
}

/// Market status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketStatus {
    Open,
    Closed,
    Settled,
    Cancelled,
}

/// Individual bet
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bet {
    pub bettor: AccountOwner,
    pub market_id: u64,
    pub predicted_winner: ChainId,
    pub amount: Amount,
    pub odds_at_bet: u64,
    pub placed_at: Timestamp,
    pub claimed: bool,
}

/// Betting leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BettingLeaderboardEntry {
    pub rank: u64,
    pub bettor: AccountOwner,
    pub total_bets: u64,
    pub total_wagered: Amount,
    pub total_winnings: Amount,
    pub profit: Amount,
    pub win_rate: f64,
}

/// Lobby state - matchmaking, leaderboards, and platform management
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LobbyState {
    pub value: RegisterView<u64>,
    pub waiting_players: MapView<AccountOwner, PlayerQueueEntry>,
    pub active_battles: MapView<ChainId, BattleMetadata>,
    pub battle_count: RegisterView<u64>,
    pub player_stats: MapView<AccountOwner, PlayerGlobalStats>,
    pub character_registry: MapView<String, CharacterRegistryEntry>,
    pub leaderboard: RegisterView<Vec<LeaderboardEntry>>,
    pub platform_fee_bps: RegisterView<u16>,
    pub treasury_owner: RegisterView<Option<AccountOwner>>,
    pub total_platform_revenue: RegisterView<Amount>,
    pub battle_token_balance: RegisterView<Amount>,
}

/// Battle state - individual combat session between two players
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct BattleState {
    pub value: RegisterView<u64>,
    pub player1: RegisterView<Option<BattleParticipant>>,
    pub player2: RegisterView<Option<BattleParticipant>>,
    pub status: RegisterView<BattleStatus>,
    pub current_round: RegisterView<u8>,
    pub max_rounds: RegisterView<u8>,
    pub turn_submissions: MapView<(AccountOwner, u8), TurnSubmission>,
    pub winner: RegisterView<Option<AccountOwner>>,
    pub round_results: RegisterView<Vec<RoundResult>>,
    pub battle_log: RegisterView<Vec<String>>,
    pub random_counter: RegisterView<u64>,
    pub lobby_chain_id: RegisterView<Option<ChainId>>,
    pub total_stake: RegisterView<Amount>,
    pub platform_fee_bps: RegisterView<u16>,
    pub treasury_owner: RegisterView<Option<AccountOwner>>,
    pub started_at: RegisterView<Option<Timestamp>>,
    pub completed_at: RegisterView<Option<Timestamp>>,
    pub round_deadline: RegisterView<Option<Timestamp>>,
}

/// Character data for player chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterData {
    pub nft_id: String,
    pub owner: AccountOwner,
    pub class: CharacterClass,
    pub level: u16,
    pub xp: u64,
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
    pub created_at: Timestamp,
    pub is_active: bool,
}

/// Player state - NFT characters, inventory, and personal statistics
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PlayerState {
    pub value: RegisterView<u64>,
    pub owner: RegisterView<Option<AccountOwner>>,
    pub lobby_chain_id: RegisterView<Option<ChainId>>,
    pub characters: MapView<String, CharacterData>,
    pub active_character: RegisterView<Option<String>>,
    pub character_count: RegisterView<u64>,
    pub battle_history: MapView<ChainId, BattleRecord>,
    pub player_stats: RegisterView<PlayerGlobalStats>,
    pub battle_token_balance: RegisterView<Amount>,
    pub locked_stakes: MapView<ChainId, Amount>,
    pub in_battle: RegisterView<bool>,
    pub current_battle_chain: RegisterView<Option<ChainId>>,
    pub last_active: RegisterView<Timestamp>,
}

/// Prediction market state - betting on battle outcomes
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PredictionState {
    pub value: RegisterView<u64>,
    pub markets: MapView<u64, Market>,
    pub battle_to_market: MapView<ChainId, u64>,
    pub market_count: RegisterView<u64>,
    pub bets: MapView<(u64, AccountOwner), Bet>,
    pub user_bet_counts: MapView<AccountOwner, u64>,
    pub user_volumes: MapView<AccountOwner, Amount>,
    pub total_volume: RegisterView<Amount>,
    pub total_fees_collected: RegisterView<Amount>,
    pub platform_fee_bps: RegisterView<u16>,
    pub treasury_owner: RegisterView<Option<AccountOwner>>,
    pub betting_leaderboard: RegisterView<Vec<BettingLeaderboardEntry>>,
}


