#!/bin/bash
# BattleChain Comprehensive Build Script
# Based on microcard's build approach
# Reference: CODE_REORGANIZATION_PLAN.md

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Print header
print_header() {
    echo -e "${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
    echo -e "${GREEN}║          BattleChain Build Script v1.0                ║${NC}"
    echo -e "${GREEN}║  PvP Fighting Game on Linera Blockchain               ║${NC}"
    echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}"
    echo ""
}

# Print section header
print_section() {
    echo -e "\n${CYAN}═══════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════${NC}\n"
}

# Print step
print_step() {
    echo -e "${YELLOW}[$1/$2]${NC} $3"
}

# Print success
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

# Print error
print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Print info
print_info() {
    echo -e "${BLUE}ℹ${NC} $1"
}

# Error handler
error_exit() {
    print_error "$1"
    exit 1
}

print_header

# Change to project root
cd "$(dirname "$0")/.."
PROJECT_ROOT=$(pwd)

print_info "Project root: $PROJECT_ROOT"

# ============================================================================
# STEP 1: Check Prerequisites
# ============================================================================
print_section "STEP 1: Checking Prerequisites"

print_step "1" "7" "Checking Rust installation..."
if ! command -v rustc &> /dev/null; then
    error_exit "Rust not found. Install from https://rustup.rs"
fi
RUST_VERSION=$(rustc --version)
print_success "Rust installed: $RUST_VERSION"

print_step "2" "7" "Checking cargo installation..."
if ! command -v cargo &> /dev/null; then
    error_exit "Cargo not found"
fi
CARGO_VERSION=$(cargo --version)
print_success "Cargo installed: $CARGO_VERSION"

print_step "3" "7" "Checking wasm32-unknown-unknown target..."
if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
    print_info "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown || error_exit "Failed to install wasm32 target"
fi
print_success "wasm32-unknown-unknown target installed"

print_step "4" "7" "Checking Linera CLI..."
if ! command -v linera &> /dev/null; then
    print_error "Linera CLI not found"
    echo -e "${YELLOW}To install Linera CLI:${NC}"
    echo "  cargo install --locked linera-service@0.15.5"
    echo "  cargo install --locked linera-storage-service@0.15.5"
    error_exit "Please install Linera CLI first"
fi
LINERA_VERSION=$(linera --version 2>&1 || echo "unknown")
print_success "Linera CLI installed: $LINERA_VERSION"

print_step "5" "7" "Checking protoc (Protocol Buffers compiler)..."
if ! command -v protoc &> /dev/null; then
    print_error "protoc not found"
    echo -e "${YELLOW}To install protoc:${NC}"
    echo "  Ubuntu/Debian: sudo apt-get install protobuf-compiler"
    echo "  macOS: brew install protobuf"
    error_exit "Please install protoc first"
fi
PROTOC_VERSION=$(protoc --version)
print_success "protoc installed: $PROTOC_VERSION"

print_step "6" "7" "Checking git..."
if ! command -v git &> /dev/null; then
    error_exit "git not found"
fi
print_success "git installed"

print_step "7" "7" "Checking workspace structure..."
CHAINS=("shared-events" "shared-types" "battle-token" "player-chain" "battle-chain" "matchmaking-chain" "prediction-chain" "registry-chain")
for chain in "${CHAINS[@]}"; do
    if [ ! -d "$PROJECT_ROOT/$chain" ]; then
        error_exit "Chain directory not found: $chain"
    fi
done
print_success "All chain directories present"

# ============================================================================
# STEP 2: Clean Previous Builds
# ============================================================================
print_section "STEP 2: Cleaning Previous Builds"

print_info "Removing target directory..."
if [ -d "$PROJECT_ROOT/target" ]; then
    rm -rf "$PROJECT_ROOT/target"
    print_success "Cleaned target directory"
else
    print_info "No target directory to clean"
fi

# ============================================================================
# STEP 3: Format Code
# ============================================================================
print_section "STEP 3: Formatting Code"

print_info "Running cargo fmt..."
cargo fmt --all || print_error "cargo fmt had issues (non-fatal)"
print_success "Code formatting complete"

# ============================================================================
# STEP 4: Run Clippy (Linting)
# ============================================================================
print_section "STEP 4: Running Clippy (Linting)"

print_info "Running clippy on all targets..."
if cargo clippy --all-targets --all-features --target wasm32-unknown-unknown -- -D warnings 2>&1 | tee /tmp/clippy.log; then
    print_success "Clippy passed with no warnings"
else
    print_error "Clippy found issues"
    echo -e "${YELLOW}Review clippy output above. Continuing build anyway...${NC}"
    # Don't exit - allow build to continue
fi

# ============================================================================
# STEP 5: Build All Chains
# ============================================================================
print_section "STEP 5: Building All Chains for WASM"

print_info "Building in dependency order..."
echo ""

# Build order: shared crates first, then applications
BUILD_ORDER=("shared-types" "shared-events" "battle-token" "player-chain" "battle-chain" "matchmaking-chain" "prediction-chain" "registry-chain")
TOTAL=${#BUILD_ORDER[@]}
CURRENT=0

for chain in "${BUILD_ORDER[@]}"; do
    CURRENT=$((CURRENT + 1))
    print_step "$CURRENT" "$TOTAL" "Building $chain..."

    if cargo build -p battlechain-$chain --release --target wasm32-unknown-unknown 2>&1 | tee "/tmp/build-$chain.log"; then
        print_success "$chain built successfully"
    else
        print_error "$chain build failed"
        echo -e "${YELLOW}Check log: /tmp/build-$chain.log${NC}"
        error_exit "Build failed for $chain"
    fi
    echo ""
done

# ============================================================================
# STEP 6: Verify WASM Artifacts
# ============================================================================
print_section "STEP 6: Verifying WASM Artifacts"

print_info "Searching for WASM files..."
WASM_FILES=$(find "$PROJECT_ROOT" -name "*.wasm" -path "*/target/wasm32-unknown-unknown/release/*" ! -path "*/deps/*")
WASM_COUNT=$(echo "$WASM_FILES" | grep -c ".wasm" || echo "0")

if [ "$WASM_COUNT" -eq 0 ]; then
    error_exit "No WASM artifacts found!"
fi

print_success "Found $WASM_COUNT WASM artifacts"
echo ""

print_info "WASM Artifacts:"
echo "$WASM_FILES" | while read -r file; do
    if [ -n "$file" ]; then
        SIZE=$(ls -lh "$file" | awk '{print $5}')
        BASENAME=$(basename "$file")
        echo -e "  ${GREEN}•${NC} $BASENAME ${BLUE}($SIZE)${NC}"
    fi
done

# ============================================================================
# STEP 7: Build Summary
# ============================================================================
print_section "Build Summary"

# Count lines of code
print_info "Counting lines of code..."
TOTAL_LINES=$(find "$PROJECT_ROOT" -name "*.rs" ! -path "*/target/*" -exec wc -l {} + | tail -1 | awk '{print $1}')
print_info "Total Rust code: $TOTAL_LINES lines"

# Show build time
print_info "Build completed successfully!"

echo -e "\n${GREEN}╔════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║              🚀 Build Complete! 🚀                     ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════╝${NC}\n"

print_info "Next steps:"
echo "  1. Run tests: ./scripts/test.sh"
echo "  2. Deploy locally: ./scripts/deploy-local.sh"
echo "  3. View WASM files: find . -name '*.wasm' -path '*/release/*' ! -path '*/deps/*'"
echo ""

print_success "All chains built successfully!"
