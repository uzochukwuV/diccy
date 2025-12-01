# Diccy Improved Architecture - Majorules Pattern

## Executive Summary

**Current Problem**: 6 separate applications with complex cross-chain messaging
**Solution**: Single unified contract with enum variants (like Majorules)

## Unified Contract Architecture

```rust
pub enum DiccyContract {
    Lobby {
        state: LobbyState,
        runtime: ContractRuntime<Self>,
    },
    Battle {
        state: BattleState,
        runtime: ContractRuntime<Self>,
    },
    Player {
        state: PlayerState,
        runtime: ContractRuntime<Self>,
    },
}
```

## Chain Types & Ownership

### 1. **Lobby Chain (Single-Owner)**
- **Purpose**: Matchmaking, leaderboards, tournaments
- **Owner**: Platform admin
- **State**: Waiting players, active battles, global stats
- **Operations**: JoinQueue, CreateBattle, UpdateLeaderboard

### 2. **Battle Chain (Multi-Owner)**
- **Purpose**: Individual battle sessions
- **Owners**: Both players in the battle
- **State**: Combat state, turn submissions, battle log
- **Operations**: SubmitTurn, ExecuteRound, FinalizeBattle

### 3. **Player Chain (Single-Owner)**
- **Purpose**: NFT characters, inventory, personal stats
- **Owner**: Individual player
- **State**: Characters, battle history, token balance
- **Operations**: MintCharacter, LevelUp, ClaimRewards

## Simplified Message Flow

### Battle Creation
```
Player Chain                    Lobby Chain                    Battle Chain
     │                              │                              │
     ├──JoinQueue──────────────────>│                              │
     │                              │                              │
     │                              ├──CreateBattle──────────────>│
     │                              │  (auto-deploy)               │
     │                              │                              │
     ├<─────BattleCreated───────────┤                              │
```

### Battle Execution
```
Battle Chain (Multi-Owner)
     │
     ├──SubmitTurn (Player 1)
     ├──SubmitTurn (Player 2)
     ├──ExecuteRound
     ├──FinalizeBattle
     │
     ├──BattleResult────────────────> Player Chains
     ├──UpdateStats─────────────────> Lobby Chain
```

## Implementation

### 1. Unified State Types

```rust
// src/state.rs

/// Lobby state - matchmaking and global stats
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LobbyState {
    // Matchmaking
    pub waiting_players: MapView<AccountOwner, PlayerQueueEntry>,
    pub active_battles: MapView<ChainId, BattleMetadata>,
    
    // Global leaderboard
    pub player_stats: MapView<AccountOwner, PlayerGlobalStats>,
    pub character_registry: MapView<String, CharacterRegistryEntry>,
    
    // Platform config
    pub platform_fee_bps: RegisterView<u16>,
    pub treasury_owner: RegisterView<AccountOwner>,
    pub battle_token_balance: RegisterView<Amount>,
}

/// Battle state - individual combat session
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct BattleState {
    // Participants
    pub player1: RegisterView<BattleParticipant>,
    pub player2: RegisterView<BattleParticipant>,
    
    // Combat state
    pub status: RegisterView<BattleStatus>,
    pub current_round: RegisterView<u8>,
    pub turn_submissions: MapView<(AccountOwner, u8), TurnSubmission>,
    
    // Results
    pub round_results: RegisterView<Vec<RoundResult>>,
    pub winner: RegisterView<Option<AccountOwner>>,
    
    // Config
    pub lobby_chain_id: RegisterView<ChainId>,
    pub total_stake: RegisterView<Amount>,
    pub platform_fee_bps: RegisterView<u16>,
}

/// Player state - NFT characters and personal data
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PlayerState {
    // NFT Characters
    pub characters: MapView<String, CharacterNFT>,
    pub active_character: RegisterView<Option<String>>,
    
    // Battle history
    pub battle_history: MapView<ChainId, BattleRecord>,
    pub total_battles: RegisterView<u64>,
    pub wins: RegisterView<u64>,
    pub losses: RegisterView<u64>,
    
    // Economy
    pub battle_token_balance: RegisterView<Amount>,
    pub locked_stakes: MapView<ChainId, Amount>,
    
    // Status
    pub in_battle: RegisterView<bool>,
    pub current_battle_chain: RegisterView<Option<ChainId>>,
}
```

### 2. Unified Operations

```rust
// src/lib.rs

#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    // ========== LOBBY OPERATIONS ==========
    JoinQueue { character_id: String, stake: Amount },
    LeaveQueue,
    CreateBattle { player1: AccountOwner, player2: AccountOwner },
    
    // ========== BATTLE OPERATIONS ==========
    SubmitTurn { round: u8, turn: u8, stance: Stance, use_special: bool },
    ExecuteRound,
    FinalizeBattle,
    
    // ========== PLAYER OPERATIONS ==========
    MintCharacter { character_id: String, class: CharacterClass },
    LevelUpCharacter { character_id: String },
    SetActiveCharacter { character_id: String },
    ClaimRewards { battle_chain_id: ChainId },
    
    // ========== SHARED OPERATIONS ==========
    TransferTokens { to: AccountOwner, amount: Amount },
}
```

### 3. Unified Messages

```rust
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    // ===== LOBBY → BATTLE =====
    InitializeBattle {
        player1: BattleParticipant,
        player2: BattleParticipant,
        lobby_chain_id: ChainId,
        platform_fee_bps: u16,
    },
    
    // ===== BATTLE → PLAYER =====
    BattleResult {
        winner: AccountOwner,
        loser: AccountOwner,
        winner_payout: Amount,
        xp_gained: u64,
        battle_stats: CombatStats,
    },
    
    // ===== BATTLE → LOBBY =====
    BattleCompleted {
        winner: AccountOwner,
        loser: AccountOwner,
        rounds_played: u8,
        total_stake: Amount,
    },
    
    // ===== PLAYER → LOBBY =====
    RequestJoinQueue {
        player: AccountOwner,
        player_chain: ChainId,
        character_id: String,
        stake: Amount,
    },
}
```

### 4. Contract Implementation

```rust
// src/contract.rs

impl Contract for DiccyContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = InitializationArgument;

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        // Determine contract variant based on initialization
        if let Ok(lobby_state) = LobbyState::load(runtime.root_view_storage_context()).await {
            if lobby_state.treasury_owner.get().is_some() {
                return Self::Lobby { state: lobby_state, runtime };
            }
        }
        
        if let Ok(battle_state) = BattleState::load(runtime.root_view_storage_context()).await {
            if battle_state.lobby_chain_id.get() != ChainId::default() {
                return Self::Battle { state: battle_state, runtime };
            }
        }
        
        // Default to Player
        let state = PlayerState::load(runtime.root_view_storage_context())
            .await.expect("Failed to load player state");
        Self::Player { state, runtime }
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match self {
            Self::Lobby { state, runtime } => {
                Self::execute_lobby_operation(state, runtime, operation).await
            }
            Self::Battle { state, runtime } => {
                Self::execute_battle_operation(state, runtime, operation).await
            }
            Self::Player { state, runtime } => {
                Self::execute_player_operation(state, runtime, operation).await
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match self {
            Self::Lobby { state, runtime } => {
                Self::execute_lobby_message(state, runtime, message).await
            }
            Self::Battle { state, runtime } => {
                Self::execute_battle_message(state, runtime, message).await
            }
            Self::Player { state, runtime } => {
                Self::execute_player_message(state, runtime, message).await
            }
        }
    }
}
```

## Key Improvements

### ✅ **Simplified Architecture**
- **1 contract** instead of 6 applications
- **3 chain types** instead of 6 separate chains
- **Clear ownership model** (single/multi-owner)

### ✅ **Reduced Complexity**
- **Fewer cross-chain messages** (only when necessary)
- **Unified deployment** (one WASM binary)
- **Consistent state management**

### ✅ **Better Performance**
- **Local operations** don't require cross-chain calls
- **Batch operations** within same chain
- **Reduced gas costs**

### ✅ **Enhanced Security**
- **Unified authentication** model
- **Validated state transitions**
- **Atomic operations** within chains

### ✅ **Easier Development**
- **Single codebase** to maintain
- **Shared types** and utilities
- **Consistent testing** approach

## Migration Strategy

### Phase 1: Core Consolidation
1. **Merge shared types** into single module
2. **Create unified state types** (Lobby/Battle/Player)
3. **Implement contract enum wrapper**
4. **Basic operations** for each variant

### Phase 2: Battle System
1. **Battle creation** from lobby
2. **Turn-based combat** mechanics
3. **Prize distribution** to players
4. **Leaderboard updates**

### Phase 3: Advanced Features
1. **Character progression** system
2. **Tournament brackets**
3. **Prediction markets** (as separate optional chain)
4. **Guild systems**

## Comparison: Before vs After

### Before (Current)
```
6 Applications × 6 Chains = 36 deployment combinations
Complex cross-chain messaging for every action
Fragmented state across multiple chains
High latency and gas costs
```

### After (Improved)
```
1 Application × 3 Chain Types = 3 deployment patterns
Minimal cross-chain messaging (only when needed)
Consolidated state within appropriate chains
Low latency and gas costs
```

## Example Usage

### Deploy Lobby
```bash
linera publish-and-create \
  diccy_{contract,service}.wasm \
  --json-argument '{"variant": "Lobby", "treasury_owner": "0x...", "platform_fee_bps": 300}'
```

### Deploy Player Chain
```bash
linera publish-and-create \
  diccy_{contract,service}.wasm \
  --json-argument '{"variant": "Player", "owner": "0x..."}'
```

### Battle Auto-Created
```rust
// Lobby automatically creates battle chain when match found
let battle_chain_id = runtime.open_chain(
    ChainOwnership::multiple([player1, player2]),
    ApplicationPermissions::default(),
    Amount::ZERO
);

runtime.prepare_message(Message::InitializeBattle { ... })
    .send_to(battle_chain_id);
```

## Benefits Summary

| Aspect | Current (6 Apps) | Improved (1 App) |
|--------|------------------|------------------|
| **Deployment** | Complex (6 apps) | Simple (1 app) |
| **Messaging** | High (constant) | Low (minimal) |
| **Latency** | High (cross-chain) | Low (local) |
| **Gas Costs** | High | Low |
| **Maintenance** | Complex | Simple |
| **Testing** | Fragmented | Unified |
| **Security** | Complex | Clear |

The improved architecture follows Linera's microchain philosophy while maintaining simplicity and performance. Each chain type serves a clear purpose with minimal cross-chain dependencies.