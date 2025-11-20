# Battlechain Test Suite

Comprehensive testing documentation for the Battlechain Linera smart contract ecosystem.

## Table of Contents

1. [Overview](#overview)
2. [Unit Tests](#unit-tests)
3. [Integration Tests](#integration-tests)
4. [Running Tests](#running-tests)
5. [Test Coverage](#test-coverage)
6. [Writing New Tests](#writing-new-tests)

## Overview

The Battlechain test suite consists of two main types of tests:

- **Unit Tests**: Rust-based tests for individual contract functionality
- **Integration Tests**: Shell scripts that test complete workflows across multiple contracts

## Unit Tests

### Battle Token Tests
**Location**: `battle-token/tests/token_tests.rs`

Tests token functionality including:
- ✅ Token transfers
- ✅ Insufficient balance handling
- ✅ Approve and transferFrom
- ✅ Token burning
- ✅ Multiple transfers and holder tracking
- ✅ Zero amount transfer rejection
- ✅ Self-transfer rejection
- ✅ Allowance deduction
- ✅ Token minting
- ✅ High-volume transfers

**Run token tests:**
```bash
cd battlechain-linera/battle-token
cargo test --target x86_64-unknown-linux-gnu
```

### Player Chain Tests
**Location**: `player-chain/tests/player_tests.rs`

Tests character management:
- ✅ Character creation
- ✅ Character leveling up
- ✅ Multiple characters of different classes
- ✅ Duplicate character prevention
- ✅ Equipment system

**Run player tests:**
```bash
cd battlechain-linera/player-chain
cargo test --target x86_64-unknown-linux-gnu
```

### Matchmaking Chain Tests
**Location**: `matchmaking-chain/tests/matchmaking_tests.rs`

Tests matchmaking functionality:
- ✅ Joining matchmaking queue
- ✅ Two-player matching
- ✅ Leaving queue
- ✅ Minimum stake requirements
- ✅ ELO-based matchmaking

**Run matchmaking tests:**
```bash
cd battlechain-linera/matchmaking-chain
cargo test --target x86_64-unknown-linux-gnu
```

### Battle Chain Tests
**Location**: `battle-chain/tests/battle_tests.rs`

Tests combat mechanics:
- ✅ Battle initialization
- ✅ Turn submission
- ✅ Round execution
- ✅ Battle finalization
- ✅ Combat mechanics (offensive/defensive/balanced)
- ✅ Special ability cooldowns
- ✅ Combo system
- ✅ Critical hits
- ✅ Dodge mechanics
- ✅ Rewards distribution

**Run battle tests:**
```bash
cd battlechain-linera/battle-chain
cargo test --target x86_64-unknown-linux-gnu
```

### Prediction Chain Tests
**Location**: `prediction-chain/tests/prediction_tests.rs`

Tests prediction market:
- ✅ Market creation
- ✅ Placing bets
- ✅ Closing markets
- ✅ Settling markets
- ✅ Multiple bets on different sides
- ✅ Odds calculation
- ✅ Winnings distribution
- ✅ Market cancellation and refunds

**Run prediction tests:**
```bash
cd battlechain-linera/prediction-chain
cargo test --target x86_64-unknown-linux-gnu
```

### Registry Chain Tests
**Location**: `registry-chain/tests/registry_tests.rs`

Tests global registry:
- ✅ Character registration
- ✅ Character stats updates
- ✅ Battle recording
- ✅ ELO rating system
- ✅ Leaderboard rankings
- ✅ Character statistics tracking
- ✅ Battle history retrieval

**Run registry tests:**
```bash
cd battlechain-linera/registry-chain
cargo test --target x86_64-unknown-linux-gnu
```

## Integration Tests

### Complete Deployment Test
**Location**: `tests/test_battlechain_deployment.sh`

This script performs a complete deployment and initial testing of all Battlechain contracts.

**What it does:**
1. Initializes a new Linera wallet
2. Creates multiple player chains
3. Deploys all 6 contracts in the correct order:
   - Battle Token
   - Registry Chain
   - Player Chain
   - Prediction Chain
   - Matchmaking Chain
   - Battle Chain
4. Tests token distribution
5. Tests character creation
6. Tests matchmaking queue
7. Tests prediction market integration

**Usage:**
```bash
./tests/test_battlechain_deployment.sh <FAUCET_URL> <GRAPHQL_URL> <LOCAL_NETWORK_URL>

# Example:
./tests/test_battlechain_deployment.sh \
  http://localhost:8080 \
  http://localhost:8081 \
  http://localhost:8081
```

**Expected Output:**
```
=== STEP 1: Initializing Wallet and Creating Chains ===
Default Chain: e476187f7b34c6eabbb1...
Player Chains Created:
  Player 1: a1b2c3d4...
  Player 2: e5f6g7h8...
  ...

=== STEP 2: Deploying Battle Token Contract ===
Battle Token App ID: ...

...

Total Runtime: 45 seconds and 234 ms
```

### Battle Flow Test
**Location**: `tests/test_battle_flow.sh`

Tests a complete battle from start to finish.

**What it does:**
1. Players join matchmaking queue
2. Match is created
3. Players submit turns for each round
4. Rounds are executed
5. Battle is finalized
6. Winner is determined
7. Rewards are distributed

**Usage:**
```bash
./tests/test_battle_flow.sh \
  <GRAPHQL_URL> \
  <DEFAULT_CHAIN> \
  <PLAYER1_CHAIN> \
  <PLAYER2_CHAIN> \
  <MATCHMAKING_APP_ID> \
  <BATTLE_CHAIN_APP_ID>
```

## Running Tests

### Run All Unit Tests
```bash
# From the battlechain-linera directory
cargo test --all --target x86_64-unknown-linux-gnu
```

### Run Tests for Specific Contract
```bash
cd battlechain-linera/<contract-name>
cargo test --target x86_64-unknown-linux-gnu
```

### Run Integration Tests

**Prerequisites:**
1. Linera node running locally
2. Faucet service available
3. GraphQL endpoint configured

**Step 1: Start Local Linera Network**
```bash
# Start local network (if not already running)
linera net up

# Get network URLs
linera net info
```

**Step 2: Run Deployment Test**
```bash
cd battlechain-linera
./tests/test_battlechain_deployment.sh \
  http://localhost:8080 \
  http://localhost:8081 \
  http://localhost:8081
```

**Step 3: Run Battle Flow Test**
Use the chain IDs and app IDs from the deployment test output.

## Test Coverage

### Current Coverage by Contract

| Contract | Unit Tests | Integration Tests | Coverage |
|----------|------------|-------------------|----------|
| Battle Token | ✅ 11 tests | ✅ Included | High |
| Player Chain | ✅ 6 tests | ✅ Included | Medium |
| Matchmaking | ✅ 5 tests | ✅ Included | Medium |
| Battle Chain | ✅ 10 tests | ✅ Included | High |
| Prediction | ✅ 8 tests | ✅ Included | High |
| Registry | ✅ 8 tests | ✅ Included | High |

### Tested Scenarios

#### Token Operations ✅
- Transfer tokens between players
- Approve spending allowances
- Burn tokens
- Mint tokens (admin only)
- Handle insufficient balances
- Track holder statistics

#### Character Management ✅
- Create characters of different classes
- Level up characters
- Equip items
- Track character statistics
- Prevent duplicate characters

#### Matchmaking ✅
- Join/leave queue
- Match players with similar ELO
- Enforce minimum stakes
- Create battles
- Handle queue timeouts

#### Battle Mechanics ✅
- Initialize battles
- Submit turns (offensive/defensive/balanced)
- Execute rounds with combat calculations
- Handle special abilities
- Track combo stacks
- Calculate critical hits and dodges
- Finalize battles
- Distribute rewards

#### Prediction Markets ✅
- Create markets for battles
- Place bets on outcomes
- Close betting when battle starts
- Settle markets with winners
- Calculate and distribute winnings
- Handle market cancellations

#### Registry & Leaderboards ✅
- Register characters globally
- Track battle statistics
- Update ELO ratings
- Maintain leaderboards
- Record battle history

## Writing New Tests

### Adding Unit Tests

1. Create a new test file in `<contract>/tests/`:
```rust
use <contract_name>::*;
use linera_sdk::test::{ActiveChain, TestValidator};

#[tokio::test(flavor = "multi_thread")]
async fn test_my_feature() {
    let (validator, module_id) = TestValidator::with_current_module::<MyAbi, (), ()>().await;
    let mut chain = validator.new_chain().await;

    // Your test logic here
}
```

2. Run your test:
```bash
cargo test test_my_feature --target x86_64-unknown-linux-gnu
```

### Adding Integration Tests

1. Create a new shell script in `tests/`:
```bash
#!/bin/bash

# Your test scenario here
```

2. Make it executable:
```bash
chmod +x tests/my_test.sh
```

3. Add documentation to this README

### Test Best Practices

1. **Isolation**: Each test should be independent
2. **Cleanup**: Tests should not leave residual state
3. **Assertions**: Always verify expected outcomes
4. **Documentation**: Comment complex test logic
5. **Coverage**: Test both success and failure cases
6. **Performance**: Keep tests reasonably fast

## Continuous Integration

### GitHub Actions (Recommended)

Create `.github/workflows/test.yml`:
```yaml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run unit tests
        run: cargo test --all --target x86_64-unknown-linux-gnu
```

## Troubleshooting

### Common Issues

**Test fails with "address already in use":**
- Another Linera service is running on the same port
- Solution: Stop other services or use different ports

**Test fails with "faucet timeout":**
- Faucet service is not responding
- Solution: Check faucet URL and network connectivity

**Test fails with "contract not found":**
- Contract was not deployed successfully
- Solution: Check deployment logs for errors

**WASM compilation errors:**
- Missing dependencies or incorrect build target
- Solution: Run `cargo build --release --target wasm32-unknown-unknown`

## Additional Resources

- [Linera SDK Documentation](https://docs.linera.io)
- [Linera Testing Guide](https://docs.linera.io/testing)
- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Microcard Test Examples](../microcard/tests)

## Contributing

When adding new features, please:
1. Write unit tests for core functionality
2. Update integration tests if workflow changes
3. Update this README with new test documentation
4. Ensure all tests pass before submitting PR

---

**Last Updated**: 2025-11-20
**Test Suite Version**: 1.0.0
