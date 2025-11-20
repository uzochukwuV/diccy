# 🎮 BattleChain - On-Chain Fighting Game

A fully on-chain turn-based fighting game built with Linera Protocol's microchains architecture.

## 🚀 Quick Start

All smart contract code and documentation is in the `battlechain-linera/` directory:

```bash
cd battlechain-linera
```

See [battlechain-linera/README.md](./battlechain-linera/README.md) for complete documentation.

## 📦 Repository Structure

```
diccy/
├── battlechain-linera/     # Smart contracts (Rust/WASM)
│   ├── battle-chain/       # Combat engine
│   ├── player-chain/       # NFT character ownership
│   ├── matchmaking-chain/  # Queue and battle coordination
│   ├── prediction-chain/   # Spectator betting markets
│   ├── registry-chain/     # Global leaderboards and ELO
│   ├── battle-token/       # BATTLE token economics
│   └── shared-*/           # Common types and utilities
│
└── web-frontend/           # Frontend (React/Next.js)
    └── [Coming soon]
```

## ⚡ Features

- **Turn-based Combat** - Strategic fighting with stances, combos, and special moves
- **5 Character Classes** - Warrior, Assassin, Mage, Tank, Trickster
- **Prediction Markets** - Spectators bet on battle outcomes
- **ELO Rating System** - Competitive rankings and leaderboards
- **Stake-based Battles** - Winner takes the pot (minus platform fee)
- **Cross-chain Architecture** - Each player has their own blockchain for maximum performance

## 🏗️ Build Smart Contracts

```bash
cd battlechain-linera
cargo build --all --release --target wasm32-unknown-unknown
```

## 📊 Implementation Status

### ✅ Completed
- ✅ Core combat engine with 5 character classes
- ✅ Turn-based battle mechanics with stances and special moves
- ✅ NFT character system with progression
- ✅ Matchmaking queue with automatic matching
- ✅ BATTLE token economics
- ✅ Prediction markets for spectators
- ✅ Global registry with ELO ratings
- ✅ Cross-chain messaging infrastructure
- ✅ Battle→Prediction integration (automatic notifications)
- ✅ Automatic market creation
- ✅ Token transfers in ClaimWinnings
- ✅ Registry battle result handling

### 🚧 In Progress
- 🚧 Frontend web interface
- 🚧 Comprehensive testing suite

### 📋 Planned
- 📋 Tournament system
- 📋 Advanced matchmaking algorithms
- 📋 Guilds/clans
- 📋 Character customization

## 📚 Documentation

- [Complete README](./battlechain-linera/README.md) - Full smart contract documentation
- [Deployment Guide](./battlechain-linera/DEPLOYMENT.md) - How to deploy to Linera
- [Game Flow Analysis](./battlechain-linera/GAME_FLOW_ANALYSIS.md) - Complete game mechanics
- [Testing Status](./battlechain-linera/TESTING_STATUS.md) - Test coverage and status

## 🛠️ Technology Stack

### Smart Contracts
- **Linera Protocol v0.15.5** - Microchains blockchain platform
- **Rust** - Smart contract development
- **WASM** - WebAssembly compilation target
- **GraphQL** - Query interface for services

### Frontend (Coming Soon)
- **React** - UI framework
- **Next.js** - Web framework
- **TypeScript** - Type safety
- **TailwindCSS** - Styling

## 🏆 Architecture Highlights

BattleChain uses Linera's **microchains** architecture:

- **Player Chains** (single-owner) - Fast NFT and inventory operations
- **Battle Chains** (multi-owner) - Shared combat state between 2 players
- **Public Chains** - Matchmaking, prediction markets, registry
- **Token Chain** - BATTLE fungible token

This design eliminates gas wars and provides instant finality for player actions.

## 🔒 Security

- ✅ No unsafe Rust code
- ✅ Input validation on all operations
- ✅ Saturating arithmetic (no overflow)
- ✅ Authentication checks on cross-chain messages
- ✅ Rate limiting on operations

## 📈 Recent Progress

**Latest Updates (November 2025):**

```
Commit 00ff880 - Implement token transfers in prediction market ClaimWinnings
Commit 8cb5797 - Battle→Prediction integration and automatic market creation
Commit 804542a - Add comprehensive game flow analysis
Commit 07c3c55 - Fix balance/allowance queries and auto-matchmaking
```

**Build Status:** ✅ All chains compile to WASM successfully

```
battle_chain.wasm:      213KB
matchmaking_chain.wasm: 256KB
prediction_chain.wasm:  290KB
player_chain.wasm:      250KB
registry_chain.wasm:    247KB
battle_token.wasm:      266KB
```

## 🤝 Contributing

Contributions welcome! Areas for contribution:

- Smart contract improvements
- Frontend development
- Game balance tuning
- Documentation
- Testing

See [battlechain-linera/README.md](./battlechain-linera/README.md) for detailed contribution guidelines.

## 📄 License

[License TBD]

## 🙏 Acknowledgments

Built with [Linera Protocol](https://linera.dev) - The first blockchain infrastructure designed for highly scalable, low-latency applications.

---

**BattleChain** - Where blockchain meets fighting game excellence 🥊⛓️
