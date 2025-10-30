# Fighter Game - Real-Time Web3 Battle Arena

A production-ready, on-chain fighting game built on Linera blockchain featuring real-time combat, NFT progression, staking mechanics, prediction markets, and tournaments.

## 🎮 Features

### Core Gameplay
- **Real-time Combat**: Players execute strikes in turn-based battles with randomized damage
- **Combo System**: Landing attacks in the same damage range awards 1.5x-2x bonus multipliers
- **Critical Hits**: Probability-based critical strikes with configurable rates
- **Defense Mechanics**: Each fighter's defense stat reduces incoming damage
- **Timeout System**: Players can claim victory if opponent doesn't move within time limit

### Progression & NFTs
- **Dynamic NFT Evolution**: Fighters evolve through 6 tiers (Bronze → Legendary)
- **XP & Leveling**: Earn XP from battles to level up and improve stats
- **Stat Growth**: HP, Attack, Defense, and Critical Chance all scale with level
- **Special Abilities**: Unlock unique abilities at levels 10, 25, and 50
- **Visual Traits**: NFT metadata updates with fighter progression

### Economic Systems
- **Free Play Mode**: Practice battles with no entry cost, earn XP only
- **Staked Battles**: Players bet tokens, winner takes 90% (10% platform fee)
- **Prediction Markets**: Spectators bet on battle outcomes with odds-based payouts
- **Tournament Entry Fees**: Organizers set fees, prize pools distributed to top performers

### Matchmaking & Balance
- **Tier-Based Matching**: 5 tiers ensure fair competition (Novice → Master)
- **Grade Restrictions**: Players can only match within ±1 tier difference
- **Ranked System**: Leaderboard tracks top fighters by XP

### Social & Competitive
- **Tournaments**: Create scheduled tournaments with custom rules and prize pools
- **Battle Log**: Complete history of every action in a fight
- **Leaderboards**: Global rankings by XP, wins, and streaks
- **Statistics Tracking**: Comprehensive fighter stats and analytics

## 🏗️ Architecture

### Smart Contract Structure

```
fighter-game/
├── src/
│   ├── lib.rs          # Core game logic, data structures, and ABI
│   ├── state.rs        # Application state management
│   ├── contract.rs     # Contract execution logic
│   └── service.rs      # GraphQL service for queries
├── tests/
│   └── fighter_game.rs # Integration tests
├── Cargo.toml          # Dependencies
└── README.md           # This file
```

### Key Components

**lib.rs** - Core Logic
- `Fighter`: Player character with stats, progression, and NFT data
- `Battle`: Active combat state with HP tracking, turn management, combo system
- `ComboTracker`: Detects consecutive hits in same damage range
- `PredictionPool`: Manages spectator bets and winnings distribution
- `Tournament`: Multi-round bracket system
- Damage calculation formulas with randomness

**contract.rs** - On-Chain Execution
- Fighter registration and validation
- Battle initialization with tier matching
- Strike execution with randomness generation
- Timeout claims for inactive opponents
- Tournament creation and registration
- Prediction placement and claiming
- XP distribution and leveling
- Stake and prize distribution

**service.rs** - GraphQL Queries
- Fighter queries (individual and paginated lists)
- Battle queries (active, historical, by fighter)
- Tournament listings
- Leaderboard with rankings
- Fighter statistics and analytics
- Global platform statistics

**state.rs** - Persistent Storage
- Fighters indexed by owner
- Active battles by ID
- Tournaments by ID
- Leaderboard rankings
- Matchmaking queues by tier
- Platform earnings tracking

## 📊 Game Mechanics

### Damage Calculation

```rust
// Base damage range scales with level
min_damage = base_attack + (level * 2)
max_damage = base_attack + (level * 5)

// Random value within range
strike_damage = random(min_damage, max_damage)

// Critical hit check (% based)
if random(0, 100) < critical_chance {
    strike_damage *= critical_multiplier  // Default 1.5x
}

// Combo bonus
if (damage_range == last_damage_range) {
    combo_count++
    if combo_count == 2: damage *= 1.5
    if combo_count >= 3: damage *= 2.0
}

// Apply defender's defense
final_damage = strike_damage * (1 - defense / 100)
```

### XP & Leveling

```rust
// XP required for next level
xp_required = 100 * (current_level ^ 2)

// XP rewards
win_xp = 100 * (staked_battle ? 1.5 : 1.0)
loss_xp = 20 * (staked_battle ? 1.5 : 1.0)

// Level up bonuses
+10 HP
+3 Attack
+1 Defense
+2% Crit Chance (every 5 levels, max 30%)
```

### Fighter Tiers

| Tier | Level Range | Abilities |
|------|-------------|-----------|
| Bronze | 1-10 | Basic stats |
| Silver | 11-25 | +Power Strike ability |
| Gold | 26-50 | +Defensive Stance |
| Platinum | 51-75 | Enhanced stats |
| Diamond | 76-99 | Elite performance |
| Legendary | 100+ | +Berserker Mode, Max stats |

### Matchmaking Tiers

| Tier | Level Range | Can Match With |
|------|-------------|----------------|
| Novice | 1-10 | Novice, Intermediate |
| Intermediate | 11-25 | Novice, Intermediate, Advanced |
| Advanced | 26-50 | Intermediate, Advanced, Expert |
| Expert | 51-100 | Advanced, Expert, Master |
| Master | 100+ | Expert, Master |

## 🚀 Deployment

### Prerequisites

```bash
# Install Linera CLI
cargo install linera-sdk

# Verify installation
linera --version
```

### Local Development Setup

```bash
# Clone the repository
git clone <your-repo>
cd fighter-game

# Set up Linera devnet
export PATH="$PWD/target/debug:$PATH"
source /dev/stdin <<<"$(linera net helper 2>/dev/null)"

# Start local network with faucet
FAUCET_PORT=8079
FAUCET_URL=http://localhost:$FAUCET_PORT
linera_spawn linera net up --with-faucet --faucet-port $FAUCET_PORT
```

### Create Player Wallets

```bash
# Setup wallet directories
export LINERA_WALLET_1="$LINERA_TMP_DIR/wallet_1.json"
export LINERA_KEYSTORE_1="$LINERA_TMP_DIR/keystore_1.json"
export LINERA_STORAGE_1="rocksdb:$LINERA_TMP_DIR/client_1.db"

export LINERA_WALLET_2="$LINERA_TMP_DIR/wallet_2.json"
export LINERA_KEYSTORE_2="$LINERA_TMP_DIR/keystore_2.json"
export LINERA_STORAGE_2="rocksdb:$LINERA_TMP_DIR/client_2.db"

# Initialize wallets
linera --with-wallet 1 wallet init --faucet $FAUCET_URL
linera --with-wallet 2 wallet init --faucet $FAUCET_URL

# Request chains
INFO_1=($(linera --with-wallet 1 wallet request-chain --faucet $FAUCET_URL))
INFO_2=($(linera --with-wallet 2 wallet request-chain --faucet $FAUCET_URL))

CHAIN_1="${INFO_1[0]}"
CHAIN_2="${INFO_2[0]}"
OWNER_1="${INFO_1[1]}"
OWNER_2="${INFO_2[1]}"
```

### Deploy Application

```bash
# Build and publish the application
APP_ID=$(linera -w1 --wait-for-outgoing-messages \
  project publish-and-create . fighter_game $CHAIN_1 \
    --json-argument '{
        "turnTimeout": 30000000,
        "blockDelay": 5000000,
        "platformFee": 10
    }')

echo "Application deployed: $APP_ID"

# Start node services
linera -w1 service --port 8080 &
linera -w2 service --port 8081 &
sleep 2
```

## 🎯 Usage Examples

### Register a Fighter

```graphql
mutation {
  registerFighter(name: "DragonSlayer")
}
```

### Start a Free Battle

```graphql
mutation {
  startFreeBattle(
    opponent: "User:036c33a49a7307ff61b5e2e65b4f088c1cba05cf8b00cb4541c40e85e5cc49ce"
  )
}
```

### Start a Staked Battle

```graphql
mutation {
  startStakedBattle(
    opponent: "User:036c33a49a7307ff61b5e2e65b4f088c1cba05cf8b00cb4541c40e85e5cc49ce"
    stakeAmount: "1000000"
  )
}
```

### Execute a Strike

```graphql
mutation {
  strike(battleId: 1)
}
```

### Query Your Fighter

```graphql
query {
  fighter(owner: "User:YOUR_ADDRESS") {
    name
    level
    xp
    totalWins
    totalLosses
    maxHp
    baseAttack
    defense
    criticalChance
    nftTier
    specialAbilities
  }
}
```

### Query Active Battle

```graphql
query {
  battle(battleId: 1) {
    fighter1
    fighter2
    fighter1Hp
    fighter2Hp
    currentTurn
    turnNumber
    status
    comboTracker {
      comboCount
      lastDamageRange
    }
    battleLog {
      turn
      attacker
      actionType
      damage
    }
  }
}
```

### View Leaderboard

```graphql
query {
  leaderboard(limit: 10) {
    rank
    name
    level
    xp
    totalWins
    winRate
    currentStreak
    tier
  }
}
```

### Create Tournament

```graphql
mutation {
  createTournament(
    name: "Weekly Championship"
    entryFee: "5000000"
    startTime: 1730332800000000
    maxParticipants: 16
    prizePoolDistribution: [50, 30, 20]
  )
}
```

### Place Prediction

```graphql
mutation {
  placePrediction(
    battleId: 1
    predictedWinner: "User:036c33a49a7307ff61b5e2e65b4f088c1cba05cf8b00cb4541c40e85e5cc49ce"
    betAmount: "100000"
  )
}
```

### Get Fighter Statistics

```graphql
query {
  fighterStats(owner: "User:YOUR_ADDRESS") {
    name
    level
    xp
    xpToNextLevel
    totalWins
    totalLosses
    winRate
    totalBattles
    totalDamageDeal
    totalDamageTaken
    currentStreak
    highestStreak
    matchmakingTier
  }
}
```

### Global Statistics

```graphql
query {
  globalStats {
    totalFighters
    totalBattles
    activeBattles
    totalXpDistributed
    platformBalance
  }
}
```

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_fighter_registration

# Run with logging
RUST_LOG=debug cargo test test_free_battle_complete_flow -- --nocapture

# Integration tests
cargo test --test fighter_game
```

## 🔒 Security Considerations

### Implemented Safeguards

1. **Authentication**: All operations require authenticated owner
2. **Validation**: Input validation on all parameters (names, amounts, etc.)
3. **Tier Restrictions**: Prevents unfair matchmaking
4. **Timeout Protection**: Prevents indefinite battle locks
5. **Double-spend Prevention**: Stake amounts validated before battle creation
6. **Claim Guards**: Predictions can only be claimed once
7. **Battle State Checks**: Operations validated against current battle status

### Randomness

Current implementation uses pseudo-random generation for simplicity:
```rust
seed = battle_id ^ timestamp ^ chain_id_hash
```

**Production Recommendation**: Integrate proper VRF (Verifiable Random Function):
- Linera's built-in randomness oracle (when available)
- External oracle service (Chainlink VRF equivalent)
- Commit-reveal schemes for critical randomness

### Economic Security

- **Platform Fee**: 10% on staked battles and predictions prevents exploitation
- **Stake Escrow**: Funds locked until battle completion
- **Prediction Locking**: Pools lock when battle starts
- **Gas Costs**: Multiple strikes per battle - optimized for Linera's low fees

## 📈 Future Enhancements

### Gameplay
- [ ] Special abilities system (active skills per tier)
- [ ] Equipment and items (weapons, armor, consumables)
- [ ] Team battles (2v2, 3v3)
- [ ] Battle replays and spectator mode
- [ ] Seasonal rankings and rewards

### Technical
- [ ] Cross-chain interoperability (battle across chains)
- [ ] NFT marketplace integration
- [ ] Oracle integration for verified randomness
- [ ] Gas optimization for multi-strike transactions
- [ ] State compression for large tournaments

### Social
- [ ] Guilds and clans
- [ ] Training mode against AI
- [ ] Battle chat and emotes
- [ ] Achievement system
- [ ] Referral rewards

### Economic
- [ ] Liquidity mining for prediction pools
- [ ] Governance token for platform decisions
- [ ] Sponsored tournaments with external prizes
- [ ] NFT trait marketplace
- [ ] Staking for passive rewards

## 🤝 Contributing

Contributions welcome! Please:
1. Fork the repository
2. Create a feature branch
3. Add tests for new functionality
4. Ensure all tests pass
5. Submit a pull request

## 📄 License

Apache-2.0

## 🔗 Resources

- [Linera Documentation](https://docs.linera.io)
- [Linera SDK](https://github.com/linera-io/linera-protocol)
- [GraphQL API Reference](https://graphql.org/learn/)

## 💡 Design Rationale

### Why Linera?

1. **Low Gas Costs**: Multiple strikes per battle require minimal fees
2. **Fast Finality**: Real-time gameplay without long confirmation times
3. **Microchains**: Isolated battle state for efficient computation
4. **Native Randomness**: Built-in support for fair RNG (when available)
5. **Developer Experience**: Rust-based with excellent tooling

### Architecture Decisions

**Monolithic State vs Microchains**
- Main chain: Fighter registry, tournaments, leaderboards
- Battle-specific chains: Could be implemented for isolated combat
- Current: Single-chain for simplicity, expandable to microchains

**On-Chain vs Off-Chain**
- Combat logic: Fully on-chain for trustlessness
- NFT metadata: On-chain for composability
- Analytics: Can be indexed off-chain for performance
- Randomness: Should move to oracle in production

**Gas Optimization**
- Batch strike operations where possible
- Lazy leaderboard updates (only on query)
- Minimal storage in battle logs
- Efficient data structures (Vec, HashMap)

## 📞 Support

For questions, issues, or suggestions:
- Open a GitHub issue
- Join our Discord: [Link]
- Email: support@fightergame.io

---

**Built with ⚔️ on Linera**
