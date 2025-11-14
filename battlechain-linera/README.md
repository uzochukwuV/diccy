# 🎮 BattleChain - Blockchain Fighting Game on Linera

A fully on-chain fighting game built with Linera Protocol's microchains architecture, featuring turn-based combat, NFT characters, spectator betting, and global leaderboards.

## Features

### 🥊 Core Gameplay
- **Turn-based Combat** - Strategic fighting with stances, combos, and special moves
- **5 Character Classes** - Warrior, Assassin, Mage, Tank, Trickster (each with unique abilities)
- **Permadeath System** - Characters have 3 lives, adding risk to battles
- **Randomness Generation** - Timestamp-based deterministic randomness for fairness
- **Stake-based Battles** - Winner takes the pot (minus platform fee)

### 🎯 Character System
- **NFT Characters** - Unique blockchain-owned fighters
- **Character Progression** - Level up through victories
- **Combat Statistics** - Track damage, crits, dodges, and more
- **Inventory Management** - Single-owner player chains for fast transactions

### 🏆 Competitive Features
- **ELO Rating System** - Standard chess-formula ELO (K-factor 32)
- **Global Leaderboards** - Tracked on Registry Chain
- **Win Streaks** - Current and best streaks recorded
- **Battle History** - Full battle logs with metadata

### 💰 Economic System
- **BATTLE Token** - Native token for staking and rewards
- **Prediction Markets** - Spectators bet on battle outcomes
- **Dynamic Odds** - Pool-based odds calculation
- **Platform Fees** - Configurable fee (default 3%)

### 🎲 Matchmaking
- **Queue System** - Join with character and stake
- **Battle Offers** - Manual matchmaking with confirmation
- **Stake Requirements** - Configurable minimum stake
- **Cross-chain Messaging** - Seamless coordination

## Architecture

BattleChain uses **microchains** - individual blockchains per player for maximum performance:

```
┌─────────────────┐
│  Player Chains  │  (Single-owner, fast transactions)
│  - NFT ownership│
│  - Inventory    │
└────────┬────────┘
         │
    ┌────▼─────────────────────────────┐
    │    Matchmaking Chain (Public)    │
    │    - Queue management            │
    │    - Battle creation             │
    └──────┬───────────────────────────┘
           │
      ┌────▼──────────────┐
      │   Battle Chains   │ (Multi-owner, 2 players)
      │   - Combat engine │
      │   - Turn-based    │
      └────┬─────┬────────┘
           │     │
    ┌──────▼─┐ ┌▼──────────────────┐
    │Registry│ │ Prediction Market │ (Public)
    │ Chain  │ │ - Spectator bets  │
    │- Stats │ │ - Dynamic odds    │
    │- ELO   │ │ - Payouts         │
    └────────┘ └───────────────────┘
```

### Microchains

1. **Player Chain** - Personal NFT ownership and inventory
2. **Battle Chain** - Turn-based combat between two players
3. **Matchmaking Chain** - Queue and battle coordination
4. **Prediction Market** - Spectator betting system
5. **Registry Chain** - Global leaderboards and statistics
6. **Battle Token** - Fungible token for economy

## Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add WASM target
rustup target add wasm32-unknown-unknown

# Install Linera CLI
cargo install linera-client --version 0.15.5
```

### Build

```bash
cd battlechain-linera
./scripts/build-all.sh
```

### Deploy Locally

```bash
# Start local Linera network
linera net up

# Deploy all chains (see DEPLOYMENT.md for details)
```

## Combat Mechanics

### Character Classes

| Class     | HP  | Attack | Defense | Speed | Ability                          |
|-----------|-----|--------|---------|-------|----------------------------------|
| Warrior   | 120 | 25     | 20      | 15    | Shield Bash (stun)               |
| Assassin  | 80  | 30     | 10      | 25    | Shadow Strike (high crit)        |
| Mage      | 90  | 35     | 8       | 18    | Fireball (AoE damage)            |
| Tank      | 150 | 18     | 30      | 10    | Taunt (force opponent to attack) |
| Trickster | 100 | 22     | 15      | 22    | Smoke Bomb (dodge buff)          |

### Stances

- **Aggressive** - +20% damage, -10% defense
- **Defensive** - +20% defense, -10% damage
- **Balanced** - No modifiers
- **Reckless** - +40% damage, -20% defense, +10% crit

### Combat Flow

1. Players choose stance (Aggressive, Defensive, Balanced, Reckless)
2. Fastest character attacks first
3. Damage calculated with stance modifiers
4. Check for critical hits (2x damage)
5. Defender can dodge (nullify damage)
6. Defender can counter-attack (bonus damage)
7. Repeat until one character reaches 0 HP
8. Winner gets stake + opponent's stake - platform fee

### Fixed-Point Math

All calculations use `FP_SCALE = 1,000,000` for precise decimal math without floating point.

## GraphQL API

### Registry Chain - Leaderboards

```graphql
query {
  # Global statistics
  stats {
    totalCharacters
    totalBattles
    totalVolume
  }

  # Top players by ELO
  topCharacters(limit: 20)
}
```

### Matchmaking Chain

```graphql
query {
  stats {
    waitingPlayers
    activeBattles
    pendingBattles
    totalBattles
  }
}
```

### Prediction Market

```graphql
query {
  stats {
    totalMarkets
    totalBets
    totalVolume
    platformFeeBps
  }
}
```

## Project Structure

```
battlechain-linera/
├── battle-chain/          # Combat engine
├── battle-token/          # BATTLE token implementation
├── player-chain/          # NFT character ownership
├── matchmaking-chain/     # Queue and battle coordination
├── prediction-chain/      # Spectator betting markets
├── registry-chain/        # Global stats and leaderboards
├── shared-types/          # Common types and utilities
├── scripts/               # Build and deployment scripts
├── DEPLOYMENT.md          # Detailed deployment guide
└── README.md             # This file
```

## Development

### Running Tests

```bash
# Test all chains
cargo test --workspace

# Test specific chain
cd battle-chain
cargo test
```

### Code Coverage

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --out Html
```

### Formatting

```bash
cargo fmt --all
cargo clippy --all-targets --all-features
```

## Deployment

See [DEPLOYMENT.md](./DEPLOYMENT.md) for comprehensive deployment instructions.

### Quick Deploy

```bash
# 1. Build all chains
./scripts/build-all.sh

# 2. Start local network
linera net up

# 3. Deploy each chain (see DEPLOYMENT.md for details)
```

## Roadmap

### Phase 1: Core Game ✅
- [x] Character system with 5 classes
- [x] Turn-based combat engine
- [x] Randomness generation
- [x] Player chain for NFT ownership

### Phase 2: Matchmaking & Economy ✅
- [x] Matchmaking queue system
- [x] Battle offer/confirmation flow
- [x] BATTLE token economics
- [x] Stake-based battles

### Phase 3: Social & Competition ✅
- [x] Global leaderboards
- [x] ELO rating system
- [x] Battle history tracking
- [x] Prediction markets

### Phase 4: Polish & Scale 🚧
- [ ] Multi-owner chain creation (needs SDK research)
- [ ] Frontend web interface
- [ ] Mobile-responsive design
- [ ] Advanced matchmaking (skill-based)

### Phase 5: Expansion 📋
- [ ] Tournament system
- [ ] Guilds/clans
- [ ] Seasonal rankings
- [ ] Character customization
- [ ] New character classes
- [ ] PvE game modes

## Technical Highlights

### Linera-Specific Features

- **Microchains Architecture** - Each player has their own blockchain
- **Cross-chain Messaging** - Seamless communication between chains
- **View System** - Efficient state management with MapView and RegisterView
- **WASM Execution** - Smart contracts compiled to WebAssembly
- **GraphQL Services** - Built-in GraphQL for queries

### Smart Contract Security

- ✅ No unsafe Rust code
- ✅ Input validation on all operations
- ✅ Saturating arithmetic (no overflow)
- ✅ Authentication checks
- ✅ Gas-efficient design

### Performance Optimizations

- Single-owner chains for player actions (zero contention)
- Multi-owner chains only for shared state (battles)
- Efficient state storage with views
- Minimal cross-chain messages

## Community

- **GitHub**: https://github.com/uzochukwuV/diccy
- **Discord**: [Coming Soon]
- **Twitter**: [Coming Soon]

## Contributing

Contributions welcome! Please read our contributing guidelines and code of conduct.

### Areas for Contribution

- Smart contract improvements
- Frontend development
- Game balance tuning
- Documentation
- Testing
- Art and design

## License

[License TBD - Add appropriate license]

## Acknowledgments

- Built with [Linera Protocol](https://linera.dev)
- Inspired by classic fighting games and blockchain gaming innovation
- Special thanks to the Linera team for their excellent documentation

## Support

For questions and support:
- GitHub Issues for bugs
- Discord for community discussion
- Documentation at `/docs`

---

**BattleChain** - Where blockchain meets fighting game excellence 🥊⛓️
