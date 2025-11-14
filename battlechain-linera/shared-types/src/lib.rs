use serde::{Deserialize, Serialize};
use linera_sdk::linera_base_types::{Account, AccountOwner, Amount, ChainId, Timestamp};

// Type alias for compatibility
pub type Owner = AccountOwner;

/// Character classes with unique abilities and stats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterClass {
    Warrior,
    Assassin,
    Mage,
    Tank,
    Trickster,
}

impl CharacterClass {
    /// Get base stats for a character class (HP, min_dmg, max_dmg, crit_bps)
    pub fn base_stats(&self) -> (u32, u16, u16, u16) {
        match self {
            CharacterClass::Warrior => (120, 8, 15, 1500),   // 15% crit
            CharacterClass::Assassin => (90, 12, 20, 3500),  // 35% crit
            CharacterClass::Mage => (80, 10, 18, 2000),      // 20% crit
            CharacterClass::Tank => (150, 6, 12, 1000),      // 10% crit
            CharacterClass::Trickster => (100, 8, 16, 2500), // 25% crit
        }
    }

    /// Get special ability cooldown for class
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

/// Battle stances with strategic advantages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    Balanced,    // 100% atk, 100% def
    Aggressive,  // 130% atk, 150% def (take more dmg)
    Defensive,   // 70% atk, 50% def (take less dmg)
    Berserker,   // 200% atk, 25% self-damage
    Counter,     // 90% atk, 40% counter
}

/// Currency types supported (BATTLE token only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Currency {
    /// BattleChain native token
    BATTLE,
}

/// Battle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BattleStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

/// Character NFT data stored on Player chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterNFT {
    pub nft_id: String,
    pub class: CharacterClass,
    pub level: u16,
    pub xp: u64,
    pub lives: u8,

    // Current stats (base + level bonuses + traits)
    pub hp_max: u32,
    pub min_damage: u16,
    pub max_damage: u16,
    pub crit_chance: u16,      // basis points (10000 = 100%)
    pub crit_multiplier: u16,   // basis points (20000 = 2.0x)
    pub dodge_chance: u16,      // basis points
    pub defense: u16,

    // Trait modifiers (basis points)
    pub rarity: u8,            // 1-5 stars
    pub attack_bps: i16,       // +/- attack%
    pub defense_bps: i16,      // +/- defense%
    pub crit_bps: i16,         // +/- crit chance%

    // Battle state
    pub in_battle: bool,
    pub current_hp: u32,

    // Timestamps
    pub created_at: Timestamp,
}

impl CharacterNFT {
    /// Create a new character from NFT
    pub fn new(nft_id: String, class: CharacterClass, created_at: Timestamp) -> Self {
        let (hp, min_dmg, max_dmg, crit) = class.base_stats();

        Self {
            nft_id,
            class,
            level: 1,
            xp: 0,
            lives: 3,
            hp_max: hp,
            min_damage: min_dmg,
            max_damage: max_dmg,
            crit_chance: crit,
            crit_multiplier: 20000, // 2.0x default
            dodge_chance: 800,       // 8% base
            defense: 0,
            rarity: 1,
            attack_bps: 0,
            defense_bps: 0,
            crit_bps: 0,
            in_battle: false,
            current_hp: hp,
            created_at,
        }
    }

    /// Apply trait bundle modifiers
    pub fn apply_traits(&mut self, trait_bundle: &TraitBundle) {
        self.rarity = trait_bundle.rarity;
        self.attack_bps = self.attack_bps.saturating_add(trait_bundle.attack_bps);
        self.defense_bps = self.defense_bps.saturating_add(trait_bundle.defense_bps);
        self.crit_bps = self.crit_bps.saturating_add(trait_bundle.crit_bps);
    }

    /// Calculate XP needed for next level (quadratic: 100 * level^2)
    pub fn xp_for_next_level(&self) -> u64 {
        100u64 * (self.level as u64) * (self.level as u64)
    }

    /// Level up and increase stats
    pub fn level_up(&mut self) -> bool {
        let xp_needed = self.xp_for_next_level();
        if self.xp >= xp_needed {
            self.xp -= xp_needed;
            self.level += 1;

            // Increase stats by 5% HP, 10% damage
            self.hp_max = self.hp_max.saturating_add((self.hp_max / 20).max(1)); // +5%
            self.current_hp = self.hp_max; // heal on level up
            self.min_damage = self.min_damage.saturating_add((self.min_damage / 10).max(1));
            self.max_damage = self.max_damage.saturating_add((self.max_damage / 10).max(1));

            true
        } else {
            false
        }
    }

    /// Take damage and check if defeated
    pub fn take_damage(&mut self, damage: u32) -> bool {
        self.current_hp = self.current_hp.saturating_sub(damage);
        self.current_hp == 0
    }

    /// Consume a life after defeat
    pub fn consume_life(&mut self) {
        if self.lives > 0 {
            self.lives -= 1;
        }
        self.current_hp = self.hp_max; // restore HP
    }
}

/// Trait bundle applied by authority
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TraitBundle {
    pub rarity: u8,
    pub attack_bps: i16,
    pub defense_bps: i16,
    pub crit_bps: i16,
    pub nonce: i64,
}

/// Item in player inventory (future)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub item_id: String,
    pub name: String,
    pub item_type: String,
    pub quantity: u32,
}

/// Character snapshot for battle initialization
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

impl From<&CharacterNFT> for CharacterSnapshot {
    fn from(character: &CharacterNFT) -> Self {
        Self {
            nft_id: character.nft_id.clone(),
            class: character.class,
            level: character.level,
            hp_max: character.hp_max,
            min_damage: character.min_damage,
            max_damage: character.max_damage,
            crit_chance: character.crit_chance,
            crit_multiplier: character.crit_multiplier,
            dodge_chance: character.dodge_chance,
            defense: character.defense,
            attack_bps: character.attack_bps,
            defense_bps: character.defense_bps,
            crit_bps: character.crit_bps,
        }
    }
}

/// Character registry entry for global tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterRegistryEntry {
    pub character_id: String,
    pub owner: Owner,
    pub owner_chain: ChainId,
    pub class: CharacterClass,
    pub level: u16,
    pub created_at: Timestamp,

    // Stats
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub total_damage_dealt: u64,
    pub total_damage_taken: u64,
    pub highest_crit: u64,

    // Status
    pub is_alive: bool,
    pub lives_remaining: u8,
}

/// Leaderboard entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub rank: u64,
    pub character_id: String,
    pub owner: Owner,
    pub class: CharacterClass,
    pub level: u16,
    pub wins: u64,
    pub losses: u64,
    pub win_rate: f64,
    pub elo_rating: u64,
    pub total_earnings_battle: Amount, // Total BATTLE tokens earned
}

/// Entropy seed for randomness
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EntropySeed {
    pub seed: [u8; 32],
    pub index: u64,
    pub timestamp: Timestamp,
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

/// Derive multiple random values from a single seed
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_character_creation() {
        let char = CharacterNFT::new(
            "warrior_1".to_string(),
            CharacterClass::Warrior,
            Timestamp::from(0),
        );
        assert_eq!(char.level, 1);
        assert_eq!(char.hp_max, 120);
        assert_eq!(char.lives, 3);
    }

    #[test]
    fn test_level_up() {
        let mut char = CharacterNFT::new(
            "test".to_string(),
            CharacterClass::Assassin,
            Timestamp::from(0),
        );

        char.xp = 100; // Level 1 -> 2 requires 100 XP
        assert!(char.level_up());
        assert_eq!(char.level, 2);
        assert!(char.hp_max > 90); // HP increased
    }

    #[test]
    fn test_fixed_point_math() {
        let a = 2 * FP_SCALE; // 2.0
        let b = 3 * FP_SCALE / 2; // 1.5
        let result = mul_fp(a, b);
        assert_eq!(fp_to_u64(result), 3); // 2.0 * 1.5 = 3.0
    }
}
