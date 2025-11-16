# BattleChain-Linera Analysis & Recommendations

**Date**: November 2025
**Project**: BattleChain - Blockchain Fighting Game with Prediction Market on Linera

---

## Executive Summary

BattleChain is an impressive fully on-chain fighting game built on Linera's microchains architecture. The project demonstrates a sophisticated understanding of Linera's capabilities with:

- ✅ 6 distinct microchain applications (Player, Battle, Matchmaking, Prediction, Registry, Battle Token)
- ✅ Multi-owner chain creation implemented for battles
- ✅ Turn-based combat engine with 5 character classes
- ✅ ELO rating system and global leaderboards
- ✅ Prediction market for spectator betting
- ✅ Cross-chain messaging architecture

**Current Status**: Core gameplay is complete, but there's a critical gap in the battle chain instantiation workflow.

---

## Architecture Analysis

### Current Implementation

#### 1. **Player Chain** (Single-Owner) ✅
- NFT character ownership and management
- Personal inventory and battle stakes
- Single-owner for maximum performance (1000+ TPS)
- **Status**: Well-implemented

#### 2. **Battle Chain** (Multi-Owner) ⚠️
- Turn-based combat between two players
- Multi-owner chain (both players must sign)
- Randomness generation using timestamps
- **Issue**: Application instantiation on newly created battle chains needs implementation

#### 3. **Matchmaking Chain** (Public) ✅
- Queue management and battle coordination
- Creates multi-owner battle chains using `runtime.open_chain()`
- **Gap**: Creates empty chain but doesn't instantiate battle application on it

#### 4. **Prediction Market Chain** (Public) ✅
- Spectator betting with dynamic odds
- Pool-based odds calculation
- Platform fee system (3% default)
- **Note**: Needs proper fixed-point arithmetic for payout calculations (marked TODO)

#### 5. **Registry Chain** (Public) ✅
- Global leaderboards with ELO ratings
- Battle history tracking
- Character statistics
- **Optimization needed**: Sorted leaderboard implementation

#### 6. **Battle Token** (Public) ✅
- Fungible token for game economy
- Transfer, mint, burn operations
- **Note**: Admin access control needs implementation

### Shared Types ✅
- Character classes, stances, combat mechanics
- Randomness utilities with fixed-point math
- Common data structures across all chains

---

## Critical Issue: Multi-Owner Battle Chain Application Instantiation

### The Problem

In `matchmaking-chain/src/lib.rs` (lines 264-268), the code creates a multi-owner battle chain:

```rust
let battle_chain_id = self.runtime.open_chain(
    chain_ownership,
    application_permissions,
    total_stake,
);
```

**However**, this only creates an **empty chain**. The battle application is **NOT instantiated** on the newly created chain. This means:

1. Battle chain is created with proper multi-owner ownership ✅
2. Application permissions are set ✅
3. **BUT** the battle application contract is not running on that chain ❌

### Why This Matters

- Players receive notifications about the battle chain
- When they try to submit turns, the battle application doesn't exist on that chain
- No contract to handle `SubmitStance`, `UseSpecialAbility`, etc.

### The Solution

You need to instantiate the battle application on the newly created chain. In Linera, this typically requires one of these approaches:

#### **Approach 1: Use `runtime.create_application()` (Recommended)**

If the Linera SDK supports creating applications on other chains from within a contract:

```rust
// After creating the chain
let battle_chain_id = self.runtime.open_chain(
    chain_ownership,
    application_permissions,
    total_stake,
);

// Instantiate battle application on the new chain
// This requires the battle application's bytecode ID
let battle_bytecode_id = /* stored during deployment */;

self.runtime.create_application_on_chain(
    battle_chain_id,
    battle_bytecode_id,
    battle_initialization_argument,
    vec![], // required_application_ids
)?;
```

**What you need to research:**
- Check if `ContractRuntime` has a method to create applications on other chains
- Look at `docs.rs/linera-sdk/0.15.5/linera_sdk/contract/struct.ContractRuntime.html`
- Search for methods like `create_application`, `instantiate_on_chain`, etc.

#### **Approach 2: Message-Based Instantiation**

Send a message to the newly created chain instructing it to instantiate the battle application:

```rust
// After creating the chain
let battle_chain_id = self.runtime.open_chain(...);

// Send initialization message to the new chain
let init_msg = Message::InitializeBattle {
    player1: pending.player1,
    player2: pending.player2,
    bytecode_id: battle_bytecode_id,
};

self.runtime
    .prepare_message(init_msg)
    .with_authentication()
    .send_to(battle_chain_id);
```

The battle chain would need a system application or bootstrap contract that can handle this message and create the battle application.

#### **Approach 3: CLI-Based Approach (Workaround)**

For MVP/testing, you could:

1. Matchmaking creates the multi-owner chain
2. Returns the chain ID to players
3. Players (or a bot) use CLI to instantiate the battle application:
   ```bash
   linera create-application \
     --bytecode-id $BATTLE_BYTECODE_ID \
     --chain-id $BATTLE_CHAIN_ID \
     --json-argument '{"player1": ..., "player2": ...}'
   ```

This is not ideal for production but could unblock testing.

---

## Recommendations by Category

### 1. **Immediate Priority: Battle Chain Application Instantiation**

**Research Tasks:**
- [ ] Review Linera SDK `ContractRuntime` API (v0.15.5)
- [ ] Check if `create_application` or similar method exists for creating apps on other chains
- [ ] Study Linera composition patterns for multi-chain application orchestration
- [ ] Review example applications in `linera-protocol` GitHub repository

**Documentation to Review:**
- `https://docs.rs/linera-sdk/0.15.5/linera_sdk/` (blocked, but try via GitHub)
- Look for examples in `linera-io/linera-protocol` repository
- Search for "application composition" or "create application" in SDK

**Implementation Path:**
1. Identify the correct SDK method for instantiating applications on other chains
2. Store battle application's `BytecodeId` in matchmaking state during deployment
3. Modify `create_battle_chain()` to instantiate the battle application
4. Pass initialization parameters (player1, player2, stakes, characters) to battle app

### 2. **Prediction Market Enhancements**

**Current Gaps (from TODOs):**
- Fixed-point arithmetic for odds-based payouts (line 115 in prediction-chain)
- Proper refund logic for cancelled battles (line 415)

**Recommendations:**
```rust
// Use your existing FP_SCALE from shared-types
// Calculate winnings with fixed-point math
let odds = (total_pool * FP_SCALE) / side_pool;
let winnings = mul_fp(bet_amount, odds);
let platform_fee = mul_fp(winnings, platform_fee_bps * FP_SCALE / 10000);
let net_payout = winnings.saturating_sub(platform_fee);
```

### 3. **Matchmaking Improvements**

**Current Limitation:**
- Manual matchmaking only (marked TODO at line 128, 347-348)
- No skill-based matching

**Recommendations:**
- **Phase 1**: Implement FIFO automatic matching
  ```rust
  // When player joins queue, check if any other player is waiting
  // If yes, create battle offer automatically
  // This requires iterating MapView - use collector pattern
  ```

- **Phase 2**: ELO-based matchmaking
  - Query Registry chain for player ELO ratings
  - Match players within ±200 ELO range
  - Fall back to FIFO after timeout (e.g., 30 seconds)

**Storage Optimization:**
```rust
// Add index for efficient matching
pub waiting_players_by_elo: MapView<u16, Vec<ChainId>>
```

### 4. **Token Economics & Security**

**Battle Token Issues:**
- Admin access control not implemented (line 423)
- Cross-chain credit messages commented out (line 460-461)
- Balance queries need Owner parsing (line 586, 592)

**Recommendations:**

```rust
// Add admin role
pub admin: RegisterView<Option<Owner>>,

// In execute_operation:
Operation::Mint { to, amount } => {
    let caller = self.runtime.authenticated_signer()
        .ok_or(TokenError::Unauthorized)?;

    let admin = self.state.admin.get()
        .ok_or(TokenError::NoAdmin)?;

    if caller != admin {
        return Err(TokenError::Unauthorized);
    }

    self.state.mint(to, amount, now).await?;
}
```

### 5. **Combat System Polish**

**Current State:**
- Platform fees not implemented (line 750-753)
- Winner takes all (no fee deduction)

**Implementation:**
```rust
// In battle-chain finalize_battle
let total_stake = p1.stake.saturating_add(p2.stake);
let platform_fee_bps = 300; // 3%
let fee_amount = total_stake.saturating_mul(platform_fee_bps) / 10000;
let winner_payout = total_stake.saturating_sub(fee_amount);

// Send fee to treasury
// Send payout to winner
```

### 6. **Registry & Leaderboards**

**Current Limitation:**
- Simple unsorted list (line 242)
- Inefficient for large player base

**Recommendations:**
- Use BTreeMap for auto-sorted leaderboard
- Implement pagination for top 100, 200, etc.
- Cache top N rankings in RegisterView for quick queries

```rust
pub struct RegistryState {
    // ... existing fields

    // Sorted by ELO (descending)
    pub leaderboard: CollectionView<Vec<(u16, String)>>, // (elo, character_id)

    // Quick access top 100
    pub top_100_cached: RegisterView<Vec<LeaderboardEntry>>,
}
```

---

## Linera-Specific Features to Leverage

Based on the documentation links you provided, here are features you should explore:

### 1. **Application Composition** (`backend/composition.html`)
- Synchronous calls between applications on same chain
- Ephemeral sessions for complex workflows
- Could be useful for player chain interacting with token application

**Use Case:**
```rust
// In player-chain, when joining battle
// Call battle-token app to lock stake in one transaction
self.runtime.call_application(
    token_app_id,
    Operation::Lock { amount: stake },
)?;
```

### 2. **Machine Learning Features** (`experimental/ml.html`)
- Experimental ML capabilities in Linera
- Could enhance combat AI or matchmaking

**Potential Uses:**
- AI-driven difficulty adjustment for PvE modes
- Predictive matchmaking quality scores
- Anomaly detection for cheating

### 3. **Publishing Applications** (`getting_started/hello_linera.html`)
- Two-step process: publish bytecode, then create instances
- Important for your battle chain instantiation issue

**Key Insight:**
You need to separate:
1. Publishing battle chain bytecode (done once during deployment)
2. Creating battle chain instances (done per-battle dynamically)

---

## Architecture Diagram (Current vs. Needed)

### Current Flow
```
Matchmaking Chain
    ├─> open_chain()
    │   └─> Creates empty multi-owner chain ✅
    │
    └─> Sends BattleCreated message to players ✅
        └─> Players try to submit turns ❌
            └─> No battle application exists on chain ❌
```

### Needed Flow
```
Matchmaking Chain
    ├─> open_chain()
    │   └─> Creates empty multi-owner chain ✅
    │
    ├─> instantiate_application_on_chain() 🔧
    │   └─> Deploys battle contract to new chain ✅
    │   └─> Initializes with player data ✅
    │
    └─> Sends BattleCreated message to players ✅
        └─> Players submit turns ✅
            └─> Battle application processes them ✅
```

---

## Testing Strategy

### Unit Tests
Your token tests are comprehensive ✅. Expand to:

```bash
# Test each chain
cd battle-chain && cargo test
cd matchmaking-chain && cargo test
cd prediction-chain && cargo test
```

### Integration Tests Needed
1. **End-to-End Battle Flow**
   - Player A creates character
   - Player B creates character
   - Both join queue
   - Matchmaking creates battle
   - **TEST**: Battle app exists on new chain
   - Players submit stances
   - Battle completes
   - Verify payouts

2. **Multi-Owner Chain Test**
   - Verify both players can sign
   - Verify non-players cannot sign
   - Test concurrent turn submissions

3. **Prediction Market Flow**
   - Market created for battle
   - Spectators place bets
   - Battle completes
   - Verify payouts calculated correctly
   - Test edge cases (cancelled battles, no bets, etc.)

---

## Deployment Checklist

Based on your DEPLOYMENT.md:

### Pre-Deployment
- [ ] Fix battle chain application instantiation
- [ ] Implement admin controls for token minting
- [ ] Add platform fee distribution
- [ ] Test multi-owner chain creation end-to-end
- [ ] Audit smart contract code
- [ ] Set up monitoring

### Deployment Steps
1. ✅ Deploy shared-types (dependency)
2. ✅ Deploy battle-token
3. ✅ Deploy player-chain
4. ⚠️ Deploy battle-chain (record BytecodeId!)
5. ⚠️ Deploy matchmaking-chain (needs BytecodeId)
6. ✅ Deploy prediction-chain
7. ✅ Deploy registry-chain
8. ⚠️ Configure references (including battle BytecodeId)

**New Step Required:**
```bash
# After deploying battle-chain bytecode
export BATTLE_BYTECODE_ID=$(linera publish-bytecode \
  target/wasm32-unknown-unknown/release/battle_chain.wasm)

# Then configure matchmaking with BytecodeId
linera execute-operation \
  --application-id $MATCHMAKING_APP_ID \
  --json-operation '{
    "UpdateReferences": {
      "battle_bytecode_id": "'$BATTLE_BYTECODE_ID'",
      "battle_app_id": null,
      "battle_token_app": "'$BATTLE_TOKEN_APP_ID'",
      "treasury_owner": {...}
    }
  }'
```

---

## Performance Considerations

### Current Performance Profile
| Chain | Type | Expected TPS | Bottleneck |
|-------|------|--------------|------------|
| Player | Single-owner | 1000+ | None |
| Battle | Multi-owner | 100+ | Consensus (2 owners) |
| Matchmaking | Public | 200+ | Queue operations |
| Registry | Public | 1000+ | Read-optimized |
| Prediction | Public | 500+ | Bet processing |

### Optimization Opportunities

1. **Batch Operations**
   - Bundle multiple stance submissions into one transaction
   - Reduces round-trips for multi-turn games

2. **State Pruning**
   - Archive completed battles from active state
   - Keep only recent N battles in hot storage
   - Move history to Registry chain

3. **Caching**
   - Cache frequently accessed data (top leaderboard, active battles)
   - Use RegisterView for hot data paths
   - Lazy load MapView entries

---

## Security Considerations

### Currently Implemented ✅
- No unsafe Rust code
- Input validation on operations
- Saturating arithmetic (no overflow)
- Authentication on sensitive operations

### Needs Attention ⚠️
1. **Admin Controls**: Token minting, platform configuration
2. **Rate Limiting**: Prevent spam joining/leaving queue
3. **Battle Timeout**: Handle AFK players
4. **Randomness Security**: Timestamp-based is deterministic but predictable

**Recommendations:**

```rust
// Add battle timeout
pub const MAX_TURN_TIME: u64 = 300; // 5 minutes

// In battle execution
if current_time - last_turn_time > MAX_TURN_TIME {
    // Auto-forfeit for AFK player
    return self.forfeit_battle(afk_player);
}
```

---

## Frontend Integration Roadmap

Your Phase 4 mentions frontend development. Key considerations:

### GraphQL Queries
You've already defined GraphQL schemas ✅

**Example queries to implement:**
```graphql
# Get active battles
query {
  matchmakingStats {
    activeBattles {
      battleChain
      player1
      player2
      stake
    }
  }
}

# Get character stats
query {
  character(id: "char_123") {
    class
    level
    elo
    winRate
    totalBattles
  }
}

# Get prediction market
query {
  market(battleChain: "0x...") {
    player1Odds
    player2Odds
    totalPool
    status
  }
}
```

### Web App Structure
```
frontend/
├── src/
│   ├── components/
│   │   ├── Character/
│   │   │   ├── CharacterCard.tsx
│   │   │   ├── CharacterStats.tsx
│   │   │   └── CharacterSelector.tsx
│   │   ├── Battle/
│   │   │   ├── BattleArena.tsx
│   │   │   ├── StanceSelector.tsx
│   │   │   └── CombatLog.tsx
│   │   ├── Matchmaking/
│   │   │   ├── Queue.tsx
│   │   │   └── BattleOffer.tsx
│   │   ├── Prediction/
│   │   │   ├── MarketCard.tsx
│   │   │   └── BetForm.tsx
│   │   └── Leaderboard/
│   │       └── Rankings.tsx
│   ├── hooks/
│   │   ├── useLineraClient.ts
│   │   ├── useCharacter.ts
│   │   ├── useBattle.ts
│   │   └── usePredictions.ts
│   └── lib/
│       └── linera-client.ts
```

---

## Summary of Action Items

### High Priority 🔴
1. **Research battle chain application instantiation** - Blocks battles from working
2. **Implement `create_application_on_chain()` in matchmaking** - Core feature
3. **Add BytecodeId storage to matchmaking state** - Required for instantiation
4. **Test end-to-end battle flow** - Verify solution works

### Medium Priority 🟡
1. Implement automatic matchmaking (remove manual CreateBattleOffer)
2. Add platform fee distribution in battle finalization
3. Implement admin controls for token minting
4. Fix prediction market payout calculations (fixed-point math)
5. Optimize leaderboard with sorted data structure

### Low Priority 🟢
1. Skill-based matchmaking with ELO
2. Battle timeout for AFK players
3. State pruning for completed battles
4. Frontend development
5. Tournament system (Phase 5)

---

## Questions to Research

Based on your documentation links:

1. **ContractRuntime API**
   - Does `runtime.create_application()` support specifying a target chain?
   - Is there a `runtime.instantiate_on_chain(chain_id, bytecode_id, args)`?
   - Can you query the runtime for available methods?

2. **Application Composition**
   - Can matchmaking use synchronous calls to instantiate battle app?
   - Are there examples of one app creating another app instance?

3. **Multi-Owner Chain Lifecycle**
   - After `open_chain()`, how do applications get added?
   - Is there a bootstrap process for new chains?
   - Can chain creators send initialization operations?

4. **Machine Learning Features**
   - What ML primitives are available?
   - Could you use ML for combat balancing or matchmaking?
   - Are there performance implications?

---

## Conclusion

BattleChain is an ambitious and well-architected project that leverages Linera's unique microchains paradigm effectively. The core issue is the gap between creating multi-owner chains and instantiating applications on them.

**Next Steps:**
1. Deep dive into Linera SDK documentation for `ContractRuntime` methods
2. Search linera-protocol GitHub for examples of dynamic application creation
3. Reach out to Linera community/Discord if documentation is unclear
4. Implement application instantiation in matchmaking chain
5. Test end-to-end battle creation and execution

Once the battle chain instantiation issue is resolved, the rest of the TODOs are straightforward enhancements. The architecture is solid, the code quality is good, and the game mechanics are well-designed.

**Great work on this project!** 🎮⛓️

---

*Analysis Date: November 16, 2025*
*Linera SDK Version: 0.15.5*
*Project: BattleChain on Linera Protocol*
