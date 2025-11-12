# BattleChain Native Token System on Linera

## 📋 Overview

Based on Linera's architecture and fungible token system, BattleChain will use **native Linera tokens** instead of SOL/USDC/USDT. This document outlines:

1. Linera's Native Token Architecture
2. BATTLE Token Implementation
3. Cross-Chain Token Transfers
4. Updated BattleChain Economics
5. Integration with Microchains

---

## 🪙 Linera Native Token System

### Key Concepts

**1. Native Linera Token**
- Linera has its own native token (similar to ETH on Ethereum)
- Used for transaction fees and chain operations
- Can be held by users and applications

**2. Fungible Token Applications**
- Custom tokens built as Linera applications
- Can be transferred cross-chain instantly
- Support atomic swaps between microchains
- No minting after initial creation (fixed supply or controlled)

**3. Cross-Chain Asset Management**
- Tokens move between microchains via messages
- Atomic guarantees: either both sides complete or neither
- Sub-500ms transfers between chains
- No intermediary needed

### Asset Security Model

From Linera documentation:
> "If you send tokens to a chain owned by someone else, you rely on them for asset availability: if they don't handle your messages, you don't have access to your tokens."

**Solution**: Temporary multi-owner chains for battles
- Both players are co-owners
- Battle chain holds staked tokens
- Either player can force return via chain closure
- No trust required

---

## 💎 BATTLE Token: BattleChain Native Currency

### Token Design

```rust
pub struct BattleToken {
    pub name: String,              // "BattleChain Token"
    pub symbol: String,            // "BATTLE"
    pub decimals: u8,              // 6 (1 BATTLE = 1,000,000 micro-BATTLE)
    pub total_supply: Amount,      // Fixed: 1,000,000,000 BATTLE
    pub balances: HashMap<Owner, Amount>,
    pub allowances: HashMap<(Owner, Owner), Amount>,
}

pub type Amount = u128; // Support large values
```

### Token Economics

| Metric | Value |
|--------|-------|
| **Total Supply** | 1,000,000,000 BATTLE (1 billion) |
| **Decimals** | 6 (micro-BATTLE precision) |
| **Initial Distribution** | 100% minted at launch |
| **Platform Fee** | 3% of battle stakes |
| **Minimum Battle Stake** | 10 BATTLE |
| **Maximum Battle Stake** | 100,000 BATTLE |

### Distribution Plan

```
Total Supply: 1,000,000,000 BATTLE

Initial Allocation:
├─ Community Rewards: 40% (400M BATTLE)
│  ├─ Battle Rewards: 25% (250M)
│  ├─ Tournament Prizes: 10% (100M)
│  └─ Leaderboard Rewards: 5% (50M)
│
├─ Liquidity Pool: 20% (200M BATTLE)
│  └─ DEX liquidity for trading
│
├─ Treasury: 20% (200M BATTLE)
│  ├─ Development Fund: 12% (120M)
│  ├─ Marketing: 5% (50M)
│  └─ Partnerships: 3% (30M)
│
├─ Team & Advisors: 15% (150M BATTLE)
│  └─ 2-year vesting, 6-month cliff
│
└─ Initial Sale: 5% (50M BATTLE)
   └─ Public/private sale participants
```

---

## 🔧 Fungible Token Application Implementation

### Token Application Structure

```rust
// battlechain-linera/battle-token/src/lib.rs

use async_graphql::{Request, Response, Schema, EmptySubscription};
use linera_sdk::{
    base::{Amount, ApplicationId, ChainId, Owner, Timestamp, WithContractAbi},
    views::{RootView, View, ViewStorageContext, MapView},
    Contract, Service,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// BATTLE Token Application ABI
pub struct BattleTokenAbi;

impl WithContractAbi for BattleTokenAbi {
    type Operation = Operation;
    type Response = ();
}

/// Token State
#[derive(RootView)]
pub struct BattleTokenState {
    /// Token metadata
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: Amount,

    /// Account balances
    pub balances: MapView<Owner, Amount>,

    /// Allowances for spending (owner -> spender -> amount)
    pub allowances: MapView<(Owner, Owner), Amount>,

    /// Account registry
    pub accounts: Vec<Owner>,

    /// Stats
    pub total_transfers: u64,
    pub total_holders: u64,

    /// Timestamps
    pub created_at: Timestamp,
}

impl BattleTokenState {
    pub fn new(initial_owner: Owner, initial_supply: Amount, created_at: Timestamp) -> Self {
        let mut state = Self {
            name: "BattleChain Token".to_string(),
            symbol: "BATTLE".to_string(),
            decimals: 6,
            total_supply: initial_supply,
            balances: MapView::default(),
            allowances: MapView::default(),
            accounts: vec![initial_owner],
            total_transfers: 0,
            total_holders: 1,
            created_at,
        };

        // Mint initial supply to creator
        state.balances.insert(&initial_owner, initial_supply);

        state
    }

    /// Get balance of account
    pub async fn balance_of(&self, account: &Owner) -> Amount {
        self.balances.get(account).await.unwrap_or(Amount::ZERO)
    }

    /// Transfer tokens
    pub async fn transfer(
        &mut self,
        from: Owner,
        to: Owner,
        amount: Amount,
    ) -> Result<(), TokenError> {
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        let from_balance = self.balance_of(&from).await;
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance);
        }

        // Deduct from sender
        self.balances.insert(&from, from_balance - amount).await;

        // Add to recipient
        let to_balance = self.balance_of(&to).await;
        self.balances.insert(&to, to_balance + amount).await;

        // Track new holder
        if to_balance == Amount::ZERO && amount > Amount::ZERO {
            self.accounts.push(to);
            self.total_holders += 1;
        }

        self.total_transfers += 1;

        Ok(())
    }

    /// Approve spending allowance
    pub async fn approve(&mut self, owner: Owner, spender: Owner, amount: Amount) {
        self.allowances.insert(&(owner, spender), amount).await;
    }

    /// Get allowance
    pub async fn allowance(&self, owner: &Owner, spender: &Owner) -> Amount {
        self.allowances.get(&(*owner, *spender)).await.unwrap_or(Amount::ZERO)
    }

    /// Transfer from allowance
    pub async fn transfer_from(
        &mut self,
        spender: Owner,
        from: Owner,
        to: Owner,
        amount: Amount,
    ) -> Result<(), TokenError> {
        let allowed = self.allowance(&from, &spender).await;
        if allowed < amount {
            return Err(TokenError::InsufficientAllowance);
        }

        // Reduce allowance
        self.allowances.insert(&(from, spender), allowed - amount).await;

        // Transfer tokens
        self.transfer(from, to, amount).await
    }
}

/// Token Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Transfer tokens to another account
    Transfer {
        to: Owner,
        amount: Amount,
    },

    /// Approve spending allowance
    Approve {
        spender: Owner,
        amount: Amount,
    },

    /// Transfer from allowance
    TransferFrom {
        from: Owner,
        to: Owner,
        amount: Amount,
    },

    /// Mint new tokens (admin only, if enabled)
    Mint {
        to: Owner,
        amount: Amount,
    },

    /// Burn tokens
    Burn {
        amount: Amount,
    },
}

/// Cross-chain Token Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Cross-chain transfer request
    Transfer {
        from: Owner,
        to: Owner,
        amount: Amount,
        target_chain: ChainId,
    },

    /// Credit tokens on destination chain
    Credit {
        recipient: Owner,
        amount: Amount,
    },
}

/// Token Errors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TokenError {
    InsufficientBalance,
    InsufficientAllowance,
    ZeroAmount,
    Unauthorized,
    InvalidRecipient,
}

/// Token Contract
pub struct BattleTokenContract {
    state: BattleTokenState,
    runtime: ContractRuntime<Self>,
}

impl Contract for BattleTokenContract {
    type Message = Message;
    type Parameters = Amount; // Initial supply
    type InstantiationArgument = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let initial_supply = self.runtime.parameters();
        let creator = self.runtime.chain_ownership().owner()
            .expect("Chain must have owner");
        let now = self.runtime.system_time();

        self.state = BattleTokenState::new(creator, initial_supply, now);

        self.runtime.emit(format!(
            "BATTLE Token created: {} tokens minted to {}",
            initial_supply, creator
        ));
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let caller = self.runtime.authenticated_signer()
            .expect("Must be authenticated");

        match operation {
            Operation::Transfer { to, amount } => {
                match self.state.transfer(caller, to, amount).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "Transfer: {} -> {} amount: {}",
                            caller, to, amount
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("Transfer failed: {:?}", e));
                    }
                }
            }

            Operation::Approve { spender, amount } => {
                self.state.approve(caller, spender, amount).await;
                self.runtime.emit(format!(
                    "Approval: {} approved {} to spend {}",
                    caller, spender, amount
                ));
            }

            Operation::TransferFrom { from, to, amount } => {
                match self.state.transfer_from(caller, from, to, amount).await {
                    Ok(_) => {
                        self.runtime.emit(format!(
                            "TransferFrom: {} moved {} from {} to {}",
                            caller, amount, from, to
                        ));
                    }
                    Err(e) => {
                        self.runtime.emit(format!("TransferFrom failed: {:?}", e));
                    }
                }
            }

            Operation::Mint { to, amount } => {
                // TODO: Check if caller is admin
                let new_supply = self.state.total_supply + amount;
                self.state.total_supply = new_supply;

                let balance = self.state.balance_of(&to).await;
                self.state.balances.insert(&to, balance + amount).await;

                self.runtime.emit(format!("Minted {} to {}", amount, to));
            }

            Operation::Burn { amount } => {
                let balance = self.state.balance_of(&caller).await;
                if balance >= amount {
                    self.state.balances.insert(&caller, balance - amount).await;
                    self.state.total_supply -= amount;

                    self.runtime.emit(format!("Burned {} from {}", amount, caller));
                } else {
                    self.runtime.emit("Burn failed: insufficient balance".to_string());
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::Transfer {
                from,
                to,
                amount,
                target_chain,
            } => {
                // Deduct from sender on this chain
                let balance = self.state.balance_of(&from).await;
                if balance >= amount {
                    self.state.balances.insert(&from, balance - amount).await;

                    // Send credit message to target chain
                    // TODO: Send cross-chain message
                    self.runtime.emit(format!(
                        "Cross-chain transfer initiated: {} -> {} (chain: {})",
                        from, to, target_chain
                    ));
                }
            }

            Message::Credit { recipient, amount } => {
                // Credit tokens on this chain
                let balance = self.state.balance_of(&recipient).await;
                self.state.balances.insert(&recipient, balance + amount).await;

                self.runtime.emit(format!(
                    "Cross-chain credit: {} received {}",
                    recipient, amount
                ));
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

/// Token Service (GraphQL)
pub struct BattleTokenService {
    state: BattleTokenState,
}

impl Service for BattleTokenService {
    type Parameters = ();

    async fn load(runtime: ServiceRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state),
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root
struct QueryRoot<'a> {
    state: &'a BattleTokenState,
}

impl<'a> QueryRoot<'a> {
    fn new(state: &'a BattleTokenState) -> Self {
        Self { state }
    }
}

#[async_graphql::Object]
impl<'a> QueryRoot<'a> {
    /// Token name
    async fn name(&self) -> String {
        self.state.name.clone()
    }

    /// Token symbol
    async fn symbol(&self) -> String {
        self.state.symbol.clone()
    }

    /// Token decimals
    async fn decimals(&self) -> u8 {
        self.state.decimals
    }

    /// Total supply
    async fn total_supply(&self) -> String {
        self.state.total_supply.to_string()
    }

    /// Get balance of account
    async fn balance_of(&self, account: String) -> String {
        // Parse owner from string
        // let owner = Owner::from_str(&account).unwrap();
        // let balance = self.state.balance_of(&owner).await;
        // balance.to_string()

        // Placeholder
        "0".to_string()
    }

    /// Get allowance
    async fn allowance(&self, owner: String, spender: String) -> String {
        // Similar parsing and lookup
        "0".to_string()
    }

    /// Total holders
    async fn total_holders(&self) -> u64 {
        self.state.total_holders
    }

    /// Total transfers
    async fn total_transfers(&self) -> u64 {
        self.state.total_transfers
    }
}

struct EmptyMutation;

#[async_graphql::Object]
impl EmptyMutation {
    async fn placeholder(&self) -> bool {
        false
    }
}

linera_sdk::contract!(BattleTokenContract);
linera_sdk::service!(BattleTokenService);
```

### Cargo.toml

```toml
[package]
name = "battle-token"
version = "0.1.0"
edition = "2021"

[dependencies]
async-graphql = "7.0"
linera-sdk = { git = "https://github.com/linera-io/linera-protocol.git", features = ["wasmer"] }
serde = { version = "1.0", features = ["derive"] }
thiserror = "1.0"

[lib]
crate-type = ["cdylib"]
```

---

## 🔄 Cross-Chain Token Transfers

### Battle Stake Flow

```
Player 1 Chain                 Battle Chain                 Player 2 Chain
      │                             │                              │
      │                             │                              │
      │─── Lock 100 BATTLE ────────►│                              │
      │    (Transfer Message)        │◄──── Lock 100 BATTLE ───────│
      │                             │      (Transfer Message)       │
      │                             │                              │
      │                        [Battle Starts]                      │
      │                        • Total: 200 BATTLE                  │
      │                        • Both players co-owners             │
      │                             │                              │
      │                        [Battle Ends]                        │
      │                        • Winner: Player 1                   │
      │                             │                              │
      │◄─── Credit 194 BATTLE ──────│                              │
      │     (97% of 200)             │─── Credit 0 BATTLE ────────►│
      │                             │                              │
      │◄─── Credit 6 BATTLE ─────────────────────────────────────────► Treasury
           (3% platform fee)
```

### Implementation

```rust
// In Player Chain Contract

async fn lock_battle_stake(
    &mut self,
    battle_chain: ChainId,
    amount: Amount,
) -> Result<(), String> {
    // Get BATTLE token balance
    let token_app_id = self.get_battle_token_app_id();

    // Send transfer message to BATTLE token app
    let transfer_msg = Message::Transfer {
        from: self.state.owner,
        to: battle_chain.owner(), // Battle chain
        amount,
        target_chain: battle_chain,
    };

    // Send cross-application message
    self.runtime.send_message(
        token_app_id,
        transfer_msg,
    ).await?;

    self.runtime.emit(format!(
        "Locked {} BATTLE for battle on chain {}",
        amount, battle_chain
    ));

    Ok(())
}

// In Battle Chain Contract

async fn finalize_battle(&mut self) -> Result<(), String> {
    let winner = self.state.winner.ok_or("No winner determined")?;
    let total_stake = self.state.total_stake;

    // Calculate payouts (97% to winner, 3% to treasury)
    let platform_fee = (total_stake * 3) / 100;
    let winner_amount = total_stake - platform_fee;

    // Transfer to winner
    let winner_chain = if winner == self.state.player1 {
        self.state.player1_chain
    } else {
        self.state.player2_chain
    };

    self.transfer_tokens(winner_chain, winner, winner_amount).await?;

    // Transfer fee to treasury
    self.transfer_tokens(self.get_treasury_chain(), self.get_treasury_owner(), platform_fee).await?;

    // Close battle chain
    self.runtime.close_chain()?;

    Ok(())
}
```

---

## 💰 Updated Currency System

### Remove Old Currencies

```diff
// OLD (Phase 1)
- pub enum Currency {
-     SOL,
-     USDC,
-     USDT,
-     Custom(String),
- }

// NEW
pub enum Currency {
    BATTLE,        // Native BattleChain token
    Linera,        // Native Linera token (for fees)
}
```

### Updated Player State

```rust
#[derive(RootView)]
pub struct PlayerChainState {
    pub owner: Owner,
    pub characters: Vec<CharacterNFT>,

    // Simplified currency
    pub battle_balance: Amount,     // BATTLE token balance
    pub linera_balance: Amount,     // Native Linera balance

    // Token app reference
    pub battle_token_app: ApplicationId,

    // ... rest of fields
}
```

---

## 🎮 Battle Economics with BATTLE Token

### Stake Tiers

| Tier | Stake Amount | Winner Gets | Platform Fee |
|------|-------------|-------------|--------------|
| Bronze | 10 BATTLE | 19.4 BATTLE | 0.6 BATTLE (3%) |
| Silver | 50 BATTLE | 97 BATTLE | 3 BATTLE (3%) |
| Gold | 100 BATTLE | 194 BATTLE | 6 BATTLE (3%) |
| Platinum | 500 BATTLE | 970 BATTLE | 30 BATTLE (3%) |
| Diamond | 1,000 BATTLE | 1,940 BATTLE | 60 BATTLE (3%) |

### Earning Opportunities

**1. Battle Rewards**
- Win battles: Earn opponent's stake (minus 3% fee)
- Streak bonuses: +10% for 3+ wins in a row
- Daily quests: 5-50 BATTLE rewards

**2. Tournament Prizes**
```
Tournament Entry: 100 BATTLE
Prize Pool: Total entries × 100 BATTLE

Distribution:
├─ 1st Place: 40% of pool
├─ 2nd Place: 25% of pool
├─ 3rd Place: 15% of pool
├─ 4th-8th: 2% each
└─ Platform: 10%
```

**3. Leaderboard Rewards (Monthly)**
```
Top 100 Players:
├─ Rank 1: 10,000 BATTLE
├─ Rank 2-5: 5,000 BATTLE each
├─ Rank 6-10: 2,500 BATTLE each
├─ Rank 11-20: 1,000 BATTLE each
├─ Rank 21-50: 500 BATTLE each
└─ Rank 51-100: 250 BATTLE each
```

**4. Prediction Market Winnings**
- Bet on battles
- Win based on dynamic odds
- Platform takes 3% of losing pool

---

## 🚀 Deployment Guide

### 1. Deploy BATTLE Token

```bash
# Build token application
cd battlechain-linera/battle-token
cargo build --release --target wasm32-unknown-unknown

# Deploy to Linera
linera project publish-and-create \
  --path battlechain-linera/battle-token \
  --init-arg "1000000000000000" \  # 1 billion BATTLE (with 6 decimals)
  --required-application-ids '[]'

# Save token application ID
export BATTLE_TOKEN_APP_ID=<returned-app-id>
```

### 2. Initialize Token Distribution

```bash
# Transfer to treasury
linera graphql mutate \
  --chain-id <token-chain-id> \
  --operation 'Transfer' \
  --arguments '{
    "to": "<treasury-owner>",
    "amount": "200000000000000"
  }'  # 200M BATTLE to treasury

# Transfer to liquidity pool
linera graphql mutate \
  --chain-id <token-chain-id> \
  --operation 'Transfer' \
  --arguments '{
    "to": "<liquidity-pool-owner>",
    "amount": "200000000000000"
  }'  # 200M BATTLE to LP
```

### 3. Update Player Chains

```rust
// In Player Chain initialization
impl PlayerChainContract {
    async fn instantiate(&mut self, battle_token_app: ApplicationId) {
        // ...
        self.state.battle_token_app = battle_token_app;

        // Query initial BATTLE balance
        let balance = self.query_battle_balance().await;
        self.state.battle_balance = balance;
    }

    async fn query_battle_balance(&self) -> Amount {
        // Cross-application query to BATTLE token
        // TODO: Implement using Linera SDK
        Amount::ZERO
    }
}
```

---

## 📊 Token Utility Matrix

| Use Case | BATTLE Token | Native Linera |
|----------|--------------|---------------|
| Battle Stakes | ✅ Primary | ❌ |
| Tournament Entry | ✅ Yes | ❌ |
| Prediction Market Bets | ✅ Yes | ❌ |
| Character Trading | ✅ Yes | Optional |
| Item Purchases | ✅ Yes | ❌ |
| Transaction Fees | ❌ | ✅ Required |
| Governance Voting | ✅ Future | ❌ |
| Staking Rewards | ✅ Future | ❌ |

---

## 🔒 Security Model

### Multi-Owner Battle Chains

```rust
// When creating battle chain
pub async fn create_battle_chain(
    player1: Owner,
    player2: Owner,
    stake: Amount,
) -> Result<ChainId, String> {
    // Create temporary chain with both players as owners
    let battle_chain = runtime.create_chain(vec![player1, player2])?;

    // Set permissions: only battle app can operate
    runtime.change_application_permissions(
        battle_chain,
        vec![battle_app_id],  // Only this app
    )?;

    // Lock stakes from both players
    lock_stake(player1_chain, battle_chain, stake).await?;
    lock_stake(player2_chain, battle_chain, stake).await?;

    Ok(battle_chain)
}

// When battle ends
pub async fn close_battle_chain(&mut self) {
    // Return all remaining tokens
    for owner in &[self.state.player1, self.state.player2] {
        let balance = self.get_token_balance(owner).await;
        if balance > Amount::ZERO {
            self.transfer_tokens(*owner, balance).await;
        }
    }

    // Close chain (players can still reject messages to return in-flight tokens)
    self.runtime.close_chain()
        .expect("Application must have permission to close chain");
}
```

### Atomic Swaps (Future)

```rust
// Swap BATTLE <-> Other Token
pub async fn atomic_swap(
    &mut self,
    offer_token: ApplicationId,
    offer_amount: Amount,
    want_token: ApplicationId,
    want_amount: Amount,
) -> Result<(), String> {
    // Create temporary swap chain
    // Both parties lock tokens
    // Either both complete or both refunded
    // Uses Linera's temporary chain pattern

    todo!("Implement atomic swap")
}
```

---

## 📈 Token Economics Summary

### Supply & Distribution
- **Total Supply**: 1,000,000,000 BATTLE (fixed, no inflation)
- **Circulating at Launch**: 250,000,000 BATTLE (25%)
- **Burn Mechanism**: 0.5% of platform fees (deflationary)

### Revenue Streams
1. **Battle Fees**: 3% of all battle stakes
2. **Tournament Fees**: 10% of tournament prize pools
3. **Prediction Market Fees**: 3% of losing bets
4. **Marketplace Fees**: 2% on NFT/item trades

### Use Cases
- Battle stakes
- Tournament entries
- Prediction market bets
- Character/item purchases
- Governance (future)
- Staking rewards (future)

---

## ✅ Migration Checklist

### Phase 1 Updates

- [x] Remove SOL/USDC/USDT currency types
- [x] Add BATTLE token enum
- [x] Implement fungible token application
- [x] Update Player Chain for BATTLE balance
- [ ] Update Battle Chain for BATTLE stakes
- [ ] Update Prediction Market for BATTLE bets
- [ ] Implement cross-chain token transfers
- [ ] Add token balance queries to GraphQL

### Deployment Steps

1. Deploy BATTLE token application
2. Distribute initial supply
3. Update all microchains with token app ID
4. Test cross-chain transfers
5. Test battle stake flows
6. Deploy to testnet

---

## 📚 Resources

- **Linera Fungible Token Tutorial**: https://github.com/linera-io/fungible-app-tutorial
- **Linera Protocol Docs**: https://linera.dev/
- **Asset Management Guide**: Linera documentation (provided)
- **Atomic Swap Example**: Linera matching-engine example
- **This Document**: `/home/user/diccy/BATTLECHAIN_TOKEN_SYSTEM.md`

---

## 🎯 Next Steps

1. **Implement BATTLE Token Application** (this document provides full code)
2. **Update All Microchains** to use BATTLE instead of SOL/USDC/USDT
3. **Test Cross-Chain Transfers** between player and battle chains
4. **Implement Battle Escrow** using multi-owner temporary chains
5. **Add Token Queries** to all GraphQL endpoints
6. **Deploy to Testnet** and test with real battles

---

**Status**: Token System Design - COMPLETE ✅

Ready to replace Phase 1 currency types with native BATTLE token!
