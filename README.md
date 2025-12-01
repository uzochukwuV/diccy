# Majorules - Blockchain Gaming Platform

A turn-based battle game with prediction markets built on Linera blockchain, featuring cross-chain architecture and ELO rating system.

## Architecture Overview

### Unified Contract Design
- **Single Contract**: One unified contract handles all chain variants (Lobby, Player, Battle)
- **Chain Variants**: Each chain specializes in specific functionality while sharing the same codebase
- **Cross-Chain Messaging**: Secure communication between chains with sender verification

### Chain Types

#### Lobby Chain
- **Matchmaking**: Level-based matching (within 10 levels)
- **Battle Creation**: Creates multi-owner battle chains
- **Player Registration**: Manages player chain registry
- **Prediction Markets**: Unified betting system with liquidity aggregation
- **ELO Management**: Validates and forwards ELO updates to player chains

#### Player Chain
- **Character Management**: Create/upgrade characters with stats
- **Battle Records**: Track wins/losses and battle history
- **ELO Tracking**: Maintains player rating with lobby authorization
- **Token Management**: Handle battle tokens and rewards

#### Battle Chain
- **Turn-Based Combat**: 3-round battles with simultaneous turn submission
- **Multi-Owner**: Both players have ownership for fair gameplay
- **ELO Calculation**: Uses standard ELO formula (K-factor 32)
- **Result Reporting**: Sends outcomes to lobby for processing

## Key Features

### Game Mechanics
- **Character Stats**: Health, Attack, Defense, Speed with level scaling
- **Turn System**: Players submit moves simultaneously, auto-execution after both submit
- **Battle Flow**: 3 rounds maximum, winner determined by remaining health
- **Fair Matchmaking**: Level-based pairing prevents stat imbalances

### Economic System
- **Entry Fees**: Configurable battle entry costs
- **Platform Fees**: Revenue sharing (5% lobby, 3% battle)
- **Prediction Markets**: Bet on battle outcomes with dynamic odds
- **Token Distribution**: Winner takes pot minus platform fees

### Technical Innovations
- **Chain Variant Detection**: Dynamic chain type detection from stored state
- **Modular Architecture**: Separate contract modules for each chain type
- **Cross-Chain Security**: Message authentication prevents unauthorized requests
- **Unified Prediction Markets**: Lobby handles all betting for better liquidity

## Project Structure

```
src/
├── contract.rs          # Main contract with variant routing
├── lib.rs              # Public API and message definitions
├── state.rs            # All state structures and types
├── lobby_contract.rs   # Lobby chain implementation
├── player_contract.rs  # Player chain implementation
├── battle_contract.rs  # Battle chain implementation
├── random.rs           # Deterministic randomness utilities
└── service.rs          # GraphQL service layer
```

## Key Improvements from Original

### Architecture Restructure
- **From**: 6 separate applications (lobby, player, battle, prediction, tournament, leaderboard)
- **To**: 1 unified contract with 3 specialized chain variants
- **Benefits**: Reduced complexity, better maintainability, unified codebase

### Enhanced Game Balance
- **Level-Based Matching**: Prevents unfair stat advantages
- **ELO Rating System**: Skill-based ranking with proper K-factor
- **Turn Mechanics**: Simultaneous submission prevents timing advantages

### Unified Prediction Markets
- **From**: Separate prediction chains
- **To**: Lobby-integrated betting system
- **Benefits**: Better liquidity, simplified token flow, reduced overhead

### Cross-Chain Security
- **Sender Verification**: All messages verify chain ID authenticity
- **Player Chain Authority**: Players call from their chains for automatic data provision
- **Multi-Owner Battles**: Both players control battle chain for fairness

## Deployment

### Local Development
```bash
# Build and run locally
./run.bash
```

### Docker Deployment
```bash
# Using Docker Compose
docker compose up --force-recreate
```

### Access Points
- **Frontend**: http://localhost:5173
- **Faucet**: http://localhost:8080
- **Validator Proxy**: http://localhost:9001

## Game Flow

1. **Player Registration**: Create player chain and characters
2. **Matchmaking**: Lobby finds level-appropriate opponents
3. **Battle Creation**: Multi-owner battle chain instantiated
4. **Prediction Phase**: Users bet on battle outcomes
5. **Combat**: Turn-based battles with simultaneous moves
6. **Resolution**: ELO updates, token distribution, market settlement

## Technical Specifications

- **Blockchain**: Linera
- **Language**: Rust
- **Architecture**: Multi-chain with cross-chain messaging
- **Consensus**: Linera's microchain consensus
- **Storage**: Persistent state with view-based architecture