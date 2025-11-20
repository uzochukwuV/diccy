# Microcard Repository Analysis - Inter-Contract Call Patterns

## Executive Summary

This document analyzes the microcard repository (blackjack on Linera) to understand inter-contract call patterns and architecture that can be applied to the battlechain smart contract system.

---

## 1. Contract Architecture

### Applications in microcard
1. **Bankroll** - Token management and balance tracking
2. **Blackjack** - Game logic and user interactions
3. **Public Chains** - Matchmaking and discovery

### Multi-Chain Pattern
```
┌──────────────┐         ┌───────────────┐         ┌──────────────┐
│   Master     │────────>│  Public Chain │────────>│  Play Chain  │
│   Chain      │         │  (Discovery)  │         │  (Game)      │
└──────────────┘         └───────────────┘         └──────────────┘
       │                                                    │
       │                                                    │
       └─────────────>  Bankroll Application  <────────────┘
                       (Token Management)
```

---

## 2. Inter-Contract Call Mechanisms

### 2.1 Synchronous Calls (`call_application`)

**Purpose:** Direct contract-to-contract calls with immediate responses

**Implementation Pattern:**
```rust
// Step 1: Get typed ApplicationId from parameters
let bankroll_app_id = self.runtime.application_parameters().bankroll;

// Step 2: Make the call with authentication
let response = self.runtime.call_application(
    true,                                    // authenticated
    bankroll_app_id,                        // target application
    &BankrollOperation::Balance { owner }   // operation with typed args
);

// Step 3: Handle response
match response {
    BankrollResponse::Balance(balance) => balance,
    response => panic!("Unexpected response: {response:?}"),
}
```

**Key Features:**
- **Type Safety**: `ApplicationId<BankrollAbi>` ensures type-safe calls
- **Synchronous**: Call blocks until response is received
- **Authenticated**: `true` parameter validates caller identity
- **Response Handling**: Type-safe response enum pattern matching

### 2.2 Asynchronous Messages (`prepare_message`)

**Purpose:** Cross-chain communication without blocking

**Implementation Pattern:**
```rust
fn message_manager(&mut self, destination: ChainId, message: BlackjackMessage) {
    self.runtime
        .prepare_message(message)
        .with_tracking()         // optional: track message delivery
        .send_to(destination);   // send to target chain
}
```

**Use Cases:**
- User notifications
- Event broadcasting
- State synchronization across chains
- Non-blocking updates

---

## 3. ABI Definitions

### 3.1 Contract ABI Structure
```rust
pub struct BankrollAbi;

impl ContractAbi for BankrollAbi {
    type Operation = BankrollOperation;  // Input operations
    type Response = BankrollResponse;     // Output responses
}

impl ServiceAbi for BankrollAbi {
    type Query = Request;                 // GraphQL query
    type QueryResponse = Response;        // GraphQL response
}
```

### 3.2 Typed Application Parameters
```rust
pub struct BlackjackParameters {
    pub master_chain: ChainId,
    pub public_chains: Vec<ChainId>,
    pub bankroll: ApplicationId<BankrollAbi>,  // Typed!
}
```

**Benefits:**
- Compile-time type checking
- IDE autocomplete for operations
- Prevents calling wrong operations on wrong applications

---

## 4. Security Patterns

### 4.1 Authentication
```rust
// Example: Only master chain can mint tokens
assert_eq!(
    self.runtime.chain_id(),
    self.runtime.application_parameters().master_chain,
    "MasterChain Authorization Required"
);
```

### 4.2 Caller Verification
```rust
// Get authenticated caller
let owner = self.runtime.application_id().into();
let authenticated_signer = self.runtime.authenticated_signer();
```

### 4.3 Parameter Validation
```rust
// Validate before making external calls
if amount.is_zero() {
    panic!("Amount must be greater than zero");
}
```

---

## 5. Gas Optimization Techniques

### 5.1 Batching Operations
```rust
// Instead of multiple small calls, batch operations
self.runtime.call_application(
    true,
    bankroll_app_id,
    &BankrollOperation::UpdateBalance { owner, amount }
);
```

### 5.2 Lazy State Loading
```rust
// Only load state when needed
async fn load(runtime: ContractRuntime<Self>) -> Self {
    let state = BlackjackState::load(runtime.root_view_storage_context())
        .await
        .expect("Failed to load state");
    Self { state, runtime }
}
```

### 5.3 Efficient Message Passing
```rust
// Use messages for non-critical updates
self.runtime.prepare_message(message).send_to(destination);
// vs expensive synchronous calls
```

---

## 6. Data Flow Patterns

### 6.1 Balance Query Flow
```
User Chain                     Bankroll Application
    │                                  │
    │  call_application(Balance)       │
    ├──────────────────────────────────>│
    │                                  │
    │  BankrollResponse::Balance(amt)  │
    │<──────────────────────────────────┤
    │                                  │
```

### 6.2 Token Transfer Flow
```
Play Chain                     Bankroll Application
    │                                  │
    │  call_application(UpdateBalance) │
    ├──────────────────────────────────>│
    │                                  │
    │  (Updates internal state)        │
    │                                  │
    │  BankrollResponse::Ok            │
    │<──────────────────────────────────┤
    │                                  │
```

### 6.3 Cross-Chain Message Flow
```
User Chain         Public Chain         Play Chain
    │                   │                   │
    │  FindPlayChain    │                   │
    ├──────────────────>│                   │
    │                   │  (Queries state)  │
    │                   │                   │
    │ FindPlayChainResult│                  │
    │<──────────────────┤                   │
    │                   │                   │
    │  RequestTableSeat │                   │
    ├───────────────────────────────────────>│
    │                   │                   │
    │  RequestTableSeatResult                │
    │<───────────────────────────────────────┤
```

---

## 7. Key Takeaways for Battlechain Implementation

### 7.1 Must Implement
1. **Typed ApplicationId** in Parameters structures
2. **Separate ABI definitions** with Operation and Response enums
3. **Helper methods** for common inter-contract calls
4. **Response handling** with pattern matching
5. **Authentication checks** for sensitive operations

### 7.2 Best Practices
1. Use `call_application` for **synchronous** operations requiring responses
2. Use `prepare_message` for **asynchronous** notifications and events
3. Always validate inputs before making external calls
4. Use type-safe ApplicationId to prevent errors
5. Implement proper error handling with custom error types

### 7.3 Architecture Recommendations for Battlechain

```
battle-token (Bankroll equivalent)
    ├─> Provides: Balance queries, Transfers, Minting
    └─> Called by: All other chains

battle-chain
    ├─> Calls: battle-token (for payouts)
    ├─> Sends messages: player-chain, prediction-chain, registry-chain
    └─> Receives: Player turn submissions

matchmaking-chain
    ├─> Calls: None directly
    ├─> Creates: battle-chains (via open_chain)
    └─> Sends messages: player-chains, prediction-chain

prediction-chain
    ├─> Calls: battle-token (for payouts)
    ├─> Receives messages: battle-chain (results)
    └─> Sends messages: player-chains (winnings)

registry-chain
    ├─> Calls: None
    ├─> Receives messages: battle-chain (statistics)
    └─> Tracks: Global leaderboards and ELO

player-chain
    ├─> Calls: battle-token (balance queries)
    ├─> Sends messages: matchmaking-chain (join queue)
    └─> Receives: battle-chain (battle invites), prediction-chain (winnings)
```

---

## 8. Implementation Checklist

### For Each Chain:
- [ ] Define typed ABI with Operation and Response enums
- [ ] Add ApplicationId parameters for external applications
- [ ] Implement helper methods for common inter-contract calls
- [ ] Add authentication checks for privileged operations
- [ ] Implement message handlers for cross-chain communication
- [ ] Add proper error handling and response validation
- [ ] Document all inter-contract call points
- [ ] Add logging for debugging

### Integration Testing:
- [ ] Test synchronous calls (call_application)
- [ ] Test asynchronous messages (prepare_message)
- [ ] Test authentication and authorization
- [ ] Test error handling and edge cases
- [ ] Test gas optimization under load

---

## 9. Code Examples from Microcard

### Example 1: Simple Balance Query
```rust
fn bankroll_get_balance(&mut self) -> Amount {
    let owner = self.runtime.application_id().into();
    let bankroll_app_id = self.runtime.application_parameters().bankroll;
    let response = self.runtime.call_application(
        true,
        bankroll_app_id,
        &BankrollOperation::Balance { owner }
    );
    match response {
        BankrollResponse::Balance(balance) => balance,
        response => panic!("Unexpected response: {response:?}"),
    }
}
```

### Example 2: Token Transfer
```rust
fn bankroll_update_balance(&mut self, amount: Amount) {
    let owner = self.runtime.application_id().into();
    let bankroll_app_id = self.runtime.application_parameters().bankroll;
    self.runtime.call_application(
        true,
        bankroll_app_id,
        &BankrollOperation::UpdateBalance { owner, amount }
    );
}
```

### Example 3: Cross-Chain Message
```rust
fn notify_player(&mut self, player_chain: ChainId, message: BlackjackMessage) {
    self.runtime
        .prepare_message(message)
        .with_tracking()
        .send_to(player_chain);
}
```

---

## Conclusion

The microcard repository demonstrates a clean, type-safe approach to inter-contract calls in Linera applications. The key patterns are:

1. **Type Safety**: Using typed ApplicationId for compile-time guarantees
2. **Separation of Concerns**: Clear ABIs with Operation/Response enums
3. **Dual Communication**: Synchronous calls for immediate responses, asynchronous messages for notifications
4. **Security First**: Authentication and authorization checks at every entry point
5. **Gas Efficiency**: Batching operations and using appropriate communication patterns

These patterns will be directly applied to the battlechain smart contract system to enable proper inter-chain communication for battles, predictions, and token transfers.
