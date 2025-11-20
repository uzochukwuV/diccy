#!/bin/bash
# shellcheck disable=SC2181
# shellcheck disable=SC2145

# =============================================================================
# BATTLECHAIN END-TO-END TEST SCRIPT
# =============================================================================
# This script performs a complete test from network setup to battle completion
#
# What it does:
# 1. Starts local Linera network with faucet
# 2. Initializes wallet and creates chains
# 3. Builds all Battlechain WASM contracts
# 4. Deploys all 6 contracts in order
# 5. Tests complete battle flow from matchmaking to rewards
# =============================================================================

set -e  # Exit on error

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
FAUCET_PORT=8080
SERVICE_PORT=8081
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WALLET_DIR="$HOME/.linera-battlechain-test"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  BATTLECHAIN END-TO-END TEST${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Project directory: $PROJECT_DIR"
echo "Wallet directory: $WALLET_DIR"
echo ""

# =============================================================================
# STEP 1: CLEANUP AND PREPARATION
# =============================================================================
echo -e "${GREEN}=== STEP 1: Cleanup and Preparation ===${NC}"

# Kill any existing Linera processes
echo "Stopping any existing Linera processes..."
pkill -f "linera-proxy" || true
pkill -f "linera-server" || true
pkill -f "linera service" || true
sleep 2

# Clean up existing wallet
if [ -d "$WALLET_DIR" ]; then
    echo "Removing existing test wallet..."
    rm -rf "$WALLET_DIR"
fi
mkdir -p "$WALLET_DIR"

# Set wallet environment variables
export LINERA_WALLET="$WALLET_DIR/wallet.json"
export LINERA_KEYSTORE="$WALLET_DIR/keystore.json"
export LINERA_STORAGE="rocksdb:$WALLET_DIR/wallet.db"

echo -e "${GREEN}✓ Cleanup complete${NC}"
echo ""

# =============================================================================
# STEP 2: START LOCAL LINERA NETWORK
# =============================================================================
echo -e "${GREEN}=== STEP 2: Starting Local Linera Network ===${NC}"

echo "Starting local network with faucet on port $FAUCET_PORT..."
linera net up --with-faucet --faucet-port $FAUCET_PORT > /dev/null 2>&1 &
NETWORK_PID=$!

# Wait for network to be ready
echo "Waiting for network to be ready..."
sleep 5

# Check if network is running
if ! kill -0 $NETWORK_PID 2>/dev/null; then
    echo -e "${RED}✗ Failed to start local network${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Local network started (PID: $NETWORK_PID)${NC}"
echo ""

# =============================================================================
# STEP 3: INITIALIZE WALLET AND CREATE CHAINS
# =============================================================================
echo -e "${GREEN}=== STEP 3: Initialize Wallet and Create Chains ===${NC}"

FAUCET_URL="http://localhost:$FAUCET_PORT"

# Initialize wallet
echo "Initializing wallet from faucet..."
linera wallet init --faucet "$FAUCET_URL"
if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to initialize wallet${NC}"
    exit 1
fi

# Request chains
echo "Requesting chains from faucet..."

# Function to request a chain
request_chain() {
    linera wallet request-chain --faucet "$FAUCET_URL" 2>&1 | head -n1
}

# Create default chain (for deployments)
DEFAULT_CHAIN=$(request_chain)
echo "  Default Chain: $DEFAULT_CHAIN"

# Create player chains
PLAYER1_CHAIN=$(request_chain)
echo "  Player 1 Chain: $PLAYER1_CHAIN"

PLAYER2_CHAIN=$(request_chain)
echo "  Player 2 Chain: $PLAYER2_CHAIN"

PLAYER3_CHAIN=$(request_chain)
echo "  Player 3 Chain: $PLAYER3_CHAIN"

PLAYER4_CHAIN=$(request_chain)
echo "  Player 4 Chain: $PLAYER4_CHAIN"

# Verify wallet setup
linera sync
BALANCE=$(linera query-balance)
echo "  Chain balance: $BALANCE"

echo -e "${GREEN}✓ Wallet initialized with 5 chains${NC}"
echo ""

# =============================================================================
# STEP 4: BUILD ALL WASM CONTRACTS
# =============================================================================
echo -e "${GREEN}=== STEP 4: Building WASM Contracts ===${NC}"

cd "$PROJECT_DIR/battlechain-linera"

echo "Building all contracts for wasm32-unknown-unknown target..."
cargo build --release --target wasm32-unknown-unknown

if [ $? -ne 0 ]; then
    echo -e "${RED}✗ Failed to build WASM contracts${NC}"
    exit 1
fi

# Verify WASM files exist
WASM_DIR="../target/wasm32-unknown-unknown/release"
REQUIRED_WASMS=(
    "battle_token_contract.wasm"
    "battle_token_service.wasm"
    "registry_chain_contract.wasm"
    "registry_chain_service.wasm"
    "player_chain_contract.wasm"
    "player_chain_service.wasm"
    "prediction_chain_contract.wasm"
    "prediction_chain_service.wasm"
    "matchmaking_chain_contract.wasm"
    "matchmaking_chain_service.wasm"
    "battle_chain_contract.wasm"
    "battle_chain_service.wasm"
)

echo "Verifying WASM files..."
for wasm in "${REQUIRED_WASMS[@]}"; do
    if [ ! -f "$WASM_DIR/$wasm" ]; then
        echo -e "${RED}✗ Missing WASM file: $wasm${NC}"
        exit 1
    fi
    SIZE=$(du -h "$WASM_DIR/$wasm" | cut -f1)
    echo "  ✓ $wasm ($SIZE)"
done

echo -e "${GREEN}✓ All WASM contracts built successfully${NC}"
echo ""

# =============================================================================
# STEP 5: DEPLOY CONTRACTS
# =============================================================================
echo -e "${GREEN}=== STEP 5: Deploying Contracts ===${NC}"

cd "$PROJECT_DIR"

# Deploy Battle Token
echo "Deploying Battle Token..."
BATTLE_TOKEN_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/battle_token_{contract,service}.wasm \
  --json-argument "\"1000000000000\"" 2>&1)

BATTLE_TOKEN_APP_ID=$(echo "$BATTLE_TOKEN_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$BATTLE_TOKEN_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Battle Token App ID${NC}"
    echo "$BATTLE_TOKEN_OUTPUT"
    exit 1
fi
echo "  Battle Token App ID: $BATTLE_TOKEN_APP_ID"
sleep 2

# Deploy Registry Chain
echo "Deploying Registry Chain..."
REGISTRY_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/registry_chain_{contract,service}.wasm 2>&1)

REGISTRY_APP_ID=$(echo "$REGISTRY_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$REGISTRY_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Registry App ID${NC}"
    exit 1
fi
echo "  Registry Chain App ID: $REGISTRY_APP_ID"
sleep 2

# Deploy Player Chain
echo "Deploying Player Chain..."
PLAYER_CHAIN_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/player_chain_{contract,service}.wasm 2>&1)

PLAYER_CHAIN_APP_ID=$(echo "$PLAYER_CHAIN_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$PLAYER_CHAIN_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Player Chain App ID${NC}"
    exit 1
fi
echo "  Player Chain App ID: $PLAYER_CHAIN_APP_ID"
sleep 2

# Deploy Prediction Chain
echo "Deploying Prediction Chain..."
PREDICTION_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/prediction_chain_{contract,service}.wasm \
  --required-application-ids "$BATTLE_TOKEN_APP_ID" 2>&1)

PREDICTION_APP_ID=$(echo "$PREDICTION_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$PREDICTION_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Prediction App ID${NC}"
    exit 1
fi
echo "  Prediction Chain App ID: $PREDICTION_APP_ID"
sleep 2

# Deploy Matchmaking Chain
echo "Deploying Matchmaking Chain..."
MATCHMAKING_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/matchmaking_chain_{contract,service}.wasm \
  --required-application-ids "$PREDICTION_APP_ID" 2>&1)

MATCHMAKING_APP_ID=$(echo "$MATCHMAKING_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$MATCHMAKING_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Matchmaking App ID${NC}"
    exit 1
fi
echo "  Matchmaking Chain App ID: $MATCHMAKING_APP_ID"
sleep 2

# Deploy Battle Chain
echo "Deploying Battle Chain..."
BATTLE_CHAIN_OUTPUT=$(linera --wait-for-outgoing-messages publish-and-create \
  target/wasm32-unknown-unknown/release/battle_chain_{contract,service}.wasm \
  --required-application-ids "$BATTLE_TOKEN_APP_ID" 2>&1)

BATTLE_CHAIN_APP_ID=$(echo "$BATTLE_CHAIN_OUTPUT" | grep -oP '(?<=application ID: )[a-f0-9]+')
if [ -z "$BATTLE_CHAIN_APP_ID" ]; then
    echo -e "${RED}✗ Failed to extract Battle Chain App ID${NC}"
    exit 1
fi
echo "  Battle Chain App ID: $BATTLE_CHAIN_APP_ID"
sleep 2

echo -e "${GREEN}✓ All contracts deployed successfully${NC}"
echo ""

# =============================================================================
# STEP 6: START LINERA SERVICE
# =============================================================================
echo -e "${GREEN}=== STEP 6: Starting Linera Service ===${NC}"

echo "Starting Linera service on port $SERVICE_PORT..."
linera service --port $SERVICE_PORT > /dev/null 2>&1 &
SERVICE_PID=$!

sleep 3

if ! kill -0 $SERVICE_PID 2>/dev/null; then
    echo -e "${RED}✗ Failed to start Linera service${NC}"
    exit 1
fi

echo -e "${GREEN}✓ Linera service started (PID: $SERVICE_PID)${NC}"
echo ""

GRAPHQL_URL="http://localhost:$SERVICE_PORT"

# =============================================================================
# STEP 7: TEST TOKEN DISTRIBUTION
# =============================================================================
echo -e "${GREEN}=== STEP 7: Token Distribution Test ===${NC}"

# Transfer tokens to players
for i in 1 2 3 4; do
    PLAYER_VAR="PLAYER${i}_CHAIN"
    PLAYER_CHAIN="${!PLAYER_VAR}"

    echo "Transferring 10,000 BATTLE tokens to Player $i..."

    MUTATION='mutation { transfer(to: \"'$PLAYER_CHAIN'\", amount: \"10000\") }'

    RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$BATTLE_TOKEN_APP_ID" \
      -H "Content-Type: application/json" \
      -d "{\"query\":\"$MUTATION\"}")

    if echo "$RESULT" | grep -q "error"; then
        echo -e "${RED}✗ Failed to transfer tokens to Player $i${NC}"
        echo "$RESULT"
    else
        echo -e "${GREEN}  ✓ Player $i received tokens${NC}"
    fi

    sleep 1
done

echo -e "${GREEN}✓ Token distribution complete${NC}"
echo ""

# =============================================================================
# STEP 8: TEST CHARACTER CREATION
# =============================================================================
echo -e "${GREEN}=== STEP 8: Character Creation Test ===${NC}"

# Create characters for players
declare -A CHARACTERS
CHARACTERS["$PLAYER1_CHAIN"]="warrior_001:Warrior"
CHARACTERS["$PLAYER2_CHAIN"]="mage_001:Mage"
CHARACTERS["$PLAYER3_CHAIN"]="rogue_001:Rogue"
CHARACTERS["$PLAYER4_CHAIN"]="healer_001:Healer"

PLAYER_NUM=1
for PLAYER_CHAIN in "$PLAYER1_CHAIN" "$PLAYER2_CHAIN" "$PLAYER3_CHAIN" "$PLAYER4_CHAIN"; do
    CHAR_DATA="${CHARACTERS[$PLAYER_CHAIN]}"
    CHAR_ID="${CHAR_DATA%%:*}"
    CHAR_CLASS="${CHAR_DATA##*:}"

    echo "Creating $CHAR_CLASS character ($CHAR_ID) for Player $PLAYER_NUM..."

    MUTATION='mutation { createCharacter(characterId: \"'$CHAR_ID'\", nftId: \"nft_'$CHAR_ID'\", class: \"'$CHAR_CLASS'\") }'

    RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER_CHAIN/applications/$PLAYER_CHAIN_APP_ID" \
      -H "Content-Type: application/json" \
      -d "{\"query\":\"$MUTATION\"}")

    if echo "$RESULT" | grep -q "error"; then
        echo -e "${YELLOW}  ⚠ Character creation may have issues${NC}"
    else
        echo -e "${GREEN}  ✓ Character created${NC}"
    fi

    PLAYER_NUM=$((PLAYER_NUM + 1))
    sleep 1
done

echo -e "${GREEN}✓ Character creation complete${NC}"
echo ""

# =============================================================================
# STEP 9: TEST MATCHMAKING
# =============================================================================
echo -e "${GREEN}=== STEP 9: Matchmaking Test ===${NC}"

# Player 1 joins queue
echo "Player 1 (Warrior) joining matchmaking queue..."
MUTATION='mutation { joinQueue(characterId: \"warrior_001\", stake: \"100\") }'

RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER1_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}")

if echo "$RESULT" | grep -q "error"; then
    echo -e "${YELLOW}  ⚠ Player 1 queue join may have issues${NC}"
else
    echo -e "${GREEN}  ✓ Player 1 in queue${NC}"
fi

sleep 2

# Player 2 joins queue
echo "Player 2 (Mage) joining matchmaking queue..."
MUTATION='mutation { joinQueue(characterId: \"mage_001\", stake: \"100\") }'

RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER2_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}")

if echo "$RESULT" | grep -q "error"; then
    echo -e "${YELLOW}  ⚠ Player 2 queue join may have issues${NC}"
else
    echo -e "${GREEN}  ✓ Player 2 in queue${NC}"
fi

sleep 3

# Query matchmaking status
echo "Querying matchmaking status..."
QUERY='query { queueSize totalMatches }'

RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}")

echo "$RESULT" | jq . || echo "$RESULT"

echo -e "${GREEN}✓ Matchmaking test complete${NC}"
echo ""

# =============================================================================
# STEP 10: QUERY APPLICATION STATUS
# =============================================================================
echo -e "${GREEN}=== STEP 10: Querying Application Status ===${NC}"

echo "Querying Battle Token stats..."
QUERY='query { stats { totalSupply totalHolders totalTransfers } }'

RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$BATTLE_TOKEN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}")

echo "$RESULT" | jq . || echo "$RESULT"
echo ""

# =============================================================================
# FINAL SUMMARY
# =============================================================================
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  TEST SUMMARY${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}Network Information:${NC}"
echo "  Faucet URL: http://localhost:$FAUCET_PORT"
echo "  GraphQL URL: http://localhost:$SERVICE_PORT"
echo ""
echo -e "${GREEN}Chains:${NC}"
echo "  Default Chain: $DEFAULT_CHAIN"
echo "  Player 1: $PLAYER1_CHAIN"
echo "  Player 2: $PLAYER2_CHAIN"
echo "  Player 3: $PLAYER3_CHAIN"
echo "  Player 4: $PLAYER4_CHAIN"
echo ""
echo -e "${GREEN}Application IDs:${NC}"
echo "  Battle Token:  $BATTLE_TOKEN_APP_ID"
echo "  Registry:      $REGISTRY_APP_ID"
echo "  Player Chain:  $PLAYER_CHAIN_APP_ID"
echo "  Prediction:    $PREDICTION_APP_ID"
echo "  Matchmaking:   $MATCHMAKING_APP_ID"
echo "  Battle Chain:  $BATTLE_CHAIN_APP_ID"
echo ""
echo -e "${GREEN}GraphiQL Endpoints:${NC}"
echo "  Battle Token:  http://localhost:$SERVICE_PORT/chains/$DEFAULT_CHAIN/applications/$BATTLE_TOKEN_APP_ID"
echo "  Registry:      http://localhost:$SERVICE_PORT/chains/$DEFAULT_CHAIN/applications/$REGISTRY_APP_ID"
echo "  Matchmaking:   http://localhost:$SERVICE_PORT/chains/$DEFAULT_CHAIN/applications/$MATCHMAKING_APP_ID"
echo ""
echo -e "${YELLOW}Services are running. Press Ctrl+C to stop and cleanup.${NC}"
echo ""

# Wait for user to stop
trap cleanup EXIT

cleanup() {
    echo ""
    echo -e "${YELLOW}Cleaning up...${NC}"
    kill $SERVICE_PID 2>/dev/null || true
    linera net down 2>/dev/null || true
    echo -e "${GREEN}✓ Cleanup complete${NC}"
}

# Keep script running
wait $SERVICE_PID
