FROM rust:1.86-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    git \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install Linera CLI tools
RUN cargo install --git https://github.com/linera-io/linera-protocol.git \
    --tag v0.15.5 \
    --locked \
    linera-service

# Set working directory
WORKDIR /app

# Copy workspace configuration
COPY Cargo.toml ./

# Copy all contract sources
COPY shared-types/ ./shared-types/
COPY battle-chain/ ./battle-chain/
COPY battle-token/ ./battle-token/
COPY matchmaking-chain/ ./matchmaking-chain/
COPY player-chain/ ./player-chain/
COPY prediction-chain/ ./prediction-chain/
COPY registry-chain/ ./registry-chain/

# Build all contracts
RUN cargo build --release --target wasm32-unknown-unknown

# Create directory for linera data
RUN mkdir -p /root/.config/linera

# Expose ports
# 9000: Default Linera web service port
# 19100: Default faucet port
# 19000-19099: Shard ports
EXPOSE 9000 19100 19000-19099

# Default command
CMD ["linera", "--help"]
