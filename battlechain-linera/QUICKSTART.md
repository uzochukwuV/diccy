# Battlechain Quick Start Guide

Complete guide to get Battlechain running from scratch in minutes.

## Prerequisites

- Rust toolchain with `wasm32-unknown-unknown` target
- Linera CLI tools installed
- `jq` for JSON processing

```bash
# Install Rust and wasm32 target
rustup target add wasm32-unknown-unknown

# Install jq
sudo apt-get install jq  # Ubuntu/Debian
# or
brew install jq  # macOS
```

## 🚀 One-Command Setup (Recommended)

Run the complete end-to-end test that sets up everything:

```bash
cd battlechain-linera
./tests/test_end_to_end.sh
```

This script will:
1. ✅ Start local Linera network with faucet
2. ✅ Initialize wallet and create 5 chains
3. ✅ Build all WASM contracts
4. ✅ Deploy all 6 contracts
5. ✅ Test token distribution
6. ✅ Test character creation
7. ✅ Test matchmaking
8. ✅ Keep services running for manual testing

**Expected runtime:** ~2-3 minutes

**Output:** Application IDs and GraphiQL URLs for all contracts

## 📝 Manual Setup (Step-by-Step)

If you prefer to run steps manually:

### Step 1: Start Local Network

```bash
# Start network with faucet
linera net up --with-faucet --faucet-port 8080
```

### Step 2: Initialize Wallet

```bash
# Set custom wallet location (optional)
export LINERA_WALLET="$HOME/.linera-battlechain/wallet.json"
export LINERA_KEYSTORE="$HOME/.linera-battlechain/keystore.json"
export LINERA_STORAGE="rocksdb:$HOME/.linera-battlechain/wallet.db"

# Initialize from faucet
linera wallet init --faucet http://localhost:8080

# Request chains
linera wallet request-chain --faucet http://localhost:8080  # Default chain
linera wallet request-chain --faucet http://localhost:8080  # Player 1
linera wallet request-chain --faucet http://localhost:8080  # Player 2

# Verify
linera sync
linera query-balance
```

### Step 3: Build Contracts

```bash
cd battlechain-linera
cargo build --release --target wasm32-unknown-unknown
```

**Verify WASM files:**
```bash
ls -lh ../target/wasm32-unknown-unknown/release/*_{contract,service}.wasm
```

### Step 4: Deploy Contracts

Deploy in this order:

```bash
cd ..  # Back to project root

# 1. Battle Token
linera publish-and-create \
  target/wasm32-unknown-unknown/release/battle_token_{contract,service}.wasm \
  --json-argument "\"1000000000000\""

# 2. Registry Chain
linera publish-and-create \
  target/wasm32-unknown-unknown/release/registry_chain_{contract,service}.wasm

# 3. Player Chain
linera publish-and-create \
  target/wasm32-unknown-unknown/release/player_chain_{contract,service}.wasm

# 4. Prediction Chain (requires Battle Token App ID)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/prediction_chain_{contract,service}.wasm \
  --required-application-ids <BATTLE_TOKEN_APP_ID>

# 5. Matchmaking Chain (requires Prediction App ID)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/matchmaking_chain_{contract,service}.wasm \
  --required-application-ids <PREDICTION_APP_ID>

# 6. Battle Chain (requires Battle Token App ID)
linera publish-and-create \
  target/wasm32-unknown-unknown/release/battle_chain_{contract,service}.wasm \
  --required-application-ids <BATTLE_TOKEN_APP_ID>
```

**Note:** Save the Application IDs from each deployment - you'll need them!

### Step 5: Start Service

```bash
linera service --port 8081
```

### Step 6: Test with GraphiQL

Open http://localhost:8081 in your browser

Get your chain ID:
```bash
linera wallet show
```

List applications:
```graphql
query {
  applications(chainId: "YOUR_CHAIN_ID") {
    id
    description
    link
  }
}
```

Click on a `link` to access the application's GraphiQL interface.

## 🧪 Running Tests

### Unit Tests

```bash
cd battlechain-linera
cargo test --all --target x86_64-unknown-linux-gnu
```

### Integration Tests

After running the end-to-end setup:

```bash
# Test battle flow (use IDs from deployment)
./tests/test_battle_flow.sh \
  http://localhost:8081 \
  <DEFAULT_CHAIN> \
  <PLAYER1_CHAIN> \
  <PLAYER2_CHAIN> \
  <MATCHMAKING_APP_ID> \
  <BATTLE_CHAIN_APP_ID>
```

## 📊 Example GraphQL Queries

### Battle Token

```graphql
# Query token stats
query {
  stats {
    totalSupply
    totalHolders
    totalTransfers
    totalBurned
    circulatingSupply
  }
}

# Transfer tokens
mutation {
  transfer(to: "RECIPIENT_OWNER", amount: "1000")
}
```

### Player Chain

```graphql
# Create character
mutation {
  createCharacter(
    characterId: "warrior_001"
    nftId: "nft_001"
    class: "Warrior"
  )
}

# Query characters
query {
  totalCharacters
  characters {
    characterId
    class
    level
  }
}
```

### Matchmaking Chain

```graphql
# Join queue
mutation {
  joinQueue(
    characterId: "warrior_001"
    stake: "100"
  )
}

# Query queue status
query {
  queueSize
  totalMatches
  waitingPlayers {
    owner
    characterId
    stake
  }
}
```

### Registry Chain

```graphql
# Query leaderboard
query {
  leaderboard(limit: 10) {
    characterId
    elo
    wins
    losses
    winRate
  }
}

# Query character stats
query {
  character(characterId: "warrior_001") {
    totalBattles
    wins
    losses
    totalDamageDealt
    totalDamageTaken
    elo
  }
}
```

## 🐛 Troubleshooting

### "Address already in use"

Another service is using the port:

```bash
# Kill existing services
pkill -f "linera"
linera net down

# Then restart
linera net up --with-faucet --faucet-port 8080
```

### "Network not running"

```bash
# Check if network is up
linera net info

# If not, start it
linera net up --with-faucet --faucet-port 8080
```

### "Failed to build WASM"

```bash
# Ensure wasm32 target is installed
rustup target add wasm32-unknown-unknown

# Clean and rebuild
cargo clean
cargo build --release --target wasm32-unknown-unknown
```

### "Application not found"

Make sure you're using the correct:
- Chain ID (from `linera wallet show`)
- Application ID (from deployment output)
- Endpoint URL (from service)

### Wallet Issues

```bash
# Reset wallet (CAUTION: deletes all chains)
rm -rf ~/.linera-battlechain/
linera wallet init --faucet http://localhost:8080
```

## 🔄 Clean Restart

To start fresh:

```bash
# Stop all services
pkill -f "linera"
linera net down

# Remove test wallet
rm -rf ~/.linera-battlechain/

# Run end-to-end test again
./tests/test_end_to_end.sh
```

## 📚 Next Steps

1. **Read the Test Documentation**: `tests/README_TESTS.md`
2. **Explore Contract Code**: Each contract has detailed documentation
3. **Run Unit Tests**: Test individual contract functionality
4. **Customize**: Modify parameters, add features
5. **Deploy to Testnet**: Use Linera testnet faucet instead of local

## 🆘 Getting Help

- **Documentation**: Check `tests/README_TESTS.md` and contract READMEs
- **Logs**: Service logs show detailed error messages
- **Linera Docs**: https://docs.linera.io
- **Issues**: Report bugs in the repository

## 🎯 Quick Command Reference

```bash
# Network
linera net up --with-faucet --faucet-port 8080  # Start
linera net down                                  # Stop
linera net info                                  # Status

# Wallet
linera wallet init --faucet <URL>               # Initialize
linera wallet show                               # Show chains
linera sync                                      # Sync
linera query-balance                             # Check balance

# Deploy
linera publish-and-create <contract> <service> \
  --json-argument <ARG> \
  --required-application-ids <ID>

# Service
linera service --port 8081                       # Start GraphQL service

# Build
cargo build --release --target wasm32-unknown-unknown  # Build WASM
cargo test --all --target x86_64-unknown-linux-gnu    # Run tests
```

---

**Ready to battle!** 🎮⚔️
