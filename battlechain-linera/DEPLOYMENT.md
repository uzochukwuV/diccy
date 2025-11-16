# BattleChain Deployment Guide

## Overview

BattleChain is a blockchain-based fighting game built on the Linera Protocol using microchains architecture. This guide covers building, deploying, and running the complete BattleChain ecosystem.

## Architecture

BattleChain consists of 7 microchains:

### Core Game Chains
1. **Player Chain** - Character NFT ownership, inventory management, personal game state
2. **Battle Chain** - Turn-based combat engine with randomness generation
3. **Matchmaking Chain** - Queue management, battle offer/confirmation workflow
4. **Battle Token** - BATTLE token for staking and earnings

### Meta-Game Chains
5. **Prediction Market** - Spectator betting on battle outcomes
6. **Registry Chain** - Global leaderboards, ELO ratings, statistics

### Shared Infrastructure
7. **Shared Types** - Common types, character classes, combat mechanics

## Prerequisites

### Required Software
- **Rust** 1.75+ with wasm32-unknown-unknown target
  ```bash
  rustup target add wasm32-unknown-unknown
  ```

- **Linera CLI** v0.15.5
  ```bash
  cargo install linera-client --version 0.15.5
  ```

- **Docker** (optional, for running local validator)

### System Requirements
- 4GB RAM minimum
- 10GB disk space for builds
- Linux, macOS, or WSL2 on Windows

## Building

### Build All Chains

```bash
cd battlechain-linera
./scripts/build-all.sh
```

This will compile all chains to WASM and show artifact locations.

### Build Individual Chain

```bash
cd <chain-name>
cargo build --target wasm32-unknown-unknown --release
```

### Verify Build

```bash
# Check WASM artifacts
find . -name "*.wasm" -path "*/target/wasm32-unknown-unknown/release/*" ! -path "*/deps/*"

# Check sizes
du -h */target/wasm32-unknown-unknown/release/*.wasm
```

## Deployment

### 1. Start Local Linera Network

```bash
# Start local validator
linera net up

# Create default wallet
linera wallet init --with-new-chain

# Show wallet status
linera wallet show
```

### 2. Deploy Shared Types (Dependency)

```bash
cd shared-types
linera publish-and-create \
  target/wasm32-unknown-unknown/release/battlechain_shared_types.wasm

# Note the APPLICATION_ID for use in other chains
```

### 3. Deploy Battle Token

```bash
cd battle-token

# Publish and create Battle Token application
# InstantiationArgument: (token_name: String, initial_supply: Amount)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/battle_token.wasm \
  --json-argument '{"name":"BattleToken","symbol":"BATTLE","initial_supply":"1000000000000"}'

# Note BATTLE_TOKEN_APP_ID
```

### 4. Deploy Player Chain

```bash
cd player-chain

# Publish and create Player Chain
# InstantiationArgument: Amount (minimum character mint fee)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/player_chain.wasm \
  --json-argument '100000000' # 0.1 BATTLE minimum fee
```

### 5. Deploy Battle Chain

```bash
cd battle-chain

# Publish Battle Chain application
# InstantiationArgument: ()
linera publish-and-create \
  target/wasm32-unknown-unknown/release/battle_chain.wasm \
  --json-argument 'null'

# Note BATTLE_APP_ID
```

### 6. Deploy Matchmaking Chain

```bash
cd matchmaking-chain

# Publish Matchmaking Chain
# Parameters: Amount (minimum stake)
# InstantiationArgument: ()
linera publish-and-create \
  target/wasm32-unknown-unknown/release/matchmaking_chain.wasm \
  --json-parameters '1000000000' \
  --json-argument 'null'

# Note MATCHMAKING_APP_ID
```

### 7. Deploy Prediction Market Chain

```bash
cd prediction-chain

# Publish Prediction Market
# Parameters: u16 (platform fee in basis points, 100 = 1%)
# InstantiationArgument: Owner (treasury owner)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/prediction_chain.wasm \
  --json-parameters '300' \
  --json-argument '{"chain_id":"...","owner":{"Account":"0x..."}}'

# Note PREDICTION_APP_ID
```

### 8. Deploy Registry Chain

```bash
cd registry-chain

# Publish Registry Chain
# Parameters: ()
# InstantiationArgument: ()
linera publish-and-create \
  target/wasm32-unknown-unknown/release/registry_chain.wasm \
  --json-argument 'null'

# Note REGISTRY_APP_ID
```

### 9. Configure Chain References

After deploying all chains, update the application references:

```bash
# Update Matchmaking Chain with Battle and Token app IDs
linera execute-operation \
  --application-id $MATCHMAKING_APP_ID \
  --json-operation '{
    "UpdateReferences": {
      "battle_app_id": "'$BATTLE_APP_ID'",
      "battle_token_app": "'$BATTLE_TOKEN_APP_ID'",
      "treasury_owner": {"chain_id":"...","owner":{"Account":"0x..."}}
    }
  }'
```

## Usage

### Create a Character NFT

```bash
# Mint a character on Player Chain
linera execute-operation \
  --application-id $PLAYER_APP_ID \
  --json-operation '{
    "MintCharacter": {
      "class": "Warrior",
      "level": 1,
      "fee": "100000000"
    }
  }'
```

### Join Matchmaking Queue

```bash
# Join queue with character
linera execute-operation \
  --application-id $MATCHMAKING_APP_ID \
  --json-operation '{
    "JoinQueue": {
      "player_chain": "'$YOUR_CHAIN_ID'",
      "player_owner": {"Account":"0x..."},
      "character": {...},
      "stake": "1000000000"
    }
  }'
```

### Query Registry Leaderboard

```bash
# Query top 10 characters by ELO
linera query-application \
  --application-id $REGISTRY_APP_ID \
  --json-query '{
    "topCharacters": {"limit": 10}
  }'
```

## GraphQL Queries

### Registry Chain

```graphql
# Get global stats
query {
  stats {
    totalCharacters
    totalBattles
    totalVolume
  }
}

# Get leaderboard
query {
  topCharacters(limit: 20)
}
```

### Matchmaking Chain

```graphql
# Get matchmaking stats
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
# Get market stats
query {
  stats {
    totalMarkets
    totalBets
    totalVolume
    platformFeeBps
  }
}
```

## Monitoring

### View Chain State

```bash
# List all chains
linera wallet show

# Query application state
linera query-application --application-id $APP_ID
```

### Check Logs

```bash
# Validator logs
linera net helper logs

# Service logs
linera service --port 8080
```

## Troubleshooting

### Build Errors

**Issue**: `linker 'rust-lld' not found`
```bash
rustup component add rust-lld
```

**Issue**: WASM target not found
```bash
rustup target add wasm32-unknown-unknown
```

### Deployment Errors

**Issue**: Insufficient balance
```bash
# Request tokens from faucet (testnet) or check wallet balance
linera wallet show
```

**Issue**: Application not found
```bash
# Verify application was published correctly
linera wallet show
```

### Runtime Errors

**Issue**: Cross-chain message not received
- Check that source chain has sent the message
- Verify destination chain is subscribed
- Wait for message propagation (can take a few seconds)

## Testing

### Run Unit Tests

```bash
# Test individual chain
cd <chain-name>
cargo test

# Test all chains
cargo test --workspace
```

### Integration Testing

```bash
# Start local network
linera net up

# Run deployment script
./scripts/deploy-local.sh

# Run integration tests
cargo test --test integration_tests
```

## Production Deployment

### Testnet Deployment

1. Connect to Linera Devnet:
   ```bash
   linera wallet init --with-new-chain --faucet https://faucet.devnet.linera.net
   ```

2. Follow deployment steps above using devnet

3. Note application IDs for frontend configuration

### Mainnet Deployment

⚠️ **Mainnet not yet available** - Linera is currently in Devnet phase.

When mainnet launches:
1. Audit all smart contract code
2. Test thoroughly on devnet
3. Deploy to mainnet with production configuration
4. Monitor closely in initial weeks

## Configuration

### Environment Variables

Create `.env` file:

```bash
# Linera Network
LINERA_WALLET=$HOME/.config/linera/wallet.json
LINERA_STORAGE=$HOME/.config/linera/storage

# Application IDs (set after deployment)
BATTLE_TOKEN_APP_ID=
PLAYER_APP_ID=
BATTLE_APP_ID=
MATCHMAKING_APP_ID=
PREDICTION_APP_ID=
REGISTRY_APP_ID=

# Platform Configuration
PLATFORM_FEE_BPS=300  # 3%
MIN_STAKE=1000000000  # 1 BATTLE
```

## Security Considerations

### Smart Contract Security
- ✅ No unsafe Rust code
- ✅ Input validation on all operations
- ✅ Amount overflow protection with saturating_add
- ✅ Authentication required for sensitive operations
- ⚠️ Multi-owner chain creation needs SDK research

### Operational Security
- Use hardware wallets for production keys
- Implement multi-sig for treasury
- Monitor unusual activity
- Regular security audits

## Performance Optimization

### Gas Optimization
- Batch operations when possible
- Minimize cross-chain messages
- Use RegisterView for single values
- Use MapView for collections

### Scaling
- Player chains are single-owner (maximum performance)
- Battle chains are multi-owner (both players)
- Registry/Prediction are public (optimized for reads)

## Upgrades

### Upgrading Applications

```bash
# Publish new version
linera publish \
  target/wasm32-unknown-unknown/release/new_version.wasm

# Upgrade existing application
linera upgrade \
  --application-id $APP_ID \
  --bytecode-id $NEW_BYTECODE_ID
```

### Migration

- State migration may be required depending on changes
- Test migrations on testnet first
- Plan for downtime if necessary

## Support

### Documentation
- Linera Docs: https://linera.dev
- BattleChain Docs: /docs

### Community
- Discord: [Link]
- GitHub: https://github.com/uzochukwuV/diccy

### Reporting Issues
- GitHub Issues for bugs
- Discord for questions
- Security issues: security@battlechain.io (if applicable)

## License

See LICENSE file in repository root.
