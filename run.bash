#!/usr/bin/env bash
export PATH="$HOME/.cargo/bin:$PATH"
set -eu

# Start Linera network
eval "$(linera net helper)"
export LINERA_FAUCET_URL=http://localhost:8080

# Setup wallet
rm -rf /tmp/.linera*
export LINERA_WALLET="/tmp/majorules_wallet.json"
export LINERA_KEYSTORE="/tmp/majorules_keystore.json"
export LINERA_STORAGE="memory"

linera wallet init --faucet="$LINERA_FAUCET_URL"
linera wallet request-chain --faucet="$LINERA_FAUCET_URL"

# Get lobby owner
LOBBY_OWNER=$(linera wallet show | grep "Public Key" | head -1 | awk '{print $3}')

# Build WASM
cargo build --target wasm32-unknown-unknown --release

# Deploy Majorules lobby
linera publish-and-create \
  target/wasm32-unknown-unknown/release/majorules_{contract,service}.wasm \
  --json-argument "{\"entry_fee\": \"1000000000000000000\", \"lobby_owner\": \"$LOBBY_OWNER\", \"is_lobby\": true}"

# Start GraphQL service
echo "🎯 Majorules Lobby deployed successfully!"
echo "🌐 GraphQL service starting on http://localhost:5173"
linera service --port 5173