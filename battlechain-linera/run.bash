#!/usr/bin/env bash

set -e

echo "🚀 Starting BattleChain Linera Network..."

# Clean up any existing network data
if [ -d "/tmp/linera" ]; then
    echo "🧹 Cleaning up existing network data..."
    rm -rf /tmp/linera
fi

# Create storage directory
mkdir -p /tmp/linera

# Start local Linera network with faucet
echo "🌐 Starting Linera network with faucet..."
linera net up \
    --storage-path /tmp/linera \
    --faucet-port 19100 \
    --testing-prng-seed 37 \
    --num-shards 4

echo "✅ Linera network started successfully!"
echo ""
echo "📋 Network Information:"
echo "   - Web Service: http://localhost:9000"
echo "   - Faucet: http://localhost:19100"
echo "   - Shards: http://localhost:19000-19003"
echo ""
echo "🎮 BattleChain contracts are ready to deploy!"
echo ""
echo "To deploy contracts, run:"
echo "  cd /app"
echo "  linera project publish"
