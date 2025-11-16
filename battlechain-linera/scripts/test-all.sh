#!/bin/bash
set -e

echo "🧪 Running BattleChain Test Suite..."
echo "====================================="

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

test_chain() {
    local chain=$1
    echo -e "${BLUE}Testing $chain...${NC}"
    cd "$chain"

    if cargo test --quiet 2>&1; then
        echo -e "${GREEN}✓ $chain tests passed${NC}"
        cd ..
        return 0
    else
        echo -e "${RED}✗ $chain tests failed${NC}"
        cd ..
        return 1
    fi
}

# Navigate to battlechain-linera directory
cd "$(dirname "$0")/.."

FAILED=0

# Test all chains
test_chain "shared-types" || FAILED=1
test_chain "battle-token" || FAILED=1
test_chain "player-chain" || FAILED=1
test_chain "battle-chain" || FAILED=1
test_chain "matchmaking-chain" || FAILED=1
test_chain "prediction-chain" || FAILED=1
test_chain "registry-chain" || FAILED=1

echo ""
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}====================================="
    echo -e "✓ All tests passed!"
    echo -e "=====================================${NC}"
    exit 0
else
    echo -e "${RED}====================================="
    echo -e "✗ Some tests failed"
    echo -e "=====================================${NC}"
    exit 1
fi
