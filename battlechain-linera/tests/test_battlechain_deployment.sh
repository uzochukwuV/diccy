#!/bin/bash
# shellcheck disable=SC2181
# shellcheck disable=SC2145

# Battlechain Integration Test - Full Deployment and Testing
# This script deploys all battlechain contracts and tests a complete battle flow

# Check if three values are provided
if [ "$#" -ne 3 ]; then
  echo "Usage: $0 <FAUCET_URL> <GRAPHQL_URL> <LOCAL_NETWORK_URL>"
  exit 1
fi

start=$(date +%s%3N)

FAUCET_URL=$1
GRAPHQL_URL=$2
LOCAL_NETWORK_URL=$3

LINERA_TMP_DIR=~/.config/linera
TOKEN_INITIAL_SUPPLY=1000000000000  # 1 trillion BATTLE tokens
LINERA_SERVICE_PORT=8081

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# ----------------------------------------------------------
# Clear current wallet
# ----------------------------------------------------------
echo -e "${YELLOW}Cleaning up existing wallet...${NC}"
rm -rf "$LINERA_TMP_DIR"
mkdir -p "$LINERA_TMP_DIR"

# ----------------------------------------------------------
# [FUNCTION] Initiate New Wallet from Faucet
# ----------------------------------------------------------
initiate_new_wallet_from_faucet() {
  linera wallet init --faucet "$FAUCET_URL"
  if [ $? -ne 0 ]; then
      echo -e "${RED}Initiate New Wallet from Faucet failed. Exiting...${NC}"
      exit 1
  fi
}

# ----------------------------------------------------------
# [FUNCTION] Open Chain from Faucet
# ----------------------------------------------------------
open_chain_from_faucet() {
  linera wallet request-chain --faucet "$FAUCET_URL"
  if [ $? -ne 0 ]; then
      echo -e "${RED}Open Chain from Faucet failed. Exiting...${NC}"
      exit 1
  fi
}

# ----------------------------------------------------------
# Initialize Wallet and Create Chains
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 1: Initializing Wallet and Creating Chains ===${NC}"

INITIATE_WALLET=$(initiate_new_wallet_from_faucet)

# Create default chain (for contract deployments)
OPEN_DEFAULT_CHAIN=$(open_chain_from_faucet)
mapfile -t StringArray <<< "$OPEN_DEFAULT_CHAIN"
DEFAULT_CHAIN_ID=${StringArray[0]}

echo -e "${GREEN}Default Chain: $DEFAULT_CHAIN_ID${NC}"

# Create player chains
OPEN_PLAYER1_CHAIN=$(open_chain_from_faucet)
mapfile -t StringArray <<< "$OPEN_PLAYER1_CHAIN"
PLAYER1_CHAIN_ID=${StringArray[0]}

OPEN_PLAYER2_CHAIN=$(open_chain_from_faucet)
mapfile -t StringArray <<< "$OPEN_PLAYER2_CHAIN"
PLAYER2_CHAIN_ID=${StringArray[0]}

OPEN_PLAYER3_CHAIN=$(open_chain_from_faucet)
mapfile -t StringArray <<< "$OPEN_PLAYER3_CHAIN"
PLAYER3_CHAIN_ID=${StringArray[0]}

OPEN_PLAYER4_CHAIN=$(open_chain_from_faucet)
mapfile -t StringArray <<< "$OPEN_PLAYER4_CHAIN"
PLAYER4_CHAIN_ID=${StringArray[0]}

echo -e "${GREEN}Player Chains Created:${NC}"
echo "  Player 1: $PLAYER1_CHAIN_ID"
echo "  Player 2: $PLAYER2_CHAIN_ID"
echo "  Player 3: $PLAYER3_CHAIN_ID"
echo "  Player 4: $PLAYER4_CHAIN_ID"

linera sync && linera query-balance

# ----------------------------------------------------------
# Deploy Battle Token Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 2: Deploying Battle Token Contract ===${NC}"

deploy_battle_token() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera battle-token \
  --json-argument "\"$TOKEN_INITIAL_SUPPLY\""
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Battle Token failed. Exiting...${NC}"
      exit 1
  fi
}

BATTLE_TOKEN_APP_ID=$(deploy_battle_token)
echo -e "${GREEN}Battle Token App ID: $BATTLE_TOKEN_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Deploy Registry Chain Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 3: Deploying Registry Chain Contract ===${NC}"

deploy_registry_chain() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera registry-chain
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Registry Chain failed. Exiting...${NC}"
      exit 1
  fi
}

REGISTRY_APP_ID=$(deploy_registry_chain)
echo -e "${GREEN}Registry Chain App ID: $REGISTRY_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Deploy Player Chain Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 4: Deploying Player Chain Contract ===${NC}"

deploy_player_chain() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera player-chain
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Player Chain failed. Exiting...${NC}"
      exit 1
  fi
}

PLAYER_CHAIN_APP_ID=$(deploy_player_chain)
echo -e "${GREEN}Player Chain App ID: $PLAYER_CHAIN_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Deploy Prediction Chain Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 5: Deploying Prediction Chain Contract ===${NC}"

deploy_prediction_chain() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera prediction-chain \
  --required-application-ids "$BATTLE_TOKEN_APP_ID"
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Prediction Chain failed. Exiting...${NC}"
      exit 1
  fi
}

PREDICTION_APP_ID=$(deploy_prediction_chain)
echo -e "${GREEN}Prediction Chain App ID: $PREDICTION_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Deploy Matchmaking Chain Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 6: Deploying Matchmaking Chain Contract ===${NC}"

deploy_matchmaking_chain() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera matchmaking-chain \
  --required-application-ids "$PREDICTION_APP_ID"
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Matchmaking Chain failed. Exiting...${NC}"
      exit 1
  fi
}

MATCHMAKING_APP_ID=$(deploy_matchmaking_chain)
echo -e "${GREEN}Matchmaking Chain App ID: $MATCHMAKING_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Deploy Battle Chain Contract
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 7: Deploying Battle Chain Contract ===${NC}"

deploy_battle_chain() {
  linera --wait-for-outgoing-messages project publish-and-create battlechain-linera battle-chain \
  --required-application-ids "$BATTLE_TOKEN_APP_ID"
  if [ $? -ne 0 ]; then
      echo -e "${RED}Deploy Battle Chain failed. Exiting...${NC}"
      exit 1
  fi
}

BATTLE_CHAIN_APP_ID=$(deploy_battle_chain)
echo -e "${GREEN}Battle Chain App ID: $BATTLE_CHAIN_APP_ID${NC}"
sleep 3

# ----------------------------------------------------------
# Start Node Service
# ----------------------------------------------------------
echo -e "${GREEN}=== STEP 8: Starting Node Service ===${NC}"

linera service --port $LINERA_SERVICE_PORT &
SERVICE_PID=$!
sleep 3
echo -e "${GREEN}Node service started with PID $SERVICE_PID${NC}"
sleep 2

# ----------------------------------------------------------
# Test Token Distribution
# ----------------------------------------------------------
echo -e "${GREEN}=== TEST 1: Token Distribution ===${NC}"

# Transfer tokens to players
for i in 1 2 3 4; do
  PLAYER_VAR="PLAYER${i}_CHAIN_ID"
  PLAYER_CHAIN="${!PLAYER_VAR}"

  echo "Transferring tokens to Player $i ($PLAYER_CHAIN)..."

  MUTATION="mutation { transfer(to: \\\"$PLAYER_CHAIN\\\", amount: \\\"10000\\\") }"

  curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN_ID/applications/$BATTLE_TOKEN_APP_ID" \
    -H "Content-Type: application/json" \
    -d "{\"query\":\"$MUTATION\"}" \
    | jq .

  sleep 2
done

# ----------------------------------------------------------
# Test Character Creation
# ----------------------------------------------------------
echo -e "${GREEN}=== TEST 2: Character Creation ===${NC}"

# Create character for Player 1
MUTATION="mutation { createCharacter(characterId: \\\"warrior_001\\\", nftId: \\\"nft_001\\\", class: \\\"Warrior\\\") }"

curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER1_CHAIN_ID/applications/$PLAYER_CHAIN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}" \
  | jq .

sleep 2

# Create character for Player 2
MUTATION="mutation { createCharacter(characterId: \\\"mage_001\\\", nftId: \\\"nft_002\\\", class: \\\"Mage\\\") }"

curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER2_CHAIN_ID/applications/$PLAYER_CHAIN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}" \
  | jq .

sleep 2

# ----------------------------------------------------------
# Test Matchmaking
# ----------------------------------------------------------
echo -e "${GREEN}=== TEST 3: Matchmaking ===${NC}"

# Player 1 joins queue
echo "Player 1 joining matchmaking queue..."
MUTATION="mutation { joinQueue(characterId: \\\"warrior_001\\\", stake: \\\"100\\\") }"

curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER1_CHAIN_ID/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}" \
  | jq .

sleep 2

# Player 2 joins queue
echo "Player 2 joining matchmaking queue..."
MUTATION="mutation { joinQueue(characterId: \\\"mage_001\\\", stake: \\\"100\\\") }"

curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER2_CHAIN_ID/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}" \
  | jq .

sleep 3

# ----------------------------------------------------------
# Test Prediction Market
# ----------------------------------------------------------
echo -e "${GREEN}=== TEST 4: Prediction Market ===${NC}"

# Query if match was created
echo "Querying matchmaking status..."
QUERY="query { queueSize totalMatches }"

curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN_ID/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}" \
  | jq .

sleep 2

# ----------------------------------------------------------
# Stop Node Service
# ----------------------------------------------------------
echo -e "${YELLOW}Stopping service...${NC}"
kill $SERVICE_PID

# ----------------------------------------------------------
# Summary
# ----------------------------------------------------------
echo ""
echo -e "${GREEN}============================================${NC}"
echo -e "${GREEN}  BATTLECHAIN DEPLOYMENT SUMMARY${NC}"
echo -e "${GREEN}============================================${NC}"
echo ""
echo "Default Chain: $DEFAULT_CHAIN_ID"
echo ""
echo "Application IDs:"
echo "  Battle Token:  $BATTLE_TOKEN_APP_ID"
echo "  Registry:      $REGISTRY_APP_ID"
echo "  Player Chain:  $PLAYER_CHAIN_APP_ID"
echo "  Prediction:    $PREDICTION_APP_ID"
echo "  Matchmaking:   $MATCHMAKING_APP_ID"
echo "  Battle Chain:  $BATTLE_CHAIN_APP_ID"
echo ""
echo "Player Chains:"
echo "  Player 1: $PLAYER1_CHAIN_ID"
echo "  Player 2: $PLAYER2_CHAIN_ID"
echo "  Player 3: $PLAYER3_CHAIN_ID"
echo "  Player 4: $PLAYER4_CHAIN_ID"
echo ""
echo "Access URLs:"
echo "  Battle Token:  $LOCAL_NETWORK_URL/chains/$DEFAULT_CHAIN_ID/applications/$BATTLE_TOKEN_APP_ID"
echo "  Registry:      $LOCAL_NETWORK_URL/chains/$DEFAULT_CHAIN_ID/applications/$REGISTRY_APP_ID"
echo "  Matchmaking:   $LOCAL_NETWORK_URL/chains/$DEFAULT_CHAIN_ID/applications/$MATCHMAKING_APP_ID"
echo ""

end=$(date +%s%3N)
total_ms=$(( end - start ))
ms=$(( total_ms % 1000 ))
seconds=$(( total_ms / 1000 ))
printf "Total Runtime: %d seconds and %d ms\n" $seconds $ms
echo ""
