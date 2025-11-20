#!/usr/bin/env bash
# BattleChain Local Deployment Script
# Based on microcard's deployment pattern
# Deploys all BattleChain applications to a local Linera testnet

set -eu

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
FAUCET_PORT=8080
LINERA_SERVICE_PORT=8081
FAUCET_URL=http://localhost:$FAUCET_PORT
GRAPHQL_URL=http://localhost:$LINERA_SERVICE_PORT

# Token and game parameters
INITIAL_TOKEN_SUPPLY=1000000000000  # 1 trillion tokens (with 6 decimals = 1M BATTLE)
PLAYER_COUNT=4
BATTLE_STAKE_AMOUNT=10000  # 0.01 BATTLE per battle

# Print functions
print_header() {
    echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║       BattleChain Local Deployment Script            ║${NC}"
    echo -e "${GREEN}║  PvP Fighting Game on Linera Blockchain               ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

print_section() {
    echo -e "\n${CYAN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════${NC}\n"
}

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

error_exit() {
    print_error "$1"
    exit 1
}

print_header

# ============================================================================
# STEP 1: Start Linera Network
# ============================================================================
print_section "STEP 1: Starting Linera Network"

print_info "Starting local Linera testnet with faucet..."

# Add linera to PATH if it exists in cargo bin
export PATH="$HOME/.cargo/bin:$PATH"

# Initialize Linera network helper
source /dev/stdin <<<"$(linera net helper 2>/dev/null)" || error_exit "Failed to load Linera network helper"

# Start network with faucet
linera net up --initial-amount 1000000000000 --with-faucet \
    --faucet-port $FAUCET_PORT \
    --faucet-amount 1000000000 2>&1 &
NETWORK_PID=$!

print_success "Network started (PID: $NETWORK_PID)"
sleep 5

# ============================================================================
# STEP 2: Create Wallets and Chains
# ============================================================================
print_section "STEP 2: Creating Wallets and Chains"

# Function to initialize wallet from faucet
initiate_wallet() {
    linera wallet init --faucet "$FAUCET_URL" 2>&1
    if [ $? -ne 0 ]; then
        error_exit "Failed to initialize wallet"
    fi
}

# Function to open new chain from faucet
open_chain() {
    linera wallet request-chain --faucet "$FAUCET_URL" 2>&1
    if [ $? -ne 0 ]; then
        error_exit "Failed to open chain"
    fi
}

print_info "Initializing default wallet..."
INIT_OUTPUT=$(initiate_wallet)

print_info "Creating admin chain..."
ADMIN_CHAIN_OUTPUT=$(open_chain)
mapfile -t StringArray <<< "$ADMIN_CHAIN_OUTPUT"
ADMIN_CHAIN_ID=${StringArray[0]}
print_success "Admin chain: $ADMIN_CHAIN_ID"

# Create player chains
print_info "Creating $PLAYER_COUNT player chains..."
PLAYER_CHAIN_IDS=()
for i in $(seq 1 $PLAYER_COUNT); do
    PLAYER_CHAIN_OUTPUT=$(open_chain)
    mapfile -t StringArray <<< "$PLAYER_CHAIN_OUTPUT"
    PLAYER_CHAIN_ID=${StringArray[0]}
    PLAYER_CHAIN_IDS+=("$PLAYER_CHAIN_ID")
    print_success "Player $i chain: $PLAYER_CHAIN_ID"
    sleep 1
done

# Sync and check balance
linera sync && linera query-balance
echo ""

# ============================================================================
# STEP 3: Build All Chains
# ============================================================================
print_section "STEP 3: Building All Chains"

print_info "Building WASM artifacts..."
cd "$(dirname "$0")/.."
PROJECT_ROOT=$(pwd)

cargo build --all --release --target wasm32-unknown-unknown || error_exit "Build failed"
print_success "All chains built successfully"

# ============================================================================
# STEP 4: Deploy Battle Token
# ============================================================================
print_section "STEP 4: Deploying Battle Token"

print_info "Deploying battle-token with initial supply: $INITIAL_TOKEN_SUPPLY..."

BATTLE_TOKEN_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . battle-token \
    --json-parameters "$INITIAL_TOKEN_SUPPLY" 2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy battle-token"
fi

print_success "Battle Token deployed: $BATTLE_TOKEN_APP_ID"
sleep 3

# ============================================================================
# STEP 5: Deploy Player Chains
# ============================================================================
print_section "STEP 5: Deploying Player Chain Application"

print_info "Deploying player-chain application..."

PLAYER_CHAIN_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . player-chain \
    2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy player-chain"
fi

print_success "Player Chain deployed: $PLAYER_CHAIN_APP_ID"
sleep 3

# ============================================================================
# STEP 6: Deploy Battle Chain
# ============================================================================
print_section "STEP 6: Deploying Battle Chain"

print_info "Deploying battle-chain application..."

BATTLE_CHAIN_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . battle-chain \
    2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy battle-chain"
fi

print_success "Battle Chain deployed: $BATTLE_CHAIN_APP_ID"
sleep 3

# ============================================================================
# STEP 7: Deploy Matchmaking Chain
# ============================================================================
print_section "STEP 7: Deploying Matchmaking Chain"

print_info "Deploying matchmaking-chain application..."

MATCHMAKING_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . matchmaking-chain \
    2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy matchmaking-chain"
fi

print_success "Matchmaking Chain deployed: $MATCHMAKING_APP_ID"
sleep 3

# ============================================================================
# STEP 8: Deploy Prediction Chain
# ============================================================================
print_section "STEP 8: Deploying Prediction Chain"

print_info "Deploying prediction-chain application..."

# Platform fee: 100 basis points (1%)
PREDICTION_CHAIN_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . prediction-chain \
    --json-parameters "{\"platform_fee_bps\": 100}" 2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy prediction-chain"
fi

print_success "Prediction Chain deployed: $PREDICTION_CHAIN_APP_ID"
sleep 3

# ============================================================================
# STEP 9: Deploy Registry Chain
# ============================================================================
print_section "STEP 9: Deploying Registry Chain"

print_info "Deploying registry-chain application..."

REGISTRY_CHAIN_APP_ID=$(linera --wait-for-outgoing-messages project publish-and-create . registry-chain \
    2>&1)

if [ $? -ne 0 ]; then
    error_exit "Failed to deploy registry-chain"
fi

print_success "Registry Chain deployed: $REGISTRY_CHAIN_APP_ID"
sleep 3

# ============================================================================
# STEP 10: Start GraphQL Service
# ============================================================================
print_section "STEP 10: Starting GraphQL Service"

print_info "Starting Linera service on port $LINERA_SERVICE_PORT..."
linera service --port $LINERA_SERVICE_PORT &
SERVICE_PID=$!

sleep 5
print_success "Service started (PID: $SERVICE_PID)"

# ============================================================================
# STEP 11: Initialize Player Chains
# ============================================================================
print_section "STEP 11: Initializing Player Chains"

print_info "Initializing player chains with battle-token reference..."

for i in "${!PLAYER_CHAIN_IDS[@]}"; do
    PLAYER_NUM=$((i + 1))
    PLAYER_CHAIN=${PLAYER_CHAIN_IDS[$i]}

    print_info "Initializing Player $PLAYER_NUM chain: $PLAYER_CHAIN"

    # GraphQL mutation to initialize player chain
    MUTATION="mutation { initialize(battleTokenApp: \\\"$BATTLE_TOKEN_APP_ID\\\") }"

    curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER_CHAIN/applications/$PLAYER_CHAIN_APP_ID" \
        -H "Content-Type: application/json" \
        -d "{\"query\":\"$MUTATION\"}" \
        | jq . || print_error "Failed to initialize Player $PLAYER_NUM"

    sleep 2
done

print_success "All player chains initialized"

# ============================================================================
# STEP 12: Distribute Tokens to Players
# ============================================================================
print_section "STEP 12: Distributing BATTLE Tokens"

TOKENS_PER_PLAYER=100000000  # 100 BATTLE per player

print_info "Distributing $TOKENS_PER_PLAYER tokens to each player..."

for i in "${!PLAYER_CHAIN_IDS[@]}"; do
    PLAYER_NUM=$((i + 1))
    PLAYER_CHAIN=${PLAYER_CHAIN_IDS[$i]}

    print_info "Sending tokens to Player $PLAYER_NUM..."

    # TODO: This needs the proper Owner format
    # For now, we'll skip actual distribution - needs to be done via battle-token operations
    # MUTATION="mutation { transfer(to: \\\"$PLAYER_CHAIN\\\", amount: \\\"$TOKENS_PER_PLAYER\\\") }"

    print_info "Tokens distribution requires proper Owner format - skipping for now"
    sleep 1
done

# ============================================================================
# STEP 13: Generate Configuration File
# ============================================================================
print_section "STEP 13: Generating Configuration"

print_info "Creating deployment config..."

CONFIG_FILE="$PROJECT_ROOT/deployment-config.json"

jq -n \
    --arg graphqlUrl "$GRAPHQL_URL" \
    --arg adminChain "$ADMIN_CHAIN_ID" \
    --arg battleTokenApp "$BATTLE_TOKEN_APP_ID" \
    --arg playerChainApp "$PLAYER_CHAIN_APP_ID" \
    --arg battleChainApp "$BATTLE_CHAIN_APP_ID" \
    --arg matchmakingApp "$MATCHMAKING_APP_ID" \
    --arg predictionApp "$PREDICTION_CHAIN_APP_ID" \
    --arg registryApp "$REGISTRY_CHAIN_APP_ID" \
    --argjson playerChains "$(printf '%s\n' "${PLAYER_CHAIN_IDS[@]}" | jq -R . | jq -s .)" \
    '{
        graphqlUrl: $graphqlUrl,
        adminChain: $adminChain,
        applications: {
            battleToken: $battleTokenApp,
            playerChain: $playerChainApp,
            battleChain: $battleChainApp,
            matchmaking: $matchmakingApp,
            prediction: $predictionApp,
            registry: $registryApp
        },
        playerChains: $playerChains
    }' > "$CONFIG_FILE"

print_success "Configuration saved to: $CONFIG_FILE"

# ============================================================================
# Deployment Complete
# ============================================================================
print_section "Deployment Summary"

echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║        🚀 BattleChain Deployment Complete! 🚀         ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
echo ""

print_info "Network Configuration:"
echo "  GraphQL URL:    $GRAPHQL_URL"
echo "  Faucet URL:     $FAUCET_URL"
echo "  Admin Chain:    $ADMIN_CHAIN_ID"
echo ""

print_info "Deployed Applications:"
echo "  Battle Token:   $BATTLE_TOKEN_APP_ID"
echo "  Player Chain:   $PLAYER_CHAIN_APP_ID"
echo "  Battle Chain:   $BATTLE_CHAIN_APP_ID"
echo "  Matchmaking:    $MATCHMAKING_APP_ID"
echo "  Prediction:     $PREDICTION_CHAIN_APP_ID"
echo "  Registry:       $REGISTRY_CHAIN_APP_ID"
echo ""

print_info "Player Chains:"
for i in "${!PLAYER_CHAIN_IDS[@]}"; do
    echo "  Player $((i + 1)):       ${PLAYER_CHAIN_IDS[$i]}"
done
echo ""

print_info "Service Status:"
echo "  Network PID:    $NETWORK_PID"
echo "  Service PID:    $SERVICE_PID"
echo ""

print_success "Configuration: $CONFIG_FILE"
echo ""

print_info "Next Steps:"
echo "  1. Test battle-token operations:"
echo "     curl -X POST $GRAPHQL_URL/chains/$ADMIN_CHAIN_ID/applications/$BATTLE_TOKEN_APP_ID \\"
echo "       -H 'Content-Type: application/json' \\"
echo "       -d '{\"query\":\"query { name symbol totalSupply }\"}'"
echo ""
echo "  2. Create characters for players"
echo "  3. Start a battle via matchmaking"
echo "  4. Place bets on prediction market"
echo ""

print_info "To stop services:"
echo "  kill $SERVICE_PID  # Stop GraphQL service"
echo "  kill $NETWORK_PID  # Stop Linera network"
echo ""

print_success "Deployment complete!"
