use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    linera_base_types::{Amount, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, ViewStorageContext},
};
use serde::{Deserialize, Serialize};

use crate::RegistryError;

/// Character statistics in the global registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterStats {
    pub character_id: String,
    pub nft_id: String,
    pub owner: Owner,
    pub owner_chain: ChainId,
    pub class: CharacterClass,
    pub level: u16,

    // Battle statistics
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64, // Calculated as wins / total_battles
    pub current_streak: i32, // Positive for wins, negative for losses
    pub best_win_streak: u32,

    // Combat statistics
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub highest_crit: u64,
    pub total_crits: u64,
    pub total_dodges: u64,

    // Earnings
    pub total_earnings: Amount,
    pub total_wagered: Amount,

    // ELO rating (starts at 1500)
    pub elo_rating: u64,

    // Status
    pub is_alive: bool,
    pub lives_remaining: u8,

    // Timestamps
    pub registered_at: Timestamp,
    pub last_battle_at: Option<Timestamp>,
}

impl CharacterStats {
    pub fn new(
        character_id: String,
        nft_id: String,
        owner: Owner,
        owner_chain: ChainId,
        class: CharacterClass,
        level: u16,
        registered_at: Timestamp,
    ) -> Self {
        Self {
            character_id,
            nft_id,
            owner,
            owner_chain,
            class,
            level,
            total_battles: 0,
            wins: 0,
            losses: 0,
            win_rate: 0.0,
            current_streak: 0,
            best_win_streak: 0,
            total_damage_dealt: 0,
            total_damage_taken: 0,
            highest_crit: 0,
            total_crits: 0,
            total_dodges: 0,
            total_earnings: Amount::ZERO,
            total_wagered: Amount::ZERO,
            elo_rating: 1500, // Starting ELO
            is_alive: true,
            lives_remaining: 3,
            registered_at,
            last_battle_at: None,
        }
    }

    /// Update stats after a battle
    pub fn update_after_battle(
        &mut self,
        won: bool,
        damage_dealt: u64,
        damage_taken: u64,
        crits: u64,
        dodges: u64,
        highest_crit: u64,
        earnings: Amount,
        stake: Amount,
        opponent_elo: u64,
        timestamp: Timestamp,
    ) {
        self.total_battles += 1;

        if won {
            self.wins += 1;
            self.current_streak = if self.current_streak >= 0 {
                self.current_streak + 1
            } else {
                1
            };
            if self.current_streak > self.best_win_streak as i32 {
                self.best_win_streak = self.current_streak as u32;
            }
        } else {
            self.losses += 1;
            self.current_streak = if self.current_streak <= 0 {
                self.current_streak - 1
            } else {
                -1
            };
        }

        self.win_rate = (self.wins as f64) / (self.total_battles as f64);

        self.total_damage_dealt += damage_dealt;
        self.total_damage_taken += damage_taken;
        self.total_crits += crits;
        self.total_dodges += dodges;

        if highest_crit > self.highest_crit {
            self.highest_crit = highest_crit;
        }

        self.total_earnings = self.total_earnings.saturating_add(earnings);
        self.total_wagered = self.total_wagered.saturating_add(stake);

        // Update ELO rating
        self.elo_rating = calculate_new_elo(self.elo_rating, opponent_elo, won);

        self.last_battle_at = Some(timestamp);
    }
}

/// Calculate new ELO rating using standard ELO formula
fn calculate_new_elo(player_elo: u64, opponent_elo: u64, won: bool) -> u64 {
    const K_FACTOR: f64 = 32.0; // Standard K-factor

    // Expected score
    let expected = 1.0 / (1.0 + 10f64.powf((opponent_elo as f64 - player_elo as f64) / 400.0));

    // Actual score
    let actual = if won { 1.0 } else { 0.0 };

    // New rating
    let new_rating = player_elo as f64 + K_FACTOR * (actual - expected);

    new_rating.max(100.0) as u64 // Minimum ELO of 100
}

/// Battle record for history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleRecord {
    pub battle_id: u64,
    pub battle_chain: ChainId,
    pub player1_id: String,
    pub player2_id: String,
    pub winner_id: String,
    pub stake: Amount,
    pub rounds_played: u8,
    pub timestamp: Timestamp,
}

/// Registry State - tracks global game statistics
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct RegistryState {
    /// Character statistics indexed by character_id
    pub characters: MapView<String, CharacterStats>,

    /// Character ID by owner chain (for quick lookup)
    pub owner_to_character: MapView<ChainId, String>,

    /// Battle records indexed by battle_id
    pub battles: MapView<u64, BattleRecord>,

    /// Battle chain to battle ID mapping
    pub battle_chain_to_id: MapView<ChainId, u64>,

    /// Next battle ID
    pub next_battle_id: RegisterView<u64>,

    /// Total characters registered
    pub total_characters: RegisterView<u64>,

    /// Total battles recorded
    pub total_battles: RegisterView<u64>,

    /// Total volume wagered
    pub total_volume: RegisterView<Amount>,

    /// Top characters by ELO (limited to top 100)
    pub top_elo: RegisterView<Vec<String>>, // Character IDs sorted by ELO

    /// SECURITY: Track known battle chains (for message authentication)
    pub known_battle_chains: MapView<ChainId, bool>,

    /// SECURITY: Admin owner (for pause functionality)
    pub admin: RegisterView<Option<Owner>>,

    /// SECURITY: Paused state
    pub paused: RegisterView<bool>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
}

impl RegistryState {
    /// Register a new character
    pub fn register_character(&mut self, stats: CharacterStats) -> Result<(), RegistryError> {
        let character_id = stats.character_id.clone();
        let owner_chain = stats.owner_chain;

        self.characters.insert(&character_id, stats)?;
        self.owner_to_character.insert(&owner_chain, character_id)?;

        let total = *self.total_characters.get();
        self.total_characters.set(total + 1);

        Ok(())
    }

    /// Update leaderboard after character stats change
    /// Sorts by ELO rating (descending) and keeps top 100
    pub async fn update_leaderboard(&mut self, character_id: String) -> Result<(), RegistryError> {
        let mut top = self.top_elo.get().clone();

        // Remove if already exists
        top.retain(|id| id != &character_id);

        // Add to list
        top.push(character_id);

        // Fetch ELO ratings for all characters in the list
        let mut character_elos: Vec<(String, u64)> = Vec::new();
        for id in top.iter() {
            if let Some(stats) = self.characters.get(id).await? {
                character_elos.push((id.clone(), stats.elo_rating));
            }
        }

        // Sort by ELO rating (descending)
        character_elos.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep only top 100 and extract character IDs
        let sorted_ids: Vec<String> = character_elos
            .into_iter()
            .take(100)
            .map(|(id, _)| id)
            .collect();

        self.top_elo.set(sorted_ids);
        Ok(())
    }
}
