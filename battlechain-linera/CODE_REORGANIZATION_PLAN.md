# BattleChain Code Reorganization Plan

**Date:** 2025-11-19
**Reference:** Microcard architecture
**Goal:** Clean, maintainable, testable codebase

---

## Current Issues Analysis

### 1. Code Duplication ❌

**BattleEvent** duplicated in 3 files:
- `battle-chain/src/lib.rs` (lines 44-76) - Full definition
- `prediction-chain/src/lib.rs` (lines 14-43) - Partial copy
- `registry-chain/src/lib.rs` (lines 13-43) - Partial copy

**BattleParticipant** duplicated in 2 files:
- `battle-chain/src/lib.rs` (lines 119-198) - Full implementation
- `matchmaking-chain/src/lib.rs` (lines 17-51) - Partial copy

**Impact:**
- Changes require updates in multiple files
- Risk of inconsistent definitions
- Harder to maintain and test

### 2. Missing Shared Infrastructure ❌

Microcard has:
```
microcard/
├── abi/            # Shared types and logic
│   ├── blackjack.rs
│   ├── deck.rs
│   ├── player_dealer.rs
│   └── random.rs
```

BattleChain has:
```
battlechain-linera/
├── shared-types/   # Only character types
│   └── lib.rs      # Missing events, messages, errors
```

**Missing:**
- Shared event types
- Shared message types
- Shared error types
- Shared utility functions

### 3. Inconsistent Error Handling ❌

**Pattern 1: Panic with message**
```rust
// battle-chain/src/lib.rs
panic!("Battle already initialized");
```

**Pattern 2: Silent failure**
```rust
// battle-token/src/lib.rs
Err(_e) => {}  // ❌ ERROR SWALLOWED
```

**Pattern 3: Result type**
```rust
// player-chain/src/lib.rs
Err(PlayerChainError::InsufficientBalance { ... })
```

**Should be:** Consistent Result<T, E> with proper error propagation

### 4. No Build/Test Infrastructure ❌

Microcard has:
- `run.bash` (311 lines) - Full deployment automation
- `Dockerfile` - Containerized builds
- `Makefile` - Build shortcuts
- Working tests

BattleChain has:
- `run.bash` (874 bytes) - Stub only
- `scripts/build-all.sh` - Basic build script
- No deployment automation
- No test infrastructure

---

## Reorganization Plan

### Phase 1: Create Shared Infrastructure (Priority 1)

#### 1.1 Create `shared-events` Crate

**Purpose:** Centralize all cross-chain event definitions

```rust
// shared-events/Cargo.toml
[package]
name = "battlechain-shared-events"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
linera-sdk = { workspace = true }
battlechain-shared-types = { path = "../shared-types" }

// shared-events/src/lib.rs
use serde::{Deserialize, Serialize};
use linera_sdk::linera_base_types::*;
use battlechain_shared_types::Owner;

/// Battle events emitted by battle-chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BattleEvent {
    BattleStarted {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
        total_stake: Amount,
    },
    BattleCompleted {
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
        winner_chain: ChainId,
        loser_chain: ChainId,
        stake: Amount,
        rounds_played: u8,
        player1_stats: CombatStats,
        player2_stats: CombatStats,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatStats {
    pub damage_dealt: u64,
    pub damage_taken: u64,
    pub crits: u64,
    pub dodges: u64,
    pub highest_crit: u64,
}
```

**Files to update:**
- Remove BattleEvent from `prediction-chain/src/lib.rs`
- Remove BattleEvent from `registry-chain/src/lib.rs`
- Update `battle-chain/src/lib.rs` to export from shared-events
- Update all Cargo.toml files to depend on shared-events

#### 1.2 Extend `shared-types` Crate

**Add to existing shared-types:**

```rust
// shared-types/src/battle_participant.rs
use crate::{CharacterSnapshot, Owner};
use serde::{Deserialize, Serialize};
use linera_sdk::linera_base_types::{Amount, ChainId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParticipant {
    pub owner: Owner,
    pub chain: ChainId,
    pub character: CharacterSnapshot,
    pub stake: Amount,
    pub current_hp: u32,
    pub combo_stack: u8,
    pub special_cooldown: u8,
    pub turns_submitted: [Option<TurnSubmission>; 3],
}

impl BattleParticipant {
    pub fn new(owner: Owner, chain: ChainId, character: CharacterSnapshot, stake: Amount) -> Self {
        let current_hp = character.hp_max;
        Self {
            owner,
            chain,
            character,
            stake,
            current_hp,
            combo_stack: 0,
            special_cooldown: 0,
            turns_submitted: [None, None, None],
        }
    }

    pub fn reset_turns(&mut self) {
        self.turns_submitted = [None, None, None];
    }

    pub fn all_turns_submitted(&self) -> bool {
        self.turns_submitted[0].is_some()
            && self.turns_submitted[1].is_some()
            && self.turns_submitted[2].is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnSubmission {
    pub round: u8,
    pub turn: u8,
    pub stance: Stance,
    pub use_special: bool,
}
```

**Update shared-types/src/lib.rs:**
```rust
pub mod bet_chip_profile;  // If needed
mod battle_participant;

pub use battle_participant::{BattleParticipant, TurnSubmission};
// ... existing exports
```

#### 1.3 Create `shared-errors` Crate (Optional but Recommended)

```rust
// shared-errors/src/lib.rs
use thiserror::Error;
use linera_sdk::linera_base_types::Amount;

#[derive(Debug, Error)]
pub enum BattleChainError {
    #[error("Insufficient balance: have {available}, need {required}")]
    InsufficientBalance { available: Amount, required: Amount },

    #[error("Battle not found")]
    BattleNotFound,

    #[error("Invalid stake: {0}")]
    InvalidStake(String),

    #[error("View error: {0}")]
    ViewError(String),
}
```

### Phase 2: Standardize Error Handling (Priority 1)

#### 2.1 Define Error Handling Policy

**Rule 1: Never swallow errors**
```rust
// ❌ BEFORE
Err(_e) => {}

// ✅ AFTER
Err(e) => {
    log::error!("Operation failed: {:?}", e);
    panic!("Transfer failed: {:?}", e);
}
```

**Rule 2: Use Result<T, E> for recoverable errors**
```rust
// ✅ Good
pub async fn transfer(&mut self, ...) -> Result<(), TokenError> {
    if amount == Amount::ZERO {
        return Err(TokenError::ZeroAmount);
    }
    // ...
}
```

**Rule 3: Panic for invalid state**
```rust
// ✅ Good
if self.state.player1.get().is_some() {
    panic!("Battle already initialized");
}
```

#### 2.2 Add Logging Everywhere

**Install log dependency (already in workspace):**
```toml
[dependencies]
log = { workspace = true }
```

**Pattern to follow:**
```rust
pub async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
    log::info!("Executing operation: {:?}", operation);

    match operation {
        Operation::Transfer { to, amount } => {
            log::debug!("Transfer: caller={:?}, to={:?}, amount={}", caller, to, amount);

            match self.state.transfer(caller, to, amount, now).await {
                Ok(_) => {
                    log::info!("✓ Transfer successful");
                }
                Err(e) => {
                    log::error!("✗ Transfer failed: {:?}", e);
                    panic!("Transfer failed: {:?}", e);
                }
            }
        }
        // ... other operations
    }
}
```

### Phase 3: Build Infrastructure (Priority 2)

#### 3.1 Create Comprehensive Build Script

**Reference:** `microcard/run.bash` (311 lines)

**Create:** `battlechain-linera/scripts/build.sh`

```bash
#!/bin/bash
set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${GREEN}╔════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   BattleChain Build Script             ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"

# Step 1: Check prerequisites
echo -e "\n${YELLOW}[1/5] Checking prerequisites...${NC}"

# Check Rust
if ! command -v rustc &> /dev/null; then
    echo -e "${RED}✗ Rust not found. Install from https://rustup.rs${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Rust version: $(rustc --version)${NC}"

# Check wasm32 target
if ! rustup target list | grep -q "wasm32-unknown-unknown (installed)"; then
    echo -e "${YELLOW}Installing wasm32-unknown-unknown target...${NC}"
    rustup target add wasm32-unknown-unknown
fi
echo -e "${GREEN}✓ wasm32-unknown-unknown target installed${NC}"

# Check Linera
if ! command -v linera &> /dev/null; then
    echo -e "${RED}✗ Linera CLI not found. Install: cargo install linera-service@0.15.5${NC}"
    exit 1
fi
echo -e "${GREEN}✓ Linera version: $(linera --version)${NC}"

# Step 2: Clean previous builds
echo -e "\n${YELLOW}[2/5] Cleaning previous builds...${NC}"
cargo clean
echo -e "${GREEN}✓ Clean complete${NC}"

# Step 3: Format code
echo -e "\n${YELLOW}[3/5] Formatting code...${NC}"
cargo fmt --all
echo -e "${GREEN}✓ Format complete${NC}"

# Step 4: Run clippy
echo -e "\n${YELLOW}[4/5] Running clippy...${NC}"
cargo clippy --all-targets --all-features --target wasm32-unknown-unknown -- -D warnings || {
    echo -e "${RED}✗ Clippy warnings found. Fix them before building.${NC}"
    exit 1
}
echo -e "${GREEN}✓ Clippy passed${NC}"

# Step 5: Build all chains
echo -e "\n${YELLOW}[5/5] Building all chains for WASM...${NC}"

CHAINS=("shared-types" "shared-events" "battle-token" "player-chain" "battle-chain" "matchmaking-chain" "prediction-chain" "registry-chain")

for chain in "${CHAINS[@]}"; do
    echo -e "${BLUE}  Building $chain...${NC}"
    cargo build -p battlechain-$chain --release --target wasm32-unknown-unknown
    if [ $? -eq 0 ]; then
        echo -e "${GREEN}  ✓ $chain built successfully${NC}"
    else
        echo -e "${RED}  ✗ $chain build failed${NC}"
        exit 1
    fi
done

# Step 6: Verify WASM artifacts
echo -e "\n${YELLOW}Verifying WASM artifacts...${NC}"
WASM_COUNT=$(find . -name "*.wasm" -path "*/target/wasm32-unknown-unknown/release/*" ! -path "*/deps/*" | wc -l)
echo -e "${GREEN}✓ Found $WASM_COUNT WASM artifacts${NC}"

# List artifacts
echo -e "\n${BLUE}WASM Artifacts:${NC}"
find . -name "*.wasm" -path "*/target/wasm32-unknown-unknown/release/*" ! -path "*/deps/*" -exec ls -lh {} \;

echo -e "\n${GREEN}╔════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║   Build Complete! 🚀                   ║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════╝${NC}"
```

#### 3.2 Create Test Script

**Create:** `battlechain-linera/scripts/test.sh`

```bash
#!/bin/bash
set -e

echo "╔════════════════════════════════════════╗"
echo "║   BattleChain Test Suite               ║"
echo "╚════════════════════════════════════════╝"

# Run unit tests
echo -e "\n[1/3] Running unit tests..."
cargo test --workspace --lib

# Run integration tests
echo -e "\n[2/3] Running integration tests..."
cargo test --workspace --test '*'

# Run doc tests
echo -e "\n[3/3] Running doc tests..."
cargo test --workspace --doc

echo -e "\n✓ All tests passed!"
```

#### 3.3 Create Deployment Script

**Create:** `battlechain-linera/scripts/deploy-local.sh`

```bash
#!/bin/bash
# Full deployment script (will create in next step)
# Based on microcard/run.bash structure
```

### Phase 4: Directory Structure (Priority 2)

#### New Structure:

```
battlechain-linera/
├── Cargo.toml                    # Workspace config
├── rust-toolchain.toml           # Rust version
├── README.md                     # Project overview
├── CODE_REORGANIZATION_PLAN.md   # This document
├── DEPLOYMENT.md                 # Deployment guide
│
├── shared-types/                 # ✅ Existing - extend
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── character.rs          # Character types
│       ├── battle_participant.rs # NEW - Battle participant
│       ├── combat.rs             # NEW - Combat types
│       └── utils.rs              # Utility functions
│
├── shared-events/                # NEW - Event types
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── battle_events.rs      # Battle events
│       └── market_events.rs      # Prediction market events
│
├── battle-token/                 # ✅ Fix error handling
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── token_tests.rs
│
├── player-chain/                 # ✅ Existing - good structure
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── player_tests.rs       # NEW
│
├── battle-chain/                 # ✅ Refactor to use shared types
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── battle_tests.rs       # NEW
│
├── matchmaking-chain/            # ✅ Fix incomplete implementations
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── matchmaking_tests.rs  # NEW
│
├── prediction-chain/             # ✅ Complete event subscriptions
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── prediction_tests.rs   # NEW
│
├── registry-chain/               # ✅ Existing - good structure
│   ├── Cargo.toml
│   ├── src/
│   │   └── lib.rs
│   └── tests/
│       └── registry_tests.rs     # NEW
│
├── scripts/                      # NEW - Build/deploy automation
│   ├── build.sh                  # Build all chains
│   ├── test.sh                   # Run all tests
│   ├── deploy-local.sh           # Local deployment
│   ├── deploy-testnet.sh         # Testnet deployment
│   └── helpers.sh                # Shared functions
│
├── tests/                        # NEW - Integration tests
│   ├── integration_test.rs       # End-to-end tests
│   ├── battle_flow_test.rs       # Battle flow tests
│   └── market_flow_test.rs       # Prediction market tests
│
└── docs/                         # NEW - Documentation
    ├── ARCHITECTURE.md           # Architecture overview
    ├── API.md                    # GraphQL API docs
    ├── DEPLOYMENT.md             # Deployment guide
    └── DEVELOPMENT.md            # Development guide
```

### Phase 5: Implementation Order (Priority 3)

**Week 1: Foundation**
- [ ] Day 1-2: Create shared-events crate
- [ ] Day 3-4: Extend shared-types with BattleParticipant
- [ ] Day 5: Update all Cargo.toml dependencies
- [ ] Day 6-7: Remove duplicate code, update imports

**Week 2: Error Handling**
- [ ] Day 1: Fix battle-token error handling
- [ ] Day 2: Fix battle-chain error handling
- [ ] Day 3: Fix matchmaking-chain error handling
- [ ] Day 4: Fix prediction-chain error handling
- [ ] Day 5: Fix registry-chain error handling
- [ ] Day 6-7: Add logging everywhere

**Week 3: Build Infrastructure**
- [ ] Day 1-2: Create build.sh script
- [ ] Day 3: Create test.sh script
- [ ] Day 4-5: Create deploy-local.sh script
- [ ] Day 6-7: Test end-to-end deployment

**Week 4: Testing & Documentation**
- [ ] Day 1-3: Write unit tests for each chain
- [ ] Day 4-5: Write integration tests
- [ ] Day 6-7: Documentation update

---

## Success Criteria

### Code Quality
- [ ] No code duplication
- [ ] Consistent error handling (no silent failures)
- [ ] All operations logged
- [ ] All chains build successfully
- [ ] Clippy passes with zero warnings

### Testing
- [ ] Unit tests for each chain (>70% coverage)
- [ ] Integration tests for full flows
- [ ] All tests pass
- [ ] Test documentation

### Build System
- [ ] Single command to build all chains
- [ ] Single command to run all tests
- [ ] Single command to deploy locally
- [ ] Build time < 5 minutes

### Documentation
- [ ] Architecture documented
- [ ] API documented
- [ ] Deployment guide complete
- [ ] Development guide complete

---

## Next Steps

1. **Create shared-events crate** (Start here)
2. **Update Cargo.toml** to add shared-events to workspace
3. **Refactor battle-chain** to export BattleEvent from shared-events
4. **Update prediction-chain and registry-chain** to import from shared-events
5. **Test builds** to ensure everything compiles

Let's start with Step 1!
