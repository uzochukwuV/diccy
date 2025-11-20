# Battlechain Inter-Contract Call Requirements

## Phase 2: Contract Flow Analysis and Requirements

---

## 1. Current Architecture Overview

### 1.1 Six-Chain System
```
┌──────────────────────────────────────────────────────────────────┐
│                    Battlechain Ecosystem                          │
├──────────────────────────────────────────────────────────────────┤
│                                                                   │
│  ┌──────────────┐     ┌────────────────┐     ┌───────────────┐  │
│  │ player-chain │────>│ matchmaking-   │────>│ battle-chain  │  │
│  │  (Character  │     │    chain       │     │  (Combat      │  │
│  │   Management)│     │  (Matchmaking) │     │   Logic)      │  │
│  └──────────────┘     └────────────────┘     └───────────────┘  │
│         │                     │                      │           │
│         │                     │                      │           │
│         ↓                     ↓                      ↓           │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │               battle-token (ERC20-like Token)            │   │
│  │         (Balance tracking, Transfers, Staking)           │   │
│  └──────────────────────────────────────────────────────────┘   │
│         ↑                     ↑                      ↑           │
│         │                     │                      │           │
│  ┌──────────────┐     ┌────────────────┐     ┌───────────────┐  │
│  │ prediction-  │     │  registry-     │     │               │  │
│  │    chain     │     │     chain      │     │               │  │
│  │  (Betting)   │     │ (Leaderboard)  │     │               │  │
│  └──────────────┘     └────────────────┘     └───────────────┘  │
│                                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### 1.2 Chain Responsibilities

| Chain | Primary Function | Inter-Contract Needs |
|-------|-----------------|---------------------|
| **player-chain** | Character NFTs, Inventory | Query balance, Lock/unlock tokens |
| **matchmaking-chain** | Queue management, Battle creation | Create battle chains, Lock stakes |
| **battle-chain** | Turn-based combat, Damage calculation | Transfer payouts, Notify results |
| **prediction-chain** | Betting markets, Odds calculation | Transfer winnings, Query balances |
| **registry-chain** | Global stats, ELO ratings | None (receive-only) |
| **battle-token** | Fungible token (BATTLE) | None (provide services) |

---

## 2. Game Mechanics Flow

### 2.1 Complete Battle Flow

```
1. MATCHMAKING PHASE
   ┌─────────────┐
   │ Player A    │ JoinQueue(character, stake)
   │             ├────────────────────────────────┐
   └─────────────┘                                │
                                                  ↓
   ┌─────────────┐                        ┌──────────────┐
   │ Player B    │ JoinQueue(character,   │ matchmaking- │
   │             │ stake)                 │    chain     │
   └─────────────┘───────────────────────>│              │
                                          │ (Auto-match) │
                                          └──────┬───────┘
                                                 │
2. BATTLE INITIALIZATION                         │
                                                 │ create_battle_chain()
                                                 │ + Initialize message
                                                 ↓
                                          ┌──────────────┐
                                          │ battle-chain │
                                          │  (new chain) │
                                          │              │
                                          │ Player 1 HP  │
                                          │ Player 2 HP  │
                                          └──────┬───────┘
                                                 │
3. COMBAT PHASE                                  │
                                                 │
   ┌─────────────┐                              │
   │ Player A    │ SubmitTurn(stance, special)  │
   │             ├──────────────────────────────>│
   └─────────────┘                              │
                                                 │ ExecuteRound()
   ┌─────────────┐                              │ (Process combat)
   │ Player B    │ SubmitTurn(stance, special)  │
   │             ├──────────────────────────────>│
   └─────────────┘                              │
                                                 │
                                    ┌────────────┴────────────┐
                                    │                         │
                                    │ Calculate damage,       │
                                    │ Update HP,              │
                                    │ Check for winner        │
                                    │                         │
                                    └────────────┬────────────┘
                                                 │
4. BATTLE COMPLETION                             │ FinalizeBattle()
                                                 │
                    ┌────────────────────────────┼────────────────────────────┐
                    │                            │                            │
                    ↓                            ↓                            ↓
            ┌───────────────┐          ┌─────────────────┐          ┌────────────────┐
            │ battle-token  │          │ prediction-chain│          │ registry-chain │
            │               │          │                 │          │                │
            │ Transfer(     │          │ SettleMarket(   │          │ UpdateStats(   │
            │  winner,      │          │  winner)        │          │  battle_data)  │
            │  payout)      │          │                 │          │                │
            └───────────────┘          └─────────────────┘          └────────────────┘
```

### 2.2 Detailed Step-by-Step Flow

#### Step 1-2: Joining Matchmaking
```
Player Action:
  player-chain.JoinQueue(character_id, stake_amount)

What Happens:
  1. player-chain validates character and balance
  2. player-chain sends message to matchmaking-chain
  3. matchmaking-chain adds player to waiting queue
  4. When 2 players matched:
     - matchmaking-chain calls open_chain() to create battle-chain
     - sends Initialize message to battle-chain with:
       * player1: BattleParticipant { owner, chain, character, stake }
       * player2: BattleParticipant { owner, chain, character, stake }
       * battle_token_app: ApplicationId
       * platform_fee_bps: u16
```

**INTER-CONTRACT CALLS NEEDED:**
- ❌ Currently uses messages - Should validate balance via `battle-token.Balance(owner)`
- ✅ Already creates battle-chain properly
- ❌ Should call `prediction-chain.CreateMarket()` synchronously

#### Step 3: Combat Phase
```
Player Action:
  battle-chain.SubmitTurn(round, turn, stance, use_special)

What Happens:
  1. battle-chain validates turn submission
  2. When all turns submitted: ExecuteRound()
  3. battle-chain calculates damage using combat engine
  4. Updates participant HP
  5. Checks for victory condition
```

**INTER-CONTRACT CALLS NEEDED:**
- None (internal state management)

#### Step 4: Battle Completion
```
Trigger:
  battle-chain.FinalizeBattle()

What Happens:
  1. Calculate winner and loser
  2. Calculate payout (total_stake - platform_fee)
  3. Transfer tokens:
     - Platform fee → treasury
     - Winner payout → winner
  4. Notify other chains of results
```

**INTER-CONTRACT CALLS NEEDED:**
- ✅ CRITICAL: `battle-token.Transfer(winner, payout)` - Currently implemented
- ✅ CRITICAL: `battle-token.Transfer(treasury, platform_fee)` - Currently implemented
- ❌ Should send synchronous call to `prediction-chain.SettleMarket()`
- ❌ Should send message to `registry-chain.UpdateStats()`

---

## 3. Prediction Market Flow

### 3.1 Market Lifecycle

```
1. MARKET CREATION
   ┌────────────────┐
   │ matchmaking-   │ CreateMarket(battle_chain, player1, player2)
   │    chain       ├──────────────────────────────────────────────┐
   └────────────────┘                                              │
                                                                   ↓
                                                          ┌─────────────────┐
                                                          │ prediction-chain│
                                                          │                 │
                                                          │ Market OPEN     │
                                                          │ player1_pool: 0 │
                                                          │ player2_pool: 0 │
                                                          └────────┬────────┘
                                                                   │
2. BETTING PHASE                                                   │
   ┌─────────────┐                                                │
   │ Bettor A    │ PlaceBet(market_id, BetSide::Player1, amount)  │
   │             ├────────────────────────────────────────────────>│
   └─────────────┘                                                │
                                                                   │
   ┌─────────────┐                                                │
   │ Bettor B    │ PlaceBet(market_id, BetSide::Player2, amount)  │
   │             ├────────────────────────────────────────────────>│
   └─────────────┘                                                │
                                                                   │
                                              (Calculate odds,     │
                                               Update pools)       │
                                                                   │
3. MARKET CLOSES                                                   │
   ┌─────────────┐                                                │
   │battle-chain │ BattleStarted message                          │
   │             ├────────────────────────────────────────────────>│
   └─────────────┘                                                │
                                                          │ Market CLOSED  │
                                                          │ (No more bets) │
                                                                   │
4. MARKET SETTLEMENT                                               │
   ┌─────────────┐                                                │
   │battle-chain │ BattleEnded(winner_chain)                      │
   │             ├────────────────────────────────────────────────>│
   └─────────────┘                                                │
                                                          │ Market SETTLED │
                                                          │ Calc winnings  │
                                                                   │
5. PAYOUT                                                          │
   ┌─────────────┐                                                │
   │ Bettor A    │ ClaimWinnings(market_id)                       │
   │             ├────────────────────────────────────────────────>│
   └─────────────┘                                                │
                                                                   ↓
                                                          ┌─────────────────┐
                                                          │  battle-token   │
                                                          │                 │
                                                          │  Transfer(      │
                                                          │   bettor,       │
                                                          │   winnings)     │
                                                          └─────────────────┘
```

**INTER-CONTRACT CALLS NEEDED:**
- ❌ `battle-token.Transfer(bettor, winnings)` in ClaimWinnings - **CRITICAL**
- ❌ `battle-token.Balance(bettor)` in PlaceBet to validate - Optional but recommended
- ❌ `battle-token.LockFunds(bettor, amount)` during PlaceBet - **IMPORTANT**

---

## 4. Required Inter-Contract Call Implementation

### 4.1 Priority 1: CRITICAL (Blocking functionality)

#### 1. battle-chain → battle-token (Token Transfers)
**Status:** ✅ Already partially implemented
**Location:** `battle-chain/src/contract.rs:230-280`
**Current Code:**
```rust
// In FinalizeBattle operation
self.runtime.call_application::<BattleTokenAbi>(
    true,
    battle_token_app.with_abi(),
    &BattleTokenOperation::Transfer {
        to: winner_owner,
        amount: winner_payout,
    },
);
```
**Issue:** Using `with_abi()` but `BattleTokenAbi` is defined inline. Need proper ABI reference.

#### 2. prediction-chain → battle-token (Winnings Payout)
**Status:** ❌ NOT IMPLEMENTED
**Location:** `prediction-chain/src/contract.rs:572-624`
**Current Code:**
```rust
// In ClaimWinnings operation - INCOMPLETE
if let Some(battle_token_app) = self.state.battle_token_app.get().as_ref() {
    let bettor_owner = AccountOwner::from(bet.bettor);
    let transfer_op = BattleTokenOperation::Transfer {
        to: bettor_owner,
        amount: winnings,
    };

    self.runtime.call_application(
        true,
        battle_token_app.clone(),
        &transfer_op,
    );
}
```
**Issue:** BattleTokenAbi not properly defined, no response handling.

### 4.2 Priority 2: IMPORTANT (Improves reliability)

#### 3. matchmaking-chain → prediction-chain (Create Market)
**Status:** ❌ NOT IMPLEMENTED
**Location:** `matchmaking-chain/src/contract.rs:352-376`
**Current Code:**
```rust
// In create_battle_chain - Uses call_application but no response handling
if let Some(prediction_app) = self.state.prediction_app_id.get().as_ref() {
    let create_market_op = PredictionOperation::CreateMarket {
        battle_chain: battle_chain_id,
        player1_chain: pending.player1.player_chain,
        player2_chain: pending.player2.player_chain,
    };

    let result = self.runtime.call_application(
        true,
        prediction_app.clone(),
        &create_market_op,
    );
    // No proper error handling!
}
```
**Issue:** No proper ABI definition, no response validation.

#### 4. player-chain → battle-token (Balance Queries)
**Status:** ❌ NOT IMPLEMENTED
**Current Usage:**
```rust
// player-chain currently just tracks cached balance
pub battle_balance: RegisterView<Amount>,
```
**Needed:**
```rust
fn get_battle_balance(&mut self) -> Amount {
    let owner = self.runtime.authenticated_signer().unwrap();
    let battle_token_app = self.state.battle_token_app.get().unwrap();
    // CALL battle-token.Balance(owner)
}
```

### 4.3 Priority 3: NICE TO HAVE (Optimization)

#### 5. battle-chain → registry-chain (Statistics Update)
**Status:** ✅ Using messages (async) - Acceptable
**Location:** `battle-chain/src/contract.rs:433-458`
**Current:** Uses events + messages, which is appropriate for non-critical updates.

---

## 5. Implementation Plan

### 5.1 Step 1: Define Proper ABIs

#### A. Update battle-token/src/lib.rs
```rust
pub struct BattleTokenAbi;

impl ContractAbi for BattleTokenAbi {
    type Operation = Operation;
    type Response = TokenResponse;  // ADD THIS
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TokenResponse {
    Ok,
    Balance(Amount),
    TransferSuccess,
}
```

#### B. Update prediction-chain/src/lib.rs
```rust
pub struct PredictionAbi;

impl ContractAbi for PredictionAbi {
    type Operation = Operation;
    type Response = Result<PredictionResponse, String>;  // ADD THIS
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PredictionResponse {
    Ok,
    MarketCreated(u64),  // Returns market_id
}
```

### 5.2 Step 2: Update Application Parameters

#### A. battle-chain/src/lib.rs
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleParameters {
    // ... existing fields ...
    pub battle_token_app: ApplicationId,  // CHANGE TO ApplicationId<BattleTokenAbi>
}
```

#### B. prediction-chain/src/lib.rs
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionParameters {
    pub platform_fee_bps: u16,
    pub battle_token_app: ApplicationId<BattleTokenAbi>,  // ADD THIS
}
```

#### C. matchmaking-chain/src/lib.rs
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchmakingParameters {
    // ... existing fields ...
    pub prediction_app_id: Option<ApplicationId<PredictionAbi>>,  // TYPE IT
}
```

### 5.3 Step 3: Implement Helper Methods

#### In battle-chain/src/contract.rs
```rust
impl BattleContract {
    fn transfer_tokens(&mut self, to: Owner, amount: Amount) -> Result<(), BattleError> {
        let battle_token_app = self.state.battle_token_app.get()
            .ok_or(BattleError::TokenAppNotConfigured)?;

        let response = self.runtime.call_application(
            true,
            battle_token_app.with_abi::<BattleTokenAbi>(),
            &BattleTokenOperation::Transfer { to, amount },
        );

        match response {
            TokenResponse::TransferSuccess => Ok(()),
            _ => Err(BattleError::TokenTransferFailed),
        }
    }
}
```

#### In prediction-chain/src/contract.rs
```rust
impl PredictionContract {
    fn transfer_winnings(&mut self, bettor: Owner, amount: Amount) -> Result<(), PredictionError> {
        let battle_token_app = self.state.battle_token_app.get()
            .ok_or(PredictionError::TokenAppNotConfigured)?;

        let response = self.runtime.call_application(
            true,
            battle_token_app.with_abi::<BattleTokenAbi>(),
            &BattleTokenOperation::Transfer {
                to: bettor,
                amount,
            },
        );

        match response {
            TokenResponse::TransferSuccess => Ok(()),
            _ => Err(PredictionError::TransferFailed),
        }
    }
}
```

---

## 6. Testing Strategy

### 6.1 Unit Tests
- Test each helper method independently
- Mock ApplicationId and responses
- Verify error handling

### 6.2 Integration Tests
- Full battle flow: matchmaking → battle → payout
- Prediction market flow: create → bet → settle → claim
- Token transfer validation

### 6.3 End-to-End Tests
- Deploy all 6 chains to local Linera network
- Execute complete game scenario
- Verify token balances at each step
- Check prediction market settlement

---

## 7. Summary of Changes Needed

### Files to Modify:

1. **battle-token/src/lib.rs**
   - Add `TokenResponse` enum
   - Update `ContractAbi::Response` type

2. **battle-chain/src/lib.rs**
   - Import `BattleTokenAbi` properly
   - Update parameters to use typed ApplicationId

3. **battle-chain/src/contract.rs**
   - Fix `call_application` to use proper ABI
   - Add response handling
   - Add helper methods

4. **prediction-chain/src/lib.rs**
   - Add `PredictionResponse` enum
   - Update `ContractAbi::Response` type

5. **prediction-chain/src/contract.rs**
   - Implement `transfer_winnings` helper
   - Fix `ClaimWinnings` operation
   - Add proper error handling

6. **matchmaking-chain/src/lib.rs**
   - Type `prediction_app_id` properly

7. **matchmaking-chain/src/contract.rs**
   - Fix `create_battle_chain` to handle prediction market response

### Estimated Lines of Code: ~300-400 LOC
### Estimated Time: 2-3 hours
### Risk Level: Medium (changes core contract logic)

---

## Next Steps

1. ✅ Phase 1 Complete: Microcard analysis documented
2. ✅ Phase 2 Complete: Requirements identified and documented
3. 🔄 Phase 3: Implementation
   - Start with battle-token (foundation)
   - Then prediction-chain (high priority)
   - Then battle-chain (depends on token)
   - Then matchmaking-chain (orchestration)
   - Finally player-chain (optional improvements)

4. 🔄 Testing and Verification
5. 🔄 Documentation updates
6. 🔄 Commit and push

---

**Document Status:** Complete
**Last Updated:** 2025-11-20
**Author:** Claude (Analysis of battlechain-linera repository)
