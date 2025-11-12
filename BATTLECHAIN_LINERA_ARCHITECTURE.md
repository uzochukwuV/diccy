# BattleChain on Linera: Microchains Architecture Design

## 📋 Table of Contents

- [Executive Summary](#executive-summary)
- [Architecture Overview](#architecture-overview)
- [Microchains Design](#microchains-design)
- [Cross-Chain Messaging](#cross-chain-messaging)
- [Prediction Market Integration](#prediction-market-integration)
- [Data Flow Examples](#data-flow-examples)
- [Solidity to Linera Migration](#solidity-to-linera-migration)
- [Performance Optimizations](#performance-optimizations)
- [Implementation Roadmap](#implementation-roadmap)

---

## 🎯 Executive Summary

BattleChain on Linera transforms the traditional monolithic smart contract architecture into a distributed microchains system that leverages:

- **Sub-500ms finality** for instant battle actions
- **Parallel execution** for unlimited concurrent battles
- **User-owned chains** for gas-free character management
- **Elastic scaling** as the player base grows
- **Real-time prediction markets** on battle outcomes

### Key Architecture Principles

1. **User Sovereignty**: Each player owns their microchain for characters & inventory
2. **Battle Isolation**: Each battle runs on its own multi-owner microchain
3. **Public Discovery**: Matchmaking and markets run on public microchains
4. **Deterministic Randomness**: Shared entropy microchain for fair RNG
5. **Economic Security**: Escrow and staking managed via cross-chain messages

---

## 🏗️ Architecture Overview

### Microchain Types Distribution

```
┌─────────────────────────────────────────────────────────────────┐
│                    BATTLECHAIN ECOSYSTEM                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────────┐      ┌──────────────────┐                │
│  │  Player Chains   │      │  Battle Chains   │                │
│  │  (Single-Owner)  │◄────►│  (Multi-Owner)   │                │
│  │                  │      │                  │                │
│  │  • Characters    │      │  • Active Battles│                │
│  │  • Inventory     │      │  • Turn State    │                │
│  │  • Stats/XP      │      │  • Combat Logic  │                │
│  │  • Preferences   │      │                  │                │
│  └────────┬─────────┘      └────────┬─────────┘                │
│           │                         │                           │
│           │        ┌────────────────┴──────────────┐           │
│           │        │                                │           │
│           ▼        ▼                                ▼           │
│  ┌─────────────────────────┐           ┌──────────────────┐   │
│  │  Matchmaking Chain      │           │ Prediction Market│   │
│  │  (Public)               │◄─────────►│ Chain (Public)   │   │
│  │                         │           │                  │   │
│  │  • Battle Offers        │           │  • Betting Pools │   │
│  │  • Open Challenges      │           │  • Odds Calc     │   │
│  │  • Matchmaking Queue    │           │  • Payouts       │   │
│  └────────────┬────────────┘           └─────────┬────────┘   │
│               │                                   │            │
│               │        ┌──────────────────────────┘            │
│               │        │                                       │
│               ▼        ▼                                       │
│  ┌─────────────────────────────────────────────┐              │
│  │         Registry & Leaderboard Chain         │              │
│  │         (Public/Admin)                       │              │
│  │                                              │              │
│  │  • Global Stats                              │              │
│  │  • Leaderboards                              │              │
│  │  • Character Registry                        │              │
│  │  • Tournament State                          │              │
│  └───────────────────┬──────────────────────────┘              │
│                      │                                         │
│                      ▼                                         │
│  ┌─────────────────────────────────────────────┐              │
│  │         Entropy Oracle Chain                 │              │
│  │         (Admin - VRF Provider)               │              │
│  │                                              │              │
│  │  • VRF Seed Generation                       │              │
│  │  • Randomness Distribution                   │              │
│  │  • Entropy Pool Management                   │              │
│  └──────────────────────────────────────────────┘              │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 🔗 Microchains Design

### 1. Player Microchain (Single-Owner)

**Owner**: Individual player wallet
**Purpose**: Manage player-owned assets, characters, and personal state
**Block Proposal**: Player's wallet proposes blocks directly

#### State Structure

```rust
pub struct PlayerChainState {
    // Owner identity
    pub owner: Owner,

    // Character ownership (NFT references)
    pub characters: Vec<CharacterNFT>,

    // Inventory
    pub items: Vec<Item>,
    pub currencies: HashMap<Currency, u64>, // SOL, USDC, USDT balances

    // Player stats
    pub total_battles: u64,
    pub wins: u64,
    pub losses: u64,
    pub total_earned: HashMap<Currency, u64>,

    // Active battles
    pub active_battles: Vec<ChainId>, // References to battle chains

    // Preferences
    pub default_stance: Stance,
    pub auto_play: bool,
}

pub struct CharacterNFT {
    pub nft_id: String,
    pub class: CharacterClass,
    pub level: u16,
    pub xp: u64,
    pub lives: u8,

    // Stats (cached from Character chain or stored here)
    pub hp_max: u64,
    pub min_damage: u64,
    pub max_damage: u64,
    pub crit_chance: u16,
    pub crit_multiplier: u16,
    pub dodge_chance: u16,
    pub defense: u64,

    // Traits
    pub rarity: u8,
    pub attack_bps: i16,
    pub defense_bps: i16,
    pub crit_bps: i16,

    // Status
    pub in_battle: bool,
    pub current_hp: u64, // Current HP if in battle
}
```

#### Operations (Fast Rounds)

- **Mint/Import Character**: < 100ms (no contention)
- **Update Inventory**: < 100ms
- **View Stats**: Instant (local query)
- **Deposit/Withdraw Currency**: < 200ms (cross-chain)

#### Why Single-Owner?

✅ **Zero gas wars**: Player controls their own chain
✅ **Instant transactions**: No waiting for validators to select txs
✅ **Privacy**: Only owner can see full inventory
✅ **Offline-first**: Can queue actions without internet

---

### 2. Battle Microchain (Multi-Owner)

**Owners**: Two players (Player A & Player B)
**Purpose**: Execute a single battle instance with full combat logic
**Block Proposal**: Both players can propose blocks (multi-leader rounds)

#### State Structure

```rust
pub struct BattleChainState {
    // Battle metadata
    pub battle_id: String,
    pub created_at: Timestamp,
    pub offer_chain: ChainId, // Reference to matchmaking chain

    // Players
    pub player1: PlayerInfo,
    pub player2: PlayerInfo,

    // Stake info
    pub currency: Currency,
    pub stake_amount: u64,
    pub escrow_locked: bool,

    // Battle state
    pub status: BattleStatus, // Pending, Active, Completed, Cancelled
    pub current_round: u8,    // 1, 2, 3
    pub current_turn: u8,     // Whose turn (1 or 2)
    pub turn_number: u64,     // Global turn count

    // Combat state
    pub player1_health: u64,
    pub player2_health: u64,

    // Status effects
    pub player1_dot_damage: u64,
    pub player1_dot_turns: u8,
    pub player1_reflection: bool,
    pub player1_counter_pct: u16,

    pub player2_dot_damage: u64,
    pub player2_dot_turns: u8,
    pub player2_reflection: bool,
    pub player2_counter_pct: u16,

    // Cooldowns
    pub player1_special_cooldown: u8,
    pub player2_special_cooldown: u8,

    // Combat history
    pub turn_history: Vec<TurnResult>,
    pub combo_count: u16,

    // Winner
    pub winner: Option<Owner>,
    pub finalized: bool,
}

pub struct PlayerInfo {
    pub owner: Owner,
    pub player_chain: ChainId,
    pub character: CharacterSnapshot, // Snapshot at battle start
    pub moves_submitted: Vec<TurnMove>, // Queued moves
    pub last_action_time: Timestamp,
}

pub struct TurnResult {
    pub turn: u64,
    pub attacker: u8,
    pub attacker_stance: Stance,
    pub defender_stance: Stance,
    pub damage_dealt: u64,
    pub was_crit: bool,
    pub was_dodge: bool,
    pub special_used: bool,
    pub wildcard_triggered: Option<WildcardEffect>,
    pub attacker_hp_after: u64,
    pub defender_hp_after: u64,
}
```

#### Operations (Multi-Leader Rounds)

- **Initialize Battle**: Player 1 proposes, Player 2 validates (< 500ms)
- **Submit Round Moves**: Each player submits 3 moves (< 300ms)
- **Execute Round**: Combat logic runs (< 800ms for 3 turns)
- **Finalize Battle**: Winner determined, messages sent (< 400ms)

#### Why Multi-Owner?

✅ **Both players control**: Either can propose blocks
✅ **Low latency**: Direct block proposal, no mempool
✅ **Fairness**: Both players see same state
✅ **Isolation**: Battle doesn't affect other battles

#### Message Flow

```
Player 1 Chain               Battle Chain               Player 2 Chain
      │                            │                            │
      │─── Join Battle ───────────►│                            │
      │                            │◄─── Join Battle ───────────│
      │                            │                            │
      │                         [Battle Starts]                 │
      │                            │                            │
      │─── Submit Moves R1 ───────►│                            │
      │                            │◄─── Submit Moves R1 ───────│
      │                         [Execute Round 1]               │
      │                            │                            │
      │◄─── Round Result ──────────│                            │
      │                            │─── Round Result ──────────►│
      │                            │                            │
      │─── Submit Moves R2 ───────►│                            │
      │                            │◄─── Submit Moves R2 ───────│
      │                         [Execute Round 2]               │
      │                            │                            │
      │                      [Battle Ends - P1 Wins]            │
      │                            │                            │
      │◄─── Winner Message ────────│                            │
      │                            │─── Loser Message ──────────►│
      │                            │                            │
      │◄─── Stake + Reward ────────│                            │
      │                            │─── XP Awarded ─────────────►│
```

---

### 3. Matchmaking Chain (Public)

**Owners**: Open (any player can propose blocks)
**Purpose**: Discover opponents, create battle offers, manage matchmaking queues
**Block Proposal**: Public chain, anyone can propose

#### State Structure

```rust
pub struct MatchmakingChainState {
    // Open offers
    pub open_offers: Vec<BattleOffer>,

    // Quick match queue
    pub quick_match_queue: Vec<QueueEntry>,

    // Pending requests
    pub pending_requests: HashMap<String, ChallengeRequest>,

    // Completed offers (for history)
    pub completed_offers: Vec<CompletedOffer>,

    // Stats
    pub total_offers: u64,
    pub total_battles_started: u64,
}

pub struct BattleOffer {
    pub offer_id: String,
    pub creator: Owner,
    pub creator_chain: ChainId,
    pub character_id: String,

    // Stake
    pub currency: Currency,
    pub stake_amount: u64,
    pub escrow_locked: bool,

    // Constraints
    pub min_level: u16,
    pub max_level: u16,
    pub allowed_classes: Vec<CharacterClass>,
    pub auto_approve: bool,

    // Timing
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
    pub inactivity_timeout: i64,

    // Status
    pub status: OfferStatus, // Open, Matched, Cancelled, Expired
    pub challenger: Option<Owner>,
}

pub struct QueueEntry {
    pub player: Owner,
    pub player_chain: ChainId,
    pub character_id: String,
    pub stake_amount: u64,
    pub currency: Currency,
    pub joined_at: Timestamp,
}

pub struct ChallengeRequest {
    pub request_id: String,
    pub offer_id: String,
    pub challenger: Owner,
    pub challenger_chain: ChainId,
    pub character_id: String,
    pub stake_locked: bool,
    pub created_at: Timestamp,
}
```

#### Operations

- **Create Offer**: Any player posts (< 300ms)
- **Browse Offers**: Query state locally (instant)
- **Join Offer**: Submit challenge request (< 300ms)
- **Approve/Reject**: Offer creator responds (< 200ms)
- **Quick Match**: Auto-matching algorithm (< 500ms)

#### Why Public?

✅ **Global visibility**: All players see all offers
✅ **No owner bottleneck**: Anyone can post
✅ **Fair ordering**: Timestamp-based matching
✅ **Decentralized discovery**: No central server

---

### 4. Prediction Market Chain (Public)

**Owners**: Open (any bettor can propose blocks)
**Purpose**: Allow spectators to bet on battle outcomes, manage betting pools
**Block Proposal**: Public chain

#### State Structure

```rust
pub struct PredictionMarketChainState {
    // Active markets (one per battle)
    pub active_markets: HashMap<String, BattleMarket>,

    // Settled markets
    pub settled_markets: Vec<SettledMarket>,

    // User positions
    pub user_positions: HashMap<Owner, Vec<Position>>,

    // Liquidity pools
    pub pools: HashMap<Currency, LiquidityPool>,

    // Stats
    pub total_volume: HashMap<Currency, u64>,
    pub total_bets: u64,
    pub total_bettors: u64,
}

pub struct BattleMarket {
    pub market_id: String,
    pub battle_chain: ChainId,

    // Battle info
    pub player1: Owner,
    pub player2: Owner,
    pub player1_character: CharacterSnapshot,
    pub player2_character: CharacterSnapshot,

    // Betting pools
    pub player1_pool: u64, // Total bet on Player 1
    pub player2_pool: u64, // Total bet on Player 2
    pub total_pool: u64,   // Combined pool

    // Currency
    pub currency: Currency,

    // Odds (calculated)
    pub player1_odds: f64, // e.g., 1.85
    pub player2_odds: f64, // e.g., 2.10

    // Status
    pub status: MarketStatus, // Open, Locked, Settled
    pub betting_closes_at: Timestamp,
    pub settled_at: Option<Timestamp>,
    pub winner: Option<Owner>,

    // Fee
    pub house_fee_bps: u16, // e.g., 300 = 3%
}

pub struct Position {
    pub position_id: String,
    pub market_id: String,
    pub bettor: Owner,
    pub bet_on: Owner, // Which player they bet on
    pub amount: u64,
    pub odds_at_bet: f64,
    pub potential_payout: u64,
    pub claimed: bool,
}

pub struct LiquidityPool {
    pub currency: Currency,
    pub total_liquidity: u64,
    pub available_liquidity: u64,
    pub locked_in_markets: u64,
}
```

#### Betting Flow

```
1. Battle Created on Battle Chain
   └─ Battle Chain sends message to Prediction Market Chain

2. Market Opens
   ├─ Initial odds: 1.5 / 1.5 (50/50)
   ├─ Betting window: Until Round 1 starts
   └─ Anyone can place bets

3. Bettors Place Bets
   ├─ Bet on Player 1 or Player 2
   ├─ Odds adjust dynamically based on pool size
   └─ Example:
       Player 1 Pool: 100 SOL
       Player 2 Pool: 50 SOL
       Total: 150 SOL

       Player 1 Odds: 150 / 100 = 1.5x
       Player 2 Odds: 150 / 50 = 3.0x

4. Betting Closes
   └─ When Round 1 execution starts, market locks

5. Battle Completes
   └─ Battle Chain sends winner message to Prediction Market

6. Market Settles
   ├─ Calculate payouts:
       Winner Pool: 50 SOL
       Loser Pool: 100 SOL (redistributed)
       House Fee: 3% of loser pool = 3 SOL
       Payout Pool: 100 - 3 = 97 SOL

   ├─ Each winner gets: (their bet / winner pool) * payout pool
   └─ Example: Bet 10 SOL on winner
       Payout: (10 / 50) * 97 = 19.4 SOL
       Profit: 9.4 SOL

7. Bettors Claim Winnings
   └─ Winners withdraw from Prediction Market Chain
```

#### Dynamic Odds Algorithm

```rust
fn calculate_odds(player_pool: u64, total_pool: u64) -> f64 {
    if player_pool == 0 {
        return 99.0; // Max odds
    }

    let odds = (total_pool as f64) / (player_pool as f64);

    // Apply house edge (e.g., 3% margin)
    let odds_with_margin = odds * 0.97;

    // Cap odds
    odds_with_margin.min(50.0).max(1.01)
}

// Real-time odds update
fn update_odds_on_bet(market: &mut BattleMarket, bet_on: Owner, amount: u64) {
    if bet_on == market.player1 {
        market.player1_pool += amount;
    } else {
        market.player2_pool += amount;
    }

    market.total_pool += amount;

    market.player1_odds = calculate_odds(market.player1_pool, market.total_pool);
    market.player2_odds = calculate_odds(market.player2_pool, market.total_pool);
}
```

#### Why Public Chain for Prediction Market?

✅ **Open participation**: Anyone can bet
✅ **Transparent odds**: All calculations visible on-chain
✅ **Fair settlement**: Automated payout distribution
✅ **Liquidity pooling**: Aggregated across all bettors
✅ **Real-time updates**: Odds change as bets come in

---

### 5. Registry & Leaderboard Chain (Public/Admin)

**Owners**: Admin (for writes), Open (for reads)
**Purpose**: Global game state, leaderboards, character registry, tournament management
**Block Proposal**: Admin proposes, but anyone can query

#### State Structure

```rust
pub struct RegistryChainState {
    // Global stats
    pub total_characters: u64,
    pub total_battles: u64,
    pub total_battles_completed: u64,
    pub total_volume: HashMap<Currency, u64>,

    // Character registry (for lookups)
    pub characters: HashMap<String, CharacterRegistryEntry>,

    // Leaderboards
    pub global_leaderboard: Vec<LeaderboardEntry>,
    pub class_leaderboards: HashMap<CharacterClass, Vec<LeaderboardEntry>>,
    pub level_tier_leaderboards: HashMap<LevelTier, Vec<LeaderboardEntry>>,

    // Tournaments
    pub active_tournaments: Vec<Tournament>,
    pub tournament_history: Vec<CompletedTournament>,

    // Configuration
    pub config: GameConfig,
}

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
    pub total_earnings: HashMap<Currency, u64>,
}

pub struct Tournament {
    pub tournament_id: String,
    pub name: String,
    pub format: TournamentFormat, // SingleElim, DoubleElim, RoundRobin
    pub entry_fee: u64,
    pub currency: Currency,
    pub prize_pool: u64,
    pub max_participants: u64,
    pub participants: Vec<Owner>,
    pub bracket: TournamentBracket,
    pub status: TournamentStatus,
    pub starts_at: Timestamp,
}
```

#### Operations

- **Register Character**: Battle Chain → Registry (< 200ms)
- **Update Stats**: Battle Chain → Registry after battle (< 300ms)
- **Query Leaderboard**: Local query (instant)
- **Create Tournament**: Admin proposes (< 500ms)
- **Join Tournament**: Player sends message (< 300ms)

#### Leaderboard Update Flow

```
Battle Chain (Battle Ends)
      │
      │─── Send Battle Result Message ────►  Registry Chain
      │                                            │
      │                                       [Update Stats]
      │                                       • Increment wins/losses
      │                                       • Recalculate win rate
      │                                       • Update ELO rating
      │                                       • Update damage stats
      │                                            │
      │                                       [Update Leaderboard]
      │                                       • Re-rank if needed
      │                                       • Emit event
      │                                            │
      │◄─── Acknowledgment ───────────────────────│
```

#### Why Public/Admin Chain?

✅ **Global visibility**: Everyone sees leaderboards
✅ **Single source of truth**: One canonical ranking
✅ **Controlled writes**: Admin prevents spam/abuse
✅ **Fast reads**: Local queries, no network calls

---

### 6. Entropy Oracle Chain (Admin)

**Owners**: VRF Oracle Service (single admin)
**Purpose**: Generate and distribute cryptographically secure randomness
**Block Proposal**: Oracle service only

#### State Structure

```rust
pub struct EntropyChainState {
    // VRF oracle identity
    pub oracle: Owner,
    pub vrf_public_key: [u8; 32],

    // Entropy pool
    pub seed_batches: Vec<SeedBatch>,
    pub next_seed_index: u64,
    pub total_seeds_generated: u64,
    pub total_seeds_consumed: u64,

    // Statistics
    pub seeds_per_batch: u32,
    pub last_refill: Timestamp,
    pub refill_threshold: u64, // Refill when < 100 seeds left
}

pub struct SeedBatch {
    pub batch_id: u64,
    pub start_index: u64,
    pub count: u32,
    pub seeds: Vec<[u8; 32]>,
    pub vrf_proof: Vec<u8>, // Cryptographic proof
    pub created_at: Timestamp,
}
```

#### Entropy Consumption Protocol

```
Battle Chain                    Entropy Chain
      │                               │
      │─── Request Entropy ──────────►│
      │    (for Round 1)               │
      │                           [Generate/Fetch]
      │                           • Get next seed
      │                           • Increment index
      │                           • Verify VRF proof
      │                               │
      │◄─── Entropy Response ─────────│
      │    [seed: [u8; 32]]            │
      │                               │
      │                          [Check Threshold]
      │                          If seeds < 100:
      │                            Refill batch
      │
    [Use Seed]
    • Derive stance RNG
    • Derive crit RNG
    • Derive dodge RNG
    • Derive wildcard RNG
```

#### Seed Derivation (Same as Solana)

```rust
fn derive_u64_from_seed(seed: &[u8; 32], tag: u8) -> u64 {
    // Hash seed with tag to get unique value
    let hash = hash_with_tag(seed, tag);

    // Convert first 8 bytes to u64
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash[0..8]);
    u64::from_le_bytes(bytes)
}

// In battle execution
let entropy_seed = request_entropy_from_chain().await;

let stance_rng = derive_u64_from_seed(&entropy_seed, 0);
let crit_rng = derive_u64_from_seed(&entropy_seed, 1);
let dodge_rng = derive_u64_from_seed(&entropy_seed, 2);
let wildcard_rng = derive_u64_from_seed(&entropy_seed, 3);

// 1 seed → 4 random values (same as Solana optimization)
```

#### Why Dedicated Entropy Chain?

✅ **Centralized randomness**: One source of truth
✅ **VRF security**: Cryptographically verifiable
✅ **Efficient distribution**: Cross-chain messages
✅ **Predictable cost**: Known entropy consumption
✅ **Auditability**: All seeds recorded on-chain

---

## 🔄 Cross-Chain Messaging

### Message Types

#### 1. Battle Lifecycle Messages

```rust
// Player Chain → Matchmaking Chain
pub struct CreateOfferMessage {
    pub offer_id: String,
    pub character_id: String,
    pub stake_amount: u64,
    pub currency: Currency,
    pub constraints: MatchConstraints,
}

// Matchmaking Chain → Battle Chain
pub struct InitializeBattleMessage {
    pub battle_id: String,
    pub player1: Owner,
    pub player1_chain: ChainId,
    pub player1_character: CharacterSnapshot,
    pub player2: Owner,
    pub player2_chain: ChainId,
    pub player2_character: CharacterSnapshot,
    pub stake_amount: u64,
    pub currency: Currency,
}

// Battle Chain → Player Chain
pub struct BattleResultMessage {
    pub battle_id: String,
    pub winner: Owner,
    pub loser: Owner,
    pub xp_earned: u64,
    pub stake_won: u64,
    pub currency: Currency,
}

// Battle Chain → Registry Chain
pub struct UpdateStatsMessage {
    pub character_id: String,
    pub battle_result: BattleOutcome,
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub turns_played: u64,
}
```

#### 2. Prediction Market Messages

```rust
// Battle Chain → Prediction Market Chain
pub struct CreateMarketMessage {
    pub battle_id: String,
    pub battle_chain: ChainId,
    pub player1: Owner,
    pub player2: Owner,
    pub player1_character: CharacterSnapshot,
    pub player2_character: CharacterSnapshot,
    pub betting_window_end: Timestamp,
}

// Battle Chain → Prediction Market Chain
pub struct SettleMarketMessage {
    pub battle_id: String,
    pub winner: Owner,
    pub final_stats: BattleFinalStats,
}

// Prediction Market Chain → Player Chain (Bettor)
pub struct PayoutMessage {
    pub market_id: String,
    pub bettor: Owner,
    pub original_bet: u64,
    pub payout: u64,
    pub currency: Currency,
    pub profit: u64,
}
```

#### 3. Currency Transfer Messages

```rust
// Player Chain → Matchmaking Chain (Escrow)
pub struct LockStakeMessage {
    pub offer_id: String,
    pub player: Owner,
    pub amount: u64,
    pub currency: Currency,
}

// Matchmaking Chain → Battle Chain (Transfer Escrow)
pub struct TransferEscrowMessage {
    pub battle_id: String,
    pub total_stake: u64,
    pub currency: Currency,
}

// Battle Chain → Player Chain (Payout)
pub struct ReleaseStakeMessage {
    pub battle_id: String,
    pub recipient: Owner,
    pub amount: u64,
    pub currency: Currency,
}
```

---

## 📊 Complete Data Flow Examples

### Example 1: Full Battle Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Step 1: Player 1 Creates Battle Offer                           │
└─────────────────────────────────────────────────────────────────┘

Player 1 Chain
  │
  │ [Player 1 Wallet Proposes Block]
  │ • Create offer: 1 SOL stake
  │ • Character: Warrior #123, Level 5
  │ • Lock 1 SOL in escrow (local state)
  │
  ├─── CreateOfferMessage ───────────► Matchmaking Chain
  │                                         │
  │                                    [Add Offer]
  │                                    • offer_id: "ABC123"
  │                                    • status: Open
  │                                    • stake: 1 SOL locked
  │                                         │
  │◄─── OfferCreatedAck ──────────────────┘


┌─────────────────────────────────────────────────────────────────┐
│ Step 2: Player 2 Joins Offer                                    │
└─────────────────────────────────────────────────────────────────┘

Player 2 Chain
  │
  │ [Player 2 Wallet Proposes Block]
  │ • Find offer "ABC123"
  │ • Character: Assassin #456, Level 5
  │ • Lock 1 SOL in escrow (local state)
  │
  ├─── JoinOfferMessage ─────────────► Matchmaking Chain
  │                                         │
  │                                    [Create Request]
  │                                    • request_id: "REQ789"
  │                                    • challenger: Player 2
  │                                         │
  │                                    [Notify Player 1]
  │                                         │
  ├────────────────────────────────────────┼─────────► Player 1 Chain
  │                                         │               │
  │                                         │          [Notification]
  │                                         │          "New challenger!"


┌─────────────────────────────────────────────────────────────────┐
│ Step 3: Player 1 Approves → Battle Initializes                  │
└─────────────────────────────────────────────────────────────────┘

Player 1 Chain
  │
  │ [Player 1 Approves]
  ├─── ApproveRequestMessage ────────► Matchmaking Chain
  │                                         │
  │                                    [Create Battle]
  │                                    • battle_id: "BAT999"
  │                                    • Transfer escrow to battle
  │                                         │
  │                                    ├─── InitializeBattleMessage ───► Battle Chain (NEW!)
  │                                    │                                       │
  │                                    │                                  [Initialize]
  │                                    │                                  • 2 SOL locked
  │                                    │                                  • Players: P1, P2
  │                                    │                                  • Status: Active
  │                                    │                                  • Round: 0
  │                                    │                                       │
  │                                    │                                  [Send to Prediction]
  │                                    │                                       │
  │                                    │                                       ├─► Prediction Market Chain
  │                                    │                                       │        │
  │                                    │                                       │   [Create Market]
  │                                    │                                       │   • market_id: "MKT999"
  │                                    │                                       │   • Initial odds: 1.5/1.5
  │                                    │                                       │   • Betting: OPEN
  │                                    │                                       │
  │◄─── BattleInitialized ────────────┴───────────────────────────────────────┘
  │
Player 2 Chain
  │
  │◄─── BattleInitialized ────────────────────────────────────────────────────


┌─────────────────────────────────────────────────────────────────┐
│ Step 3.5: Spectators Place Bets (Parallel to Battle)            │
└─────────────────────────────────────────────────────────────────┘

Spectator Chain
  │
  │ [Spectator views market]
  │ • Player 1 (Warrior L5) vs Player 2 (Assassin L5)
  │ • Current odds: 1.5 / 1.5
  │
  ├─── PlaceBetMessage ───────────────► Prediction Market Chain
  │    (Bet 0.5 SOL on Player 1)             │
  │                                      [Update Pool]
  │                                      • P1 pool: 0 → 0.5 SOL
  │                                      • Total: 0.5 SOL
  │                                      • P1 odds: 1.0 (initial bet)
  │                                           │
  │◄─── BetConfirmed ────────────────────────┘
  │    Position ID: "POS001"

Another Spectator
  │
  ├─── PlaceBetMessage ───────────────► Prediction Market Chain
  │    (Bet 1 SOL on Player 2)               │
  │                                      [Update Pool]
  │                                      • P2 pool: 0 → 1 SOL
  │                                      • Total: 1.5 SOL
  │                                      • P1 odds: 1.5 / 0.5 = 3.0x
  │                                      • P2 odds: 1.5 / 1 = 1.5x
  │                                           │
  │◄─── BetConfirmed ────────────────────────┘

[Betting continues until Round 1 starts]


┌─────────────────────────────────────────────────────────────────┐
│ Step 4: Round 1 Execution                                        │
└─────────────────────────────────────────────────────────────────┘

Player 1 Chain
  │
  │ [Player 1 submits moves]
  ├─── SubmitMovesMessage ────────────► Battle Chain
  │    Round 1: [Aggressive, Balanced, Defensive]    │
  │                                                   │
  │                                              [Queue Moves]
  │                                              P1 moves stored
  │
Player 2 Chain
  │
  │ [Player 2 submits moves]
  ├─── SubmitMovesMessage ────────────► Battle Chain
  │    Round 1: [Defensive, Aggressive, Counter]     │
  │                                                   │
  │                                              [Both Ready]
  │                                              Execute Round 1
  │                                                   │
  │                                              ├─── RequestEntropyMessage ───► Entropy Chain
  │                                              │                                     │
  │                                              │                                [Provide Seed]
  │                                              │◄─── EntropyResponse ───────────────┘
  │                                              │    seed: 0x1234...
  │                                              │
  │                                              [Execute 3 Turns]
  │                                              • Turn 1: P1 ATK (Aggressive) → P2 DEF (Defensive)
  │                                              •   Damage: 12 * 1.3 / 0.5 = 31 dmg
  │                                              •   P2: 90 HP → 59 HP
  │                                              │
  │                                              • Turn 2: P2 ATK (Aggressive) → P1 DEF (Balanced)
  │                                              •   Damage: 18 * 1.3 / 1.0 = 23 dmg
  │                                              •   P1: 120 HP → 97 HP
  │                                              │
  │                                              • Turn 3: P1 ATK (Defensive) → P2 DEF (Counter)
  │                                              •   Damage: 10 * 0.7 / 1.0 = 7 dmg
  │                                              •   Counter: 7 * 0.4 = 3 dmg back
  │                                              •   P2: 59 → 52 HP
  │                                              •   P1: 97 → 94 HP
  │                                              │
  │                                              [Round 1 Complete]
  │                                              • P1 HP: 94/120
  │                                              • P2 HP: 52/90
  │                                              │
  ├────────────────────────────────────────────────┼────────────────► Player 1 Chain
  │                                              │                         │
  │                                              │                    [Update State]
  │                                              │                    Character HP: 94
  │                                              │
  │                                              ├────────────────────────────────────► Player 2 Chain
  │                                              │                                          │
  │                                              │                                     [Update State]
  │                                              │                                     Character HP: 52
  │                                              │
  │                                              ├─── RoundCompleteMessage ───────────► Prediction Market Chain
  │                                              │                                          │
  │                                              │                                     [Update Market]
  │                                              │                                     • Live stats shown
  │                                              │                                     • Betting now LOCKED


┌─────────────────────────────────────────────────────────────────┐
│ Step 5: Round 2 & 3 (Similar Flow)                              │
└─────────────────────────────────────────────────────────────────┘

Battle Chain
  │
  [Round 2 Execution]
  • P1 HP: 94 → 78
  • P2 HP: 52 → 25 (critical hit!)
  │
  [Round 3 - Turn 1]
  • P1 uses special: Battle Fury (+50% dmg)
  • Deals 22 * 1.5 = 33 damage
  • P2 HP: 25 → 0 (KNOCKOUT!)
  │
  [Battle Ends - Player 1 Wins]
  • Winner: Player 1
  • Final HP: P1 78, P2 0
  • Turns played: 7 (ended early)


┌─────────────────────────────────────────────────────────────────┐
│ Step 6: Battle Finalization & Payouts                           │
└─────────────────────────────────────────────────────────────────┘

Battle Chain
  │
  [Finalize Battle]
  ├─── BattleResultMessage ──────────► Player 1 Chain
  │                                         │
  │                                    [Update Character]
  │                                    • Wins: +1
  │                                    • XP: +150
  │                                    • Level: 5 → 6!
  │                                    • Unlock 1.9 SOL
  │                                         │
  ├─── BattleResultMessage ──────────────────────────────► Player 2 Chain
  │                                                             │
  │                                                        [Update Character]
  │                                                        • Losses: +1
  │                                                        • XP: +50
  │                                                        • Lives: 3 → 3 (not 3rd loss)
  │
  ├─── UpdateStatsMessage ────────────► Registry Chain
  │                                         │
  │                                    [Update Leaderboard]
  │                                    • P1: Rank 142 → 138
  │                                    • P2: Rank 89 → 91
  │
  ├─── SettleMarketMessage ───────────► Prediction Market Chain
  │                                         │
  │                                    [Settle Market]
  │                                    • Winner: Player 1
  │                                    • P1 pool: 0.5 SOL
  │                                    • P2 pool: 1 SOL
  │                                    • House fee: 3% of 1 SOL = 0.03 SOL
  │                                    • Payout pool: 0.97 SOL
  │                                         │
  │                                    [Calculate Payouts]
  │                                    • Spectator 1 bet 0.5 SOL on P1:
  │                                    •   Payout: 0.5 + (0.5/0.5 * 0.97) = 1.47 SOL
  │                                    •   Profit: 0.97 SOL (194% return!)
  │                                         │
  │                                    ├─── PayoutMessage ──────────► Spectator 1 Chain
  │                                    │                                   │
  │                                    │                              [Credit Account]
  │                                    │                              +1.47 SOL


Full Flow Complete: ~3-5 seconds total (including all rounds)
```

---

### Example 2: Quick Match Flow

```
┌─────────────────────────────────────────────────────────────────┐
│ Quick Match: Auto-Matching System                               │
└─────────────────────────────────────────────────────────────────┘

Player 1 Chain
  │
  ├─── JoinQueueMessage ──────────────► Matchmaking Chain
  │    • Character: Mage #789, Level 8       │
  │    • Stake: 0.5 SOL                      │
  │                                      [Add to Queue]
  │                                      Queue: [P1]
  │                                           │
Player 2 Chain                                │
  │                                           │
  ├─── JoinQueueMessage ─────────────────────►│
  │    • Character: Tank #234, Level 7        │
  │    • Stake: 0.5 SOL                       │
  │                                      [Match Found!]
  │                                      • Level diff: 1 (OK)
  │                                      • Stake match: ✓
  │                                      • Create battle
  │                                           │
  │                                      ├─── InitializeBattleMessage ──► Battle Chain
  │                                      │
  │◄─── MatchFoundMessage ───────────────┤
  │                                      │
  │                                      ├───────────────────────────────► Player 1 Chain
  │                                      │
  │                                      └─── MatchFoundMessage ──────────► Player 2 Chain

[Battle proceeds normally]
```

---

## 🎮 Solidity to Linera Migration Guide

### Key Differences for Solidity Developers

| Concept | Solidity (Solana) | Linera |
|---------|-------------------|--------|
| **State Storage** | Single contract state | Distributed across microchains |
| **Transactions** | Submit to mempool | Propose blocks directly |
| **Function Calls** | Synchronous | Asynchronous (cross-chain messages) |
| **Randomness** | VRF Oracle (Switchboard/Pyth) | Entropy microchain |
| **Gas/Fees** | Per-transaction compute units | Block proposal (minimal) |
| **User Identity** | Wallet address | Owner (wallet + chain ID) |
| **Escrow** | Token accounts | Cross-chain messages |
| **Events** | Emit logs | Cross-chain messages |

### Architecture Translation

#### Solana Approach (Monolithic)

```rust
// Single program handles everything
pub mod battlechain {
    // All state in program accounts
    pub struct Config { /* ... */ }
    pub struct Character { /* ... */ }
    pub struct Battle { /* ... */ }
    pub struct Offer { /* ... */ }

    // All logic in one program
    pub fn create_character() { /* ... */ }
    pub fn create_offer() { /* ... */ }
    pub fn execute_turn() { /* ... */ }
}

// Problems:
// ❌ All battles share compute budget
// ❌ Account size limits
// ❌ Mempool contention
// ❌ Sequential execution
```

#### Linera Approach (Distributed)

```rust
// Multiple microchains, each with focused state

// Player Microchain Application
pub struct PlayerChainApp {
    state: PlayerChainState,
}

impl PlayerChainApp {
    // Local operations (fast!)
    pub fn view_characters() { /* instant */ }
    pub fn update_preferences() { /* < 100ms */ }

    // Cross-chain operations
    pub fn create_offer() {
        // Send message to matchmaking chain
        send_message(matchmaking_chain, CreateOfferMessage { /* ... */ });
    }
}

// Battle Microchain Application
pub struct BattleChainApp {
    state: BattleChainState,
}

impl BattleChainApp {
    // Battle-specific logic
    pub fn execute_round() {
        // 1. Request entropy
        let seed = request_entropy().await;

        // 2. Execute 3 turns
        for turn in 0..3 {
            self.execute_turn(seed, turn);
        }

        // 3. Notify players
        send_message(player1_chain, RoundResultMessage { /* ... */ });
        send_message(player2_chain, RoundResultMessage { /* ... */ });
    }
}

// Benefits:
// ✅ Battles execute in parallel
// ✅ No state size limits
// ✅ No mempool delays
// ✅ Isolated failures
```

### Code Migration Examples

#### Example 1: Creating a Character

**Solana (Anchor)**

```rust
#[program]
pub mod battlechain {
    pub fn create_character(
        ctx: Context<CreateCharacter>,
        class: CharacterClass,
    ) -> Result<()> {
        let character = &mut ctx.accounts.character;
        let nft = &ctx.accounts.nft;

        // Verify NFT ownership
        require!(nft.owner == ctx.accounts.player.key(), ErrorCode::NotOwner);

        // Initialize character
        character.owner = ctx.accounts.player.key();
        character.class = class;
        character.level = 1;
        character.hp = class.base_hp();
        // ... more initialization

        Ok(())
    }
}
```

**Linera (Rust SDK)**

```rust
use linera_sdk::{base::Owner, Application};

pub struct PlayerChainApp {
    state: PlayerChainState,
}

impl Application for PlayerChainApp {
    // Operation: Create character (local, fast!)
    async fn execute_operation(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::CreateCharacter { nft_id, class } => {
                // Verify NFT ownership (local check)
                require!(self.verify_nft_ownership(&nft_id), "Not owner");

                // Create character
                let character = CharacterNFT {
                    nft_id,
                    class,
                    level: 1,
                    hp_max: class.base_hp(),
                    // ... more fields
                };

                self.state.characters.push(character);

                // Register with global registry (cross-chain message)
                self.send_message(
                    registry_chain_id(),
                    Message::RegisterCharacter {
                        character_id: nft_id.clone(),
                        owner: self.owner(),
                        class,
                    }
                );

                Ok(())
            }
        }
    }
}

// Key differences:
// ✅ No transaction fees for local operations
// ✅ Instant execution (< 100ms)
// ✅ Cross-chain message for global state
```

#### Example 2: Battle Execution

**Solana (Anchor)**

```rust
#[program]
pub mod battlechain {
    pub fn execute_turn(
        ctx: Context<ExecuteTurn>,
        preference: i8,
        use_special: bool,
    ) -> Result<()> {
        let battle = &mut ctx.accounts.battle;
        let entropy_pool = &mut ctx.accounts.entropy_pool;

        // Consume entropy (account mutation)
        let seed = entropy_pool.consume_next_seed()?;

        // Execute combat logic
        let result = execute_combat_logic(
            battle,
            seed,
            preference,
            use_special,
        )?;

        // Update battle state
        battle.current_turn = 3 - battle.current_turn; // Switch turns
        battle.turn_number += 1;

        // Emit event
        emit!(TurnExecuted {
            battle_id: battle.key(),
            damage: result.damage,
            // ...
        });

        Ok(())
    }
}
```

**Linera (Rust SDK)**

```rust
impl Application for BattleChainApp {
    async fn execute_operation(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::ExecuteRound { round_moves } => {
                // Request entropy (cross-chain message)
                let seed = self.request_entropy().await?;

                // Execute 3 turns in one operation (optimized!)
                for (turn_idx, turn_move) in round_moves.moves.iter().enumerate() {
                    let result = self.execute_turn(
                        seed,
                        turn_idx as u8,
                        turn_move.preference,
                        turn_move.use_special,
                    );

                    // Early exit if knockout
                    if result.knockout {
                        break;
                    }
                }

                // Increment round
                self.state.current_round += 1;

                // Notify both players (cross-chain messages)
                self.send_to_player1(Message::RoundComplete { /* ... */ });
                self.send_to_player2(Message::RoundComplete { /* ... */ });

                // Update prediction market
                self.send_to_market(Message::RoundUpdate { /* ... */ });

                Ok(())
            }
        }
    }

    // Helper: Request entropy from oracle chain
    async fn request_entropy(&self) -> Result<[u8; 32]> {
        let response = self.call_application(
            entropy_chain_id(),
            Query::RequestSeed,
        ).await?;

        Ok(response.seed)
    }
}

// Key differences:
// ✅ Async cross-chain calls
// ✅ 3 turns in 1 operation (batching)
// ✅ Messages instead of events
// ✅ Direct player notification
```

#### Example 3: Escrow & Payouts

**Solana (Anchor)**

```rust
#[program]
pub mod battlechain {
    pub fn create_offer(
        ctx: Context<CreateOffer>,
        stake_amount: u64,
    ) -> Result<()> {
        // Transfer SOL to escrow
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.player.to_account_info(),
                    to: ctx.accounts.escrow.to_account_info(),
                },
            ),
            stake_amount,
        )?;

        // Store offer
        let offer = &mut ctx.accounts.offer;
        offer.creator = ctx.accounts.player.key();
        offer.stake_amount = stake_amount;

        Ok(())
    }

    pub fn finalize_battle(ctx: Context<FinalizeBattle>) -> Result<()> {
        let battle = &ctx.accounts.battle;
        let escrow = &ctx.accounts.escrow;

        // Calculate payout
        let total_stake = battle.stake_amount * 2;
        let fee = total_stake * 5 / 100; // 5%
        let winner_amount = total_stake - fee;

        // Transfer to winner
        **escrow.to_account_info().try_borrow_mut_lamports()? -= winner_amount;
        **ctx.accounts.winner.to_account_info().try_borrow_mut_lamports()? += winner_amount;

        // Transfer fee to treasury
        **escrow.to_account_info().try_borrow_mut_lamports()? -= fee;
        **ctx.accounts.treasury.to_account_info().try_borrow_mut_lamports()? += fee;

        Ok(())
    }
}
```

**Linera (Rust SDK)**

```rust
// Player Chain: Lock stake locally
impl Application for PlayerChainApp {
    async fn execute_operation(&mut self, operation: Operation) -> Result<()> {
        match operation {
            Operation::CreateOffer { stake_amount, currency } => {
                // Lock stake in local chain state
                self.state.currencies.get_mut(&currency)
                    .ok_or("Insufficient balance")?
                    .checked_sub(stake_amount)
                    .ok_or("Insufficient balance")?;

                self.state.locked_stakes.insert(offer_id, stake_amount);

                // Send message to matchmaking chain
                self.send_message(
                    matchmaking_chain_id(),
                    Message::CreateOffer {
                        stake_amount,
                        currency,
                        // ...
                    }
                );

                Ok(())
            }
        }
    }

    // Handle incoming payout message
    async fn handle_message(&mut self, message: Message) -> Result<()> {
        match message {
            Message::BattlePayout { amount, currency } => {
                // Credit winnings
                *self.state.currencies.get_mut(&currency).unwrap() += amount;

                // Update stats
                self.state.total_earned.entry(currency)
                    .and_modify(|e| *e += amount)
                    .or_insert(amount);

                Ok(())
            }
        }
    }
}

// Battle Chain: Distribute payouts
impl Application for BattleChainApp {
    async fn finalize_battle(&mut self) -> Result<()> {
        let total_stake = self.state.stake_amount * 2;
        let fee = total_stake * 5 / 100;
        let winner_amount = total_stake - fee;

        // Send payout to winner
        self.send_message(
            self.state.winner_chain_id,
            Message::BattlePayout {
                amount: winner_amount,
                currency: self.state.currency,
            }
        );

        // Send fee to treasury chain
        self.send_message(
            treasury_chain_id(),
            Message::CollectFee {
                amount: fee,
                currency: self.state.currency,
            }
        );

        Ok(())
    }
}

// Key differences:
// ✅ No token account management
// ✅ No CPI (Cross-Program Invocation)
// ✅ Messages handle transfers
// ✅ Each chain manages its own balances
```

---

## ⚡ Performance Optimizations

### Optimization 1: Batched Turn Execution

**Solana**: 9 turns = 9 transactions = ~1,200,000 CU

**Linera**: 3 rounds = 3 blocks = ~900ms total

```rust
// Linera: Execute 3 turns in one block
pub async fn execute_round(&mut self, round_moves: RoundMoves) -> Result<()> {
    let seed = self.request_entropy().await?;

    // Turn 1
    self.execute_turn(seed, 0, &round_moves.moves[0]);
    if self.battle_ended() { return Ok(()); }

    // Turn 2
    self.execute_turn(seed, 1, &round_moves.moves[1]);
    if self.battle_ended() { return Ok(()); }

    // Turn 3
    self.execute_turn(seed, 2, &round_moves.moves[2]);

    Ok(())
}

// Performance:
// ✅ 1 entropy request (not 3)
// ✅ 1 block finalization (not 3)
// ✅ 1 cross-chain message batch (not 3)
// ✅ Sub-800ms for full round
```

### Optimization 2: Parallel Battle Execution

**Solana**: Battles share validator compute budget, sequential execution

**Linera**: Each battle on its own microchain, unlimited parallelism

```
Solana (1 validator, 400k CU/tx limit):
Battle 1: ████████░░ (200k CU)
Battle 2:         ████████░░ (waits for Battle 1)
Battle 3:                   ████████░░ (waits for Battle 2)

Time: 3 × 500ms = 1.5 seconds


Linera (parallel microchains):
Battle 1: ████████
Battle 2: ████████
Battle 3: ████████
  ...
Battle 100: ████████

Time: 500ms (all execute in parallel!)
```

### Optimization 3: Local Queries (No Network Calls)

**Solana**: Every read = RPC call to validator

**Linera**: Local Wasm VM executes queries instantly

```rust
// Solana: Network call required
const character = await program.account.character.fetch(characterPubkey);
// ~100-300ms depending on RPC node

// Linera: Instant local query
const character = await playerChain.query.getCharacter(characterId);
// < 1ms (local Wasm execution)
```

### Optimization 4: Sparse Client (Only Relevant Chains)

**Solana**: Light client must track entire blockchain state

**Linera**: Client only tracks player's chains + active battles

```
Solana Client:
├─ Global state: 500 GB
├─ Account snapshots: 100 GB
└─ Recent blocks: 10 GB

Total: 610 GB (or trust RPC node)


Linera Client:
├─ Player chain: 10 MB
├─ Active battle chains (3): 3 MB
├─ Matchmaking chain (recent): 5 MB
└─ Prediction markets (active): 2 MB

Total: 20 MB (self-sovereign!)
```

---

## 🚀 Implementation Roadmap

### Phase 1: Core Infrastructure (Months 1-2)

**Goal**: Basic microchains + character system

- [ ] Set up Linera devnet
- [ ] Implement Player microchain application
  - [ ] Character creation
  - [ ] Inventory management
  - [ ] Balance tracking
- [ ] Implement Registry microchain
  - [ ] Character registry
  - [ ] Basic leaderboard
- [ ] Implement Entropy microchain
  - [ ] VRF integration
  - [ ] Seed distribution

**Deliverable**: Players can create characters, view stats, see leaderboard

---

### Phase 2: Battle System (Months 3-4)

**Goal**: 1v1 battles with full combat logic

- [ ] Implement Matchmaking microchain
  - [ ] Create offers
  - [ ] Join offers
  - [ ] Quick match queue
- [ ] Implement Battle microchain
  - [ ] Battle initialization
  - [ ] Round execution (3 turns batched)
  - [ ] Combat logic (stances, crits, dodge)
  - [ ] Special abilities
  - [ ] Wildcards
- [ ] Cross-chain messaging
  - [ ] Player ↔ Matchmaking
  - [ ] Matchmaking ↔ Battle
  - [ ] Battle ↔ Player
  - [ ] Battle ↔ Registry

**Deliverable**: Fully functional battles with stakes

---

### Phase 3: Prediction Markets (Month 5)

**Goal**: Spectators can bet on battles

- [ ] Implement Prediction Market microchain
  - [ ] Market creation (on battle start)
  - [ ] Bet placement
  - [ ] Dynamic odds calculation
  - [ ] Market settlement
  - [ ] Payout distribution
- [ ] Integrate with Battle chain
  - [ ] Battle → Market: CreateMarket message
  - [ ] Battle → Market: SettleMarket message
  - [ ] Market → Bettor: Payout message

**Deliverable**: Live prediction markets on all battles

---

### Phase 4: Frontend & UX (Month 6)

**Goal**: User-friendly web interface

- [ ] React frontend with GraphQL
  - [ ] Connect to player's wallet (Dynamic integration)
  - [ ] View characters & inventory
  - [ ] Browse matchmaking offers
  - [ ] Watch live battles
  - [ ] Place bets on prediction markets
- [ ] Real-time updates
  - [ ] WebSocket notifications
  - [ ] Live battle animations
  - [ ] Odds ticker

**Deliverable**: Production-ready web app

---

### Phase 5: Advanced Features (Months 7-8)

**Goal**: Tournaments, team battles, governance

- [ ] Tournament system
  - [ ] Bracket generation
  - [ ] Tournament microchains
  - [ ] Prize pool distribution
- [ ] Team battles (2v2, 3v3)
  - [ ] Multi-player battle chains
  - [ ] Combo abilities
- [ ] Governance
  - [ ] DAO voting on game balance
  - [ ] Community proposals

**Deliverable**: Complete game ecosystem

---

## 📊 Expected Performance Metrics

| Metric | Solana (Current) | Linera (Projected) | Improvement |
|--------|------------------|-------------------|-------------|
| **Battle Creation** | ~500ms | < 200ms | 2.5x faster |
| **Turn Execution** | ~400ms per turn | ~800ms per round (3 turns) | 67% fewer transactions |
| **Battle Finality** | 9 × 400ms = 3.6s | 3 × 800ms = 2.4s | 33% faster |
| **Query Latency** | 100-300ms (RPC) | < 1ms (local) | 300x faster |
| **Concurrent Battles** | Limited by validator | Unlimited | ∞ |
| **Gas Fees** | ~0.0005 SOL/turn | ~0 (block proposal) | 100% reduction |
| **Character Operations** | ~200ms | < 100ms | 2x faster |
| **Prediction Bet** | ~300ms | < 200ms | 1.5x faster |

---

## 🎯 Summary: Why Linera for BattleChain?

### ✅ Perfect Fit

1. **Real-Time Combat**: Sub-500ms finality makes battles feel instant
2. **Unlimited Scaling**: Each battle = own chain = infinite parallelism
3. **Zero Gas Wars**: Players control their own chains
4. **True Ownership**: Players host their own inventory data
5. **Instant Queries**: Local Wasm VM for fast reads
6. **Prediction Markets**: Public chains perfect for betting pools
7. **Fair Randomness**: Dedicated entropy chain for VRF
8. **Economic Security**: Cross-chain messages handle escrow

### 🔄 Migration Effort

| Component | Difficulty | Time Estimate |
|-----------|------------|---------------|
| Player Chain | Medium | 2 weeks |
| Battle Chain | High | 4 weeks |
| Matchmaking | Low | 1 week |
| Prediction Market | Medium | 2 weeks |
| Entropy Chain | Medium | 2 weeks |
| Registry Chain | Low | 1 week |
| Cross-Chain Messages | High | 3 weeks |
| Frontend Integration | Medium | 3 weeks |
| **Total** | - | **18 weeks (~4.5 months)** |

### 🚀 Next Steps

1. **Learn Linera SDK**: Study Linera Rust SDK documentation
2. **Set Up Devnet**: Deploy local Linera network
3. **Prototype Player Chain**: Start with character creation
4. **Test Cross-Chain Messaging**: Implement simple message passing
5. **Build Battle Chain**: Port combat logic
6. **Integrate Prediction Markets**: Add betting system
7. **Launch Testnet**: Public beta on Linera testnet
8. **Mainnet**: Launch on Linera mainnet

---

## 📚 Resources

- **Linera Documentation**: https://linera.dev/
- **Linera SDK (Rust)**: https://github.com/linera-io/linera-protocol
- **Example Applications**: https://linera.dev/developers/getting_started/hello_linera.html
- **BattleChain Original Spec**: `/home/user/gameOn/docs/BATTLECHAIN_GAME.md`
- **This Architecture**: `/home/user/diccy/BATTLECHAIN_LINERA_ARCHITECTURE.md`

---

**Created by**: Claude (Anthropic)
**Date**: 2025-11-12
**Version**: 1.0
**For**: BattleChain → Linera Migration
