#!/bin/bash
set -e

echo "🚀 Deploying BattleChain Applications"
echo "====================================="
echo ""

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if linera CLI is installed
if ! command -v linera &> /dev/null; then
    echo -e "${RED}ERROR: linera CLI not found${NC}"
    echo "Please install Linera CLI first:"
    echo "  cargo install linera-client --version 0.15.5"
    exit 1
fi

# Navigate to project root
cd "$(dirname "$0")/.."

echo -e "${YELLOW}Step 1: Publishing bytecodes...${NC}"
echo ""

# Publish shared types (if needed)
echo "📦 Shared types..."
echo ../target/wasm32-unknown-unknown/release/battlechain_shared_types_{contract,service}.wasm
SHARED_TYPES_BYTECODE=$(linera publish-and-create \
  ../target/wasm32-unknown-unknown/release/battlechain_shared_types_{contract,service}.wasm 2>/dev/null || echo "")
if [ -n "$SHARED_TYPES_BYTECODE" ]; then
    echo -e "${GREEN}✅ Shared types bytecode: $SHARED_TYPES_BYTECODE${NC}"
fi

# Publish battle token
echo "📦 Battle Token..."
BATTLE_TOKEN_BYTECODE=$(linera publish-and-create \
  battle-token/target/wasm32-unknown-unknown/release/battle_token_{contract,service}.wasm)
echo -e "${GREEN}✅ Battle Token bytecode: $BATTLE_TOKEN_BYTECODE${NC}"

# Publish player chain
echo "📦 Player Chain..."
PLAYER_BYTECODE=$(linera publish-and-create \
  player-chain/target/wasm32-unknown-unknown/release/player_chain_{contract,service}.wasm)
echo -e "${GREEN}✅ Player Chain bytecode: $PLAYER_BYTECODE${NC}"

# Publish battle chain
echo "📦 Battle Chain..."
BATTLE_BYTECODE=$(linera publish-and-create \
  battle-chain/target/wasm32-unknown-unknown/release/battle_chain_{contract,service}.wasm)
echo -e "${GREEN}✅ Battle Chain bytecode: $BATTLE_BYTECODE${NC}"

# Publish registry
echo "📦 Registry Chain..."
REGISTRY_BYTECODE=$(linera publish-and-create \
  registry-chain/target/wasm32-unknown-unknown/release/registry_chain_{contract,service}.wasm)
echo -e "${GREEN}✅ Registry Chain bytecode: $REGISTRY_BYTECODE${NC}"

# Publish prediction market
echo "📦 Prediction Market..."
PREDICTION_BYTECODE=$(linera publish-and-create \
  prediction-chain/target/wasm32-unknown-unknown/release/prediction_chain_{contract,service}.wasm)
echo -e "${GREEN}✅ Prediction Market bytecode: $PREDICTION_BYTECODE${NC}"

# Publish matchmaking
echo "📦 Matchmaking Chain..."
MATCHMAKING_BYTECODE=$(linera publish-and-create \
  matchmaking-chain/target/wasm32-unknown-unknown/release/matchmaking_chain_{contract,service}.wasm)
echo -e "${GREEN}✅ Matchmaking Chain bytecode: $MATCHMAKING_BYTECODE${NC}"

echo ""
echo -e "${YELLOW}Step 2: Creating application instances...${NC}"
echo ""
echo "Creating applications in dependency order:"
echo "  Token → Registry → Player → Battle → Prediction → Matchmaking"
echo ""

# 1. Battle Token (no dependencies)
echo "🪙 Creating Battle Token application..."
BATTLE_TOKEN_APP=$(linera create-application $BATTLE_TOKEN_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{"name":"BattleToken","symbol":"BATTLE","initial_supply":"1000000000000"}')
echo -e "${GREEN}✅ Battle Token app: $BATTLE_TOKEN_APP${NC}"

# 2. Registry (no dependencies)
echo "📊 Creating Registry application..."
REGISTRY_APP=$(linera create-application $REGISTRY_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{}')
echo -e "${GREEN}✅ Registry app: $REGISTRY_APP${NC}"

# 3. Player Chain (depends on token)
echo "👤 Creating Player Chain application..."
PLAYER_APP=$(linera create-application $PLAYER_BYTECODE \
  --json-parameters '100000000' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo -e "${GREEN}✅ Player app: $PLAYER_APP${NC}"

# 4. Battle Chain (depends on token)
echo "⚔️  Creating Battle Chain application..."
BATTLE_APP=$(linera create-application $BATTLE_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo -e "${GREEN}✅ Battle app: $BATTLE_APP${NC}"

# 5. Prediction Market (depends on token)
echo "🎲 Creating Prediction Market application..."
PREDICTION_APP=$(linera create-application $PREDICTION_BYTECODE \
  --json-parameters '300' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo -e "${GREEN}✅ Prediction app: $PREDICTION_APP${NC}"

# 6. Matchmaking (depends on BATTLE app - CRITICAL for auto-deployment!)
echo "🤝 Creating Matchmaking application..."
echo -e "${YELLOW}   ⚠️  Including battle app in required dependencies for auto-deployment!${NC}"
MATCHMAKING_APP=$(linera create-application $MATCHMAKING_BYTECODE \
  --json-parameters '1000000000' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_APP \
  --required-application-ids $BATTLE_TOKEN_APP \
  --required-application-ids $PLAYER_APP \
  --required-application-ids $REGISTRY_APP)
echo -e "${GREEN}✅ Matchmaking app: $MATCHMAKING_APP${NC}"

echo ""
echo -e "${YELLOW}Step 3: Configuring application references...${NC}"
echo ""

# Get default chain owner
DEFAULT_OWNER=$(linera wallet show | grep "Owner" | head -1 | awk '{print $2}')
DEFAULT_CHAIN=$(linera wallet show | grep "Public Key" -A 10 | grep "Chain" | head -1 | awk '{print $3}')

echo "Using default owner: $DEFAULT_OWNER"
echo "Using default chain: $DEFAULT_CHAIN"
echo ""

# Update matchmaking with battle app ID
echo "Configuring matchmaking with application references..."
linera execute-operation \
  --application-id $MATCHMAKING_APP \
  --json-operation "{
    \"UpdateReferences\": {
      \"battle_app_id\": \"$BATTLE_APP\",
      \"battle_token_app\": \"$BATTLE_TOKEN_APP\",
      \"treasury_owner\": {\"Account\": \"$DEFAULT_OWNER\"}
    }
  }" || echo -e "${YELLOW}⚠️  Warning: Could not configure references (may need manual setup)${NC}"

echo -e "${GREEN}✅ Configuration complete${NC}"

echo ""
echo -e "${YELLOW}Step 4: Saving deployment information...${NC}"
echo ""

# Create .env file
cat > .env << EOF
# BattleChain Application IDs
# Generated: $(date)

# Applications
BATTLE_TOKEN_APP=$BATTLE_TOKEN_APP
PLAYER_APP=$PLAYER_APP
BATTLE_APP=$BATTLE_APP
REGISTRY_APP=$REGISTRY_APP
PREDICTION_APP=$PREDICTION_APP
MATCHMAKING_APP=$MATCHMAKING_APP

# Bytecode IDs
BATTLE_TOKEN_BYTECODE=$BATTLE_TOKEN_BYTECODE
PLAYER_BYTECODE=$PLAYER_BYTECODE
BATTLE_BYTECODE=$BATTLE_BYTECODE
REGISTRY_BYTECODE=$REGISTRY_BYTECODE
PREDICTION_BYTECODE=$PREDICTION_BYTECODE
MATCHMAKING_BYTECODE=$MATCHMAKING_BYTECODE

# Default chain and owner
DEFAULT_CHAIN=$DEFAULT_CHAIN
DEFAULT_OWNER=$DEFAULT_OWNER
EOF

echo -e "${GREEN}✅ Application IDs saved to .env${NC}"

echo ""
echo "======================================"
echo -e "${GREEN}🎉 Deployment Successful!${NC}"
echo "======================================"
echo ""
echo "Application Summary:"
echo "  🪙 Battle Token:    $BATTLE_TOKEN_APP"
echo "  👤 Player Chain:    $PLAYER_APP"
echo "  ⚔️  Battle Chain:    $BATTLE_APP"
echo "  📊 Registry:        $REGISTRY_APP"
echo "  🎲 Prediction:      $PREDICTION_APP"
echo "  🤝 Matchmaking:     $MATCHMAKING_APP"
echo ""
echo -e "${YELLOW}Important Note:${NC}"
echo "  Matchmaking includes battle app in required dependencies."
echo "  When battles are created, the battle application will be"
echo "  AUTOMATICALLY DEPLOYED to new battle chains! 🚀"
echo ""
echo "Next Steps:"
echo "  1. Source .env file: source .env"
echo "  2. Create characters: scripts/create-character.sh"
echo "  3. Join matchmaking queue"
echo "  4. Battle application auto-deploys when battle starts! ⚔️"
echo ""
