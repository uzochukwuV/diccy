#!/bin/bash
# shellcheck disable=SC2181

# Complete Battle Flow Test
# Tests a full battle from matchmaking to completion

if [ "$#" -ne 6 ]; then
  echo "Usage: $0 <GRAPHQL_URL> <DEFAULT_CHAIN> <PLAYER1_CHAIN> <PLAYER2_CHAIN> <MATCHMAKING_APP_ID> <BATTLE_CHAIN_APP_ID>"
  exit 1
fi

GRAPHQL_URL=$1
DEFAULT_CHAIN=$2
PLAYER1_CHAIN=$3
PLAYER2_CHAIN=$4
MATCHMAKING_APP_ID=$5
BATTLE_CHAIN_APP_ID=$6

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== Battle Flow Test ===${NC}"
echo "Player 1: $PLAYER1_CHAIN"
echo "Player 2: $PLAYER2_CHAIN"
echo ""

# ----------------------------------------------------------
# Step 1: Players join matchmaking queue
# ----------------------------------------------------------
echo -e "${YELLOW}Step 1: Players joining matchmaking queue${NC}"

# Player 1 joins
MUTATION="mutation { joinQueue(characterId: \\\"warrior_001\\\", stake: \\\"100\\\") }"
curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER1_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}"
echo "Player 1 joined queue"
sleep 1

# Player 2 joins
MUTATION="mutation { joinQueue(characterId: \\\"mage_001\\\", stake: \\\"100\\\") }"
curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER2_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}"
echo "Player 2 joined queue"
sleep 2

# ----------------------------------------------------------
# Step 2: Query match creation
# ----------------------------------------------------------
echo -e "${YELLOW}Step 2: Checking if match was created${NC}"

QUERY="query { totalMatches pendingBattles }"
RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$MATCHMAKING_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}")
echo "$RESULT" | jq .
sleep 1

# ----------------------------------------------------------
# Step 3: Submit turns for round 1
# ----------------------------------------------------------
echo -e "${YELLOW}Step 3: Submitting turns for round 1${NC}"

# Player 1 submits turns
for turn in 0 1 2; do
  MUTATION="mutation { submitTurn(round: 0, turn: $turn, stance: \\\"Offensive\\\", useSpecial: false) }"
  curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER1_CHAIN/applications/$BATTLE_CHAIN_APP_ID" \
    -H "Content-Type: application/json" \
    -d "{\"query\":\"$MUTATION\"}"
  echo "Player 1 submitted turn $turn"
  sleep 1
done

# Player 2 submits turns
for turn in 0 1 2; do
  MUTATION="mutation { submitTurn(round: 0, turn: $turn, stance: \\\"Defensive\\\", useSpecial: false) }"
  curl -s -X POST "$GRAPHQL_URL/chains/$PLAYER2_CHAIN/applications/$BATTLE_CHAIN_APP_ID" \
    -H "Content-Type: application/json" \
    -d "{\"query\":\"$MUTATION\"}"
  echo "Player 2 submitted turn $turn"
  sleep 1
done

# ----------------------------------------------------------
# Step 4: Execute round
# ----------------------------------------------------------
echo -e "${YELLOW}Step 4: Executing round${NC}"

MUTATION="mutation { executeRound }"
curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$BATTLE_CHAIN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}"
sleep 2

# ----------------------------------------------------------
# Step 5: Query battle status
# ----------------------------------------------------------
echo -e "${YELLOW}Step 5: Querying battle status${NC}"

QUERY="query { battleStatus currentRound player1Hp player2Hp }"
RESULT=$(curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$BATTLE_CHAIN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$QUERY\"}")
echo "$RESULT" | jq .

# ----------------------------------------------------------
# Step 6: Continue battle until completion
# ----------------------------------------------------------
echo -e "${YELLOW}Step 6: Continuing battle (simplified)${NC}"

# In a real test, you would loop through rounds until one player is defeated
# For now, we'll just simulate finalization

MUTATION="mutation { finalizeBattle }"
curl -s -X POST "$GRAPHQL_URL/chains/$DEFAULT_CHAIN/applications/$BATTLE_CHAIN_APP_ID" \
  -H "Content-Type: application/json" \
  -d "{\"query\":\"$MUTATION\"}"

echo ""
echo -e "${GREEN}Battle flow test completed!${NC}"
