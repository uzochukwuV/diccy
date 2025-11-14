#!/bin/bash
set -e

echo "🎮 Building BattleChain Microchains..."
echo "======================================"

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

build_chain() {
    local chain=$1
    echo -e "${BLUE}Building $chain...${NC}"
    cd "$chain"
    cargo build --target wasm32-unknown-unknown --release
    cd ..
    echo -e "${GREEN}✓ $chain built successfully${NC}"
    echo ""
}

# Navigate to battlechain-linera directory
cd "$(dirname "$0")/.."

# Build all chains
build_chain "shared-types"
build_chain "battle-token"
build_chain "player-chain"
build_chain "battle-chain"
build_chain "matchmaking-chain"
build_chain "prediction-chain"
build_chain "registry-chain"

echo -e "${GREEN}======================================"
echo -e "✓ All chains built successfully!"
echo -e "======================================${NC}"

# Show WASM artifacts
echo ""
echo "WASM Artifacts:"
find . -name "*.wasm" -path "*/target/wasm32-unknown-unknown/release/*" ! -path "*/deps/*" | while read -r file; do
    size=$(du -h "$file" | cut -f1)
    echo "  - $(basename $(dirname $(dirname $(dirname "$file")))): $file ($size)"
done
