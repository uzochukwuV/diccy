# BattleChain Automatic Deployment Implementation Guide

**Based on**: Linera SDK 0.15.6 Research
**Solution**: Use automatic deployment triggered by cross-chain messages

---

## Implementation Steps

### Step 1: Add Initialize Message to Battle Chain

The battle chain needs an `Initialize` message variant to receive when auto-deployed.

**File**: `battle-chain/src/lib.rs`

Add to Message enum:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Initialize battle after auto-deployment (FIRST message received)
    Initialize {
        player1: BattleParticipant,
        player2: BattleParticipant,
        matchmaking_chain: ChainId,
    },

    /// Notify player of battle result (existing)
    BattleResult {
        winner: Owner,
        loser: Owner,
        winner_payout: Amount,
    },

    // ... other messages
}
```

Implement handler in `execute_message`:
```rust
async fn execute_message(&mut self, message: Message) {
    match message {
        Message::Initialize { player1, player2, matchmaking_chain } => {
            // Verify sender is matchmaking chain (security)
            let sender_chain = self.runtime.message_origin_chain_id()
                .expect("Message must have origin");

            assert_eq!(sender_chain, matchmaking_chain, "Only matchmaking can initialize");

            // Set battle participants
            self.state.player1.set(Some(player1));
            self.state.player2.set(Some(player2));

            // Initialize battle state
            self.state.status.set(BattleStatus::InProgress);
            self.state.current_round.set(0);
            self.state.max_rounds.set(10);  // Or from config
            self.state.winner.set(None);

            log::info!("Battle initialized: {:?} vs {:?}",
                player1.owner, player2.owner);
        }

        Message::BattleResult { .. } => {
            // Existing handling (currently empty)
        }
    }
}
```

---

### Step 2: Update Matchmaking to Send Initialize Message

**File**: `matchmaking-chain/src/lib.rs`

Modify `create_battle_chain()` method:

```rust
async fn create_battle_chain(&mut self, pending: PendingBattle) {
    // ... existing ownership setup code ...

    // Create the battle chain
    let battle_chain_id = self.runtime.open_chain(
        chain_ownership,
        application_permissions,
        total_stake,
    );

    // ===== NEW CODE: Send Initialize message =====
    // This triggers automatic deployment of battle application!

    use battle_chain::Message as BattleMessage;
    use battle_chain::BattleParticipant;

    // Create participant structs
    let participant1 = BattleParticipant::new(
        pending.player1.player_owner,
        pending.player1.player_chain,
        pending.player1.character,
        pending.player1.stake,
    );

    let participant2 = BattleParticipant::new(
        pending.player2.player_owner,
        pending.player2.player_chain,
        pending.player2.character,
        pending.player2.stake,
    );

    // Send initialization message to NEW battle chain
    // Linera will auto-deploy the battle application when it sees this message!
    self.runtime.prepare_message(BattleMessage::Initialize {
        player1: participant1,
        player2: participant2,
        matchmaking_chain: self.runtime.chain_id(),
    })
    .with_authentication()  // Verify sender
    .send_to(battle_chain_id);

    log::info!("Sent Initialize message to battle chain {:?}", battle_chain_id);

    // ===== END NEW CODE =====

    // Store battle metadata (existing code)
    let metadata = BattleMetadata {
        player1: pending.player1.player_chain,
        player2: pending.player2.player_chain,
        stake: total_stake,
        started_at: self.runtime.system_time(),
    };

    self.state.active_battles.insert(&battle_chain_id, metadata)
        .expect("Failed to store battle metadata");

    // Notify players (existing code)
    // ... rest of existing code ...
}
```

---

### Step 3: Update Deployment Script

**File**: `scripts/deploy-all.sh` (create if doesn't exist)

```bash
#!/bin/bash
set -e

echo "🚀 Deploying BattleChain Applications"
echo "====================================="

# 1. Publish bytecodes
echo "📦 Publishing bytecodes..."

SHARED_TYPES_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/shared-types/target/wasm32-unknown-unknown/release/battlechain_shared_types_{contract,service}.wasm 2>/dev/null || echo "")

BATTLE_TOKEN_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/battle-token/target/wasm32-unknown-unknown/release/battle_token_{contract,service}.wasm)
echo "✅ Battle Token bytecode: $BATTLE_TOKEN_BYTECODE"

PLAYER_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/player-chain/target/wasm32-unknown-unknown/release/player_chain_{contract,service}.wasm)
echo "✅ Player Chain bytecode: $PLAYER_BYTECODE"

BATTLE_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/battle-chain/target/wasm32-unknown-unknown/release/battle_chain_{contract,service}.wasm)
echo "✅ Battle Chain bytecode: $BATTLE_BYTECODE"

REGISTRY_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/registry-chain/target/wasm32-unknown-unknown/release/registry_chain_{contract,service}.wasm)
echo "✅ Registry Chain bytecode: $REGISTRY_BYTECODE"

PREDICTION_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/prediction-chain/target/wasm32-unknown-unknown/release/prediction_chain_{contract,service}.wasm)
echo "✅ Prediction Chain bytecode: $PREDICTION_BYTECODE"

MATCHMAKING_BYTECODE=$(linera publish-bytecode \
  battlechain-linera/matchmaking-chain/target/wasm32-unknown-unknown/release/matchmaking_chain_{contract,service}.wasm)
echo "✅ Matchmaking Chain bytecode: $MATCHMAKING_BYTECODE"

# 2. Create applications in dependency order
echo ""
echo "🔗 Creating application instances..."

# Token first (no dependencies)
BATTLE_TOKEN_APP=$(linera create-application $BATTLE_TOKEN_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{"name":"BattleToken","symbol":"BATTLE","initial_supply":"1000000000000"}')
echo "✅ Battle Token app: $BATTLE_TOKEN_APP"

# Registry (no dependencies)
REGISTRY_APP=$(linera create-application $REGISTRY_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{}')
echo "✅ Registry app: $REGISTRY_APP"

# Player chain (depends on token)
PLAYER_APP=$(linera create-application $PLAYER_BYTECODE \
  --json-parameters '100000000' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo "✅ Player app: $PLAYER_APP"

# Battle chain (depends on token)
BATTLE_APP=$(linera create-application $BATTLE_BYTECODE \
  --json-parameters '{}' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo "✅ Battle app: $BATTLE_APP"

# Prediction market (depends on token)
PREDICTION_APP=$(linera create-application $PREDICTION_BYTECODE \
  --json-parameters '300' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_TOKEN_APP)
echo "✅ Prediction app: $PREDICTION_APP"

# Matchmaking (depends on battle app - CRITICAL for auto-deployment!)
MATCHMAKING_APP=$(linera create-application $MATCHMAKING_BYTECODE \
  --json-parameters '1000000000' \
  --json-argument '{}' \
  --required-application-ids $BATTLE_APP \
  --required-application-ids $BATTLE_TOKEN_APP \
  --required-application-ids $PLAYER_APP \
  --required-application-ids $REGISTRY_APP)
echo "✅ Matchmaking app: $MATCHMAKING_APP"

# 3. Configure references
echo ""
echo "⚙️  Configuring application references..."

# Get default chain owner
DEFAULT_OWNER=$(linera wallet show | grep "Owner" | head -1 | awk '{print $2}')
DEFAULT_CHAIN=$(linera wallet show | grep "Chain" | head -1 | awk '{print $3}')

# Update matchmaking with app IDs
linera execute-operation \
  --application-id $MATCHMAKING_APP \
  --json-operation "{
    \"UpdateReferences\": {
      \"battle_app_id\": \"$BATTLE_APP\",
      \"battle_token_app\": \"$BATTLE_TOKEN_APP\",
      \"treasury_owner\": {\"Account\": \"$DEFAULT_OWNER\"}
    }
  }"
echo "✅ Matchmaking configured"

# 4. Export environment variables
echo ""
echo "📝 Saving application IDs to .env..."

cat > .env << EOF
# BattleChain Application IDs
# Generated: $(date)

BATTLE_TOKEN_APP=$BATTLE_TOKEN_APP
PLAYER_APP=$PLAYER_APP
BATTLE_APP=$BATTLE_APP
REGISTRY_APP=$REGISTRY_APP
PREDICTION_APP=$PREDICTION_APP
MATCHMAKING_APP=$MATCHMAKING_APP

# Bytecode IDs (for reference)
BATTLE_TOKEN_BYTECODE=$BATTLE_TOKEN_BYTECODE
PLAYER_BYTECODE=$PLAYER_BYTECODE
BATTLE_BYTECODE=$BATTLE_BYTECODE
REGISTRY_BYTECODE=$REGISTRY_BYTECODE
PREDICTION_BYTECODE=$PREDICTION_BYTECODE
MATCHMAKING_BYTECODE=$MATCHMAKING_BYTECODE

# Default chain/owner
DEFAULT_CHAIN=$DEFAULT_CHAIN
DEFAULT_OWNER=$DEFAULT_OWNER
EOF

echo ""
echo "✅ Deployment complete!"
echo "====================================="
echo "Application IDs saved to .env"
echo ""
echo "Quick reference:"
echo "  Matchmaking: $MATCHMAKING_APP"
echo "  Battle:      $BATTLE_APP"
echo "  Player:      $PLAYER_APP"
echo "  Token:       $BATTLE_TOKEN_APP"
echo ""
echo "Next steps:"
echo "  1. Test character creation: linera execute-operation --application-id $PLAYER_APP ..."
echo "  2. Join matchmaking queue"
echo "  3. Confirm battle offer"
echo "  4. Battle application will auto-deploy to new battle chain! 🎮"
```

---

### Step 4: Update DEPLOYMENT.md

Add section explaining automatic deployment:

```markdown
## How Automatic Deployment Works

BattleChain uses Linera's automatic application deployment feature:

1. **Matchmaking creates empty battle chain** (multi-owner)
2. **Matchmaking sends `Initialize` message** to battle chain
3. **Linera detects message** to non-existent application
4. **Linera checks** matchmaking's `required_application_ids`
5. **Linera auto-deploys** battle application to battle chain
6. **Message delivered** to newly deployed battle application
7. **Battle initialized** and ready for combat!

### Required Application IDs

When creating matchmaking application, you MUST include battle app:

```bash
linera create-application $MATCHMAKING_BYTECODE \
  --required-application-ids $BATTLE_APP  # <-- CRITICAL!
```

This tells Linera: "When matchmaking sends messages to chains without the battle app, automatically deploy it."
```

---

### Step 5: Add Integration Test

**File**: `tests/battle_creation_test.rs`

```rust
#[tokio::test]
async fn test_battle_chain_automatic_deployment() {
    // Setup validator
    let validator = linera_sdk::test::TestValidator::default();

    // Create matchmaking and battle bytecodes
    let battle_bytecode = /* ... */;
    let matchmaking_bytecode = /* ... */;

    // Create battle app
    let mut admin_chain = validator.new_chain().await;
    let battle_app = admin_chain
        .create_application(battle_bytecode, (), (), vec![])
        .await;

    // Create matchmaking app with battle as required dependency
    let matchmaking_app = admin_chain
        .create_application(
            matchmaking_bytecode,
            (),
            (),
            vec![battle_app],  // <-- Required for auto-deployment
        )
        .await;

    // Create two player chains
    let mut player1_chain = validator.new_chain().await;
    let mut player2_chain = validator.new_chain().await;

    // Players join queue
    player1_chain.execute_operation(
        matchmaking_app,
        MatchmakingOperation::JoinQueue { /* ... */ },
    ).await;

    player2_chain.execute_operation(
        matchmaking_app,
        MatchmakingOperation::JoinQueue { /* ... */ },
    ).await;

    // Create battle offer
    admin_chain.execute_operation(
        matchmaking_app,
        MatchmakingOperation::CreateBattleOffer {
            player1_chain: player1_chain.id(),
            player2_chain: player2_chain.id(),
        },
    ).await;

    // Players confirm
    player1_chain.execute_operation(
        matchmaking_app,
        MatchmakingOperation::ConfirmBattleOffer { offer_id: 0, /* ... */ },
    ).await;

    player2_chain.execute_operation(
        matchmaking_app,
        MatchmakingOperation::ConfirmBattleOffer { offer_id: 0, /* ... */ },
    ).await;

    // Verify battle chain was created
    let battles = admin_chain.query_application(
        matchmaking_app,
        MatchmakingQuery::ActiveBattles,
    ).await;

    assert_eq!(battles.len(), 1);
    let battle_chain_id = battles[0].battle_chain;

    // CRITICAL TEST: Verify battle application exists on battle chain
    let battle_state = validator.chain(battle_chain_id)
        .query_application(battle_app, BattleQuery::Status)
        .await;

    assert_eq!(battle_state.status, BattleStatus::InProgress);
    assert!(battle_state.player1.is_some());
    assert!(battle_state.player2.is_some());

    println!("✅ Battle application auto-deployed successfully!");
}
```

---

## Summary of Changes

| File | Change | Why |
|------|--------|-----|
| `battle-chain/src/lib.rs` | Add `Message::Initialize` variant | Receive initialization from matchmaking |
| `battle-chain/src/lib.rs` | Implement `execute_message()` | Handle auto-deployment initialization |
| `matchmaking-chain/src/lib.rs` | Send `BattleMessage::Initialize` | Trigger auto-deployment |
| `scripts/deploy-all.sh` | Add `--required-application-ids` | Enable auto-deployment mechanism |
| `DEPLOYMENT.md` | Document auto-deployment | Help future developers |
| `tests/battle_creation_test.rs` | Integration test | Verify solution works |

---

## Testing Checklist

- [ ] Build all applications successfully
- [ ] Deploy with updated script including `--required-application-ids`
- [ ] Create two player chains
- [ ] Join matchmaking queue
- [ ] Create and confirm battle offer
- [ ] Verify battle chain created (check logs)
- [ ] **CRITICAL**: Verify battle application exists on battle chain
- [ ] Submit turn from player 1 (tests message handling)
- [ ] Submit turn from player 2
- [ ] Complete battle and verify results

---

## How to Verify It Worked

```bash
# After battle chain is created, check if application exists:
linera query-application --chain-id $BATTLE_CHAIN_ID --application-id $BATTLE_APP

# Should return battle state, not "Application not found"
```

If you see the battle state, congratulations! Automatic deployment worked! 🎉
