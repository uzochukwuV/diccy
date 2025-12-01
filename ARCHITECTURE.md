# Improved Majority Rules Microchain Architecture

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        LOBBY CHAIN                               │
│  - Persistent multi-owner chain                                  │
│  - Collects entry fees                                           │
│  - Manages waiting players                                       │
│  - Creates temporary game chains                                 │
│  - Receives 5% platform fee on game completion                   │
└─────────────────────────────────────────────────────────────────┘
         │                    │                    │
         │ Creates            │ Creates            │ Creates
         ▼                    ▼                    ▼
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ GAME #1  │         │ GAME #2  │         │ GAME #3  │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
         │                    │                    │
         │ Sends prizes       │ Sends prizes       │ Sends prizes
         ▼                    ▼                    ▼
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 1 │         │ Player 4 │         │ Player 7 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 2 │         │ Player 5 │         │ Player 8 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 3 │         │ Player 6 │         │ Player 9 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
```

## Message Flow

### 1. Players Join Lobby (Player Chain → Lobby Chain)
```
Player 1 Chain                           Lobby Chain
      │                                        │
      │──── JoinLobby Operation ────────────→ │
      │     (transfers entry fee)              │
      │                                        │ (stores player info)
      │                                        │ (holds entry fees)
```

### 2. Game Creation (Lobby Chain → Game Chain)
```
Lobby Chain                              Game Chain (new temporary)
      │                                        │
      │──── InitializeGame Message ────────→  │
      │     (player list, entry fees)          │
      │                                        │ (sets up game state)
      │                                        │ (transfers entry fees)
```

### 3. Game Completion (Game Chain → Player Chains + Lobby Chain)
```
Game Chain                              Player Chains
      │                                        │
      │──── DistributePrize Messages ──────→  │ Player 1 Chain
      │     (95% of pool split equally)        │ Player 2 Chain
      │                                        │ Player 3 Chain
      │
      └──── Platform Fee Message ──────────→  Lobby Chain (5%)
```

## Benefits of This Architecture

### ✅ Horizontal Scalability
- Lobby can spawn unlimited concurrent games
- Each game is isolated on its own chain
- No state conflicts between games

### ✅ Clean State Separation
- Lobby state: waiting_players, game_count, lobby_owner
- Game state: players, eliminated, rounds, questions
- No `is_lobby` flag needed!

### ✅ Proper Ownership
- Lobby chain: Multi-owner (lobby creator + admins)
- Game chain: Multi-owner (all players in that game)
- Player chains: Single-owner (individual players)

### ✅ Secure Token Handling
- Entry fees flow: Player Chain → Lobby Chain → Game Chain
- Prize distribution: Game Chain → Player Chains
- Platform fee: Game Chain → Lobby Chain

### ✅ Better User Experience
- Players can join multiple games simultaneously
- Lobbies can run continuously without downtime
- Game history preserved on separate chains

## Chain Lifecycle

### Lobby Chain (Persistent)
```rust
// Created once by lobby owner
linera open-multi-owner-chain \
  --owners <LOBBY_CREATOR_KEY> \
  --multi-leader-rounds 10

// Deploy lobby application
linera publish-and-create \
  majorules_{contract,service}.wasm \
  --json-argument '{"entry_fee":"1000000000000000000", "lobby_owner":"..."}'
```

### Game Chain (Temporary, Auto-Created)
```rust
// Created automatically by lobby contract when game starts
// Multi-owner chain with all player keys
// Receives InitializeGame message from lobby
// Self-destructs or goes inactive after prizes distributed
```

### Player Chain (Persistent)
```rust
// Each player's personal chain
// Holds their tokens
// Receives prize distributions
```

## Implementation Changes Needed

### 1. Separate State Types
```rust
// src/lobby_state.rs
pub struct LobbyState {
    waiting_players: MapView<AccountOwner, ChainId>,
    entry_fee: RegisterView<Amount>,
    game_count: RegisterView<u64>,
    lobby_owner: RegisterView<AccountOwner>,
    active_games: MapView<ChainId, u64>, // Track created game chains
}

// src/game_state.rs
pub struct GameState {
    players: RegisterView<Vec<AccountOwner>>,
    eliminated: RegisterView<Vec<AccountOwner>>,
    current_round: RegisterView<u64>,
    // ... all game-specific fields
}
```

### 2. Create Game Chain from Lobby
```rust
impl LobbyContract {
    async fn start_game(&mut self) {
        let players = self.collect_waiting_players().await;
        let entry_fee = *self.state.entry_fee.get();
        
        // Create temporary multi-owner game chain
        let game_chain_id = self.runtime.open_chain(
            ChainOwnership::multi_owner(players.iter().map(|(owner, _)| *owner).collect()),
            Balance::ZERO
        );
        
        // Transfer total entry fees to game chain
        let total_fees = entry_fee.saturating_mul(players.len() as u128);
        self.runtime.transfer(
            AccountOwner::CHAIN,
            Account { chain_id: game_chain_id, owner: AccountOwner::CHAIN },
            total_fees
        );
        
        // Send initialization message to game chain
        self.runtime
            .prepare_message(Message::InitializeGame {
                players: players.iter().map(|(owner, _)| *owner).collect(),
                entry_fee,
                platform_fee_recipient: *self.state.lobby_owner.get(),
            })
            .send_to(game_chain_id);
        
        // Clear waiting players
        self.clear_waiting_list().await;
    }
}
```

### 3. Game Chain Handles Initialization
```rust
impl GameContract {
    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::InitializeGame { players, entry_fee, platform_fee_recipient } => {
                // This now actually runs on the NEW game chain!
                self.state.players.set(players.clone());
                self.state.entry_fee.set(entry_fee);
                self.state.platform_fee_recipient.set(Some(platform_fee_recipient));
                
                // Select random questioner
                let first_questioner = players[random_value(0, players.len() - 1)];
                self.state.current_questioner.set(Some(first_questioner));
                
                self.state.current_round.set(1);
                self.state.status.set(STATUS_ACTIVE);
            }
        }
    }
}
```

### 4. Prize Distribution via Messages
```rust
async fn auto_distribute_prizes(&mut self) {
    let players = self.state.players.get().clone();
    let eliminated = self.state.eliminated.get().clone();
    let entry_fee = *self.state.entry_fee.get();
    
    // Calculate prizes
    let survivors: Vec<_> = players.iter()
        .filter(|p| !eliminated.contains(p))
        .collect();
    
    let total_pool = entry_fee.saturating_mul(players.len() as u128);
    let prize_pool = total_pool.saturating_mul(95).saturating_div(100);
    let prize_per_survivor = prize_pool.saturating_div(survivors.len() as u128);
    
    // Send prizes to player chains (cross-chain!)
    for survivor in survivors {
        self.runtime
            .prepare_message(Message::DistributePrize {
                winner: *survivor,
                amount: Amount::from_attos(prize_per_survivor),
            })
            .send_to(self.get_player_chain_id(survivor).await);
    }
    
    // Send platform fee to lobby chain
    let platform_fee = total_pool.saturating_mul(5).saturating_div(100);
    self.runtime
        .prepare_message(Message::PlatformFee {
            amount: Amount::from_attos(platform_fee),
        })
        .send_to(self.state.lobby_chain_id.get());
}
```

## Advanced Features

### Leaderboard (Global State on Lobby Chain)
```rust
// Lobby chain tracks global statistics
pub struct LobbyState {
    // ...existing fields
    global_games_played: MapView<AccountOwner, u64>,
    global_games_won: MapView<AccountOwner, u64>,
    global_total_winnings: MapView<AccountOwner, Amount>,
}

// Game chain sends results to lobby at end
Message::GameResults {
    winners: Vec<AccountOwner>,
    prize_per_winner: Amount,
}
```

### Tournament Mode (Chain Hierarchy)
```
Tournament Lobby Chain
       │
       ├─ Round 1 Games (8 games, 50 players each)
       │   └─ Winners → Round 2
       ├─ Round 2 Games (4 games)
       │   └─ Winners → Round 3
       └─ Finals (1 game, top 50 players)
```

### Tiered Lobbies (Multiple Persistent Lobbies)
```
Free Lobby Chain (0.001 tokens)
Bronze Lobby Chain (0.01 tokens)
Silver Lobby Chain (0.1 tokens)
Gold Lobby Chain (1 token)
Diamond Lobby Chain (10 tokens)
```



# Improved Majority Rules Microchain Architecture

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        LOBBY CHAIN                               │
│  - Persistent multi-owner chain                                  │
│  - Collects entry fees                                           │
│  - Manages waiting players                                       │
│  - Creates temporary game chains                                 │
│  - Receives 5% platform fee on game completion                   │
└─────────────────────────────────────────────────────────────────┘
         │                    │                    │
         │ Creates            │ Creates            │ Creates
         ▼                    ▼                    ▼
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ GAME #1  │         │ GAME #2  │         │ GAME #3  │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
         │                    │                    │
         │ Sends prizes       │ Sends prizes       │ Sends prizes
         ▼                    ▼                    ▼
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 1 │         │ Player 4 │         │ Player 7 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 2 │         │ Player 5 │         │ Player 8 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
   ┌──────────┐         ┌──────────┐         ┌──────────┐
   │ Player 3 │         │ Player 6 │         │ Player 9 │
   │ Chain    │         │ Chain    │         │ Chain    │
   └──────────┘         └──────────┘         └──────────┘
```

## Message Flow

### 1. Players Join Lobby (Player Chain → Lobby Chain)
```
Player 1 Chain                           Lobby Chain
      │                                        │
      │──── JoinLobby Operation ────────────→ │
      │     (transfers entry fee)              │
      │                                        │ (stores player info)
      │                                        │ (holds entry fees)
```

### 2. Game Creation (Lobby Chain → Game Chain)
```
Lobby Chain                              Game Chain (new temporary)
      │                                        │
      │──── InitializeGame Message ────────→  │
      │     (player list, entry fees)          │
      │                                        │ (sets up game state)
      │                                        │ (transfers entry fees)
```

### 3. Game Completion (Game Chain → Player Chains + Lobby Chain)
```
Game Chain                              Player Chains
      │                                        │
      │──── DistributePrize Messages ──────→  │ Player 1 Chain
      │     (95% of pool split equally)        │ Player 2 Chain
      │                                        │ Player 3 Chain
      │
      └──── Platform Fee Message ──────────→  Lobby Chain (5%)
```

## Benefits of This Architecture

### ✅ Horizontal Scalability
- Lobby can spawn unlimited concurrent games
- Each game is isolated on its own chain
- No state conflicts between games

### ✅ Clean State Separation
- Lobby state: waiting_players, game_count, lobby_owner
- Game state: players, eliminated, rounds, questions
- No `is_lobby` flag needed!

### ✅ Proper Ownership
- Lobby chain: Multi-owner (lobby creator + admins)
- Game chain: Multi-owner (all players in that game)
- Player chains: Single-owner (individual players)

### ✅ Secure Token Handling
- Entry fees flow: Player Chain → Lobby Chain → Game Chain
- Prize distribution: Game Chain → Player Chains
- Platform fee: Game Chain → Lobby Chain

### ✅ Better User Experience
- Players can join multiple games simultaneously
- Lobbies can run continuously without downtime
- Game history preserved on separate chains

## Chain Lifecycle

### Lobby Chain (Persistent)
```rust
// Created once by lobby owner
linera open-multi-owner-chain \
  --owners <LOBBY_CREATOR_KEY> \
  --multi-leader-rounds 10

// Deploy lobby application
linera publish-and-create \
  majorules_{contract,service}.wasm \
  --json-argument '{"entry_fee":"1000000000000000000", "lobby_owner":"..."}'
```

### Game Chain (Temporary, Auto-Created)
```rust
// Created automatically by lobby contract when game starts
// Multi-owner chain with all player keys
// Receives InitializeGame message from lobby
// Self-destructs or goes inactive after prizes distributed
```

### Player Chain (Persistent)
```rust
// Each player's personal chain
// Holds their tokens
// Receives prize distributions
```

## Implementation Changes Needed

### 1. Separate State Types
```rust
// src/lobby_state.rs
pub struct LobbyState {
    waiting_players: MapView<AccountOwner, ChainId>,
    entry_fee: RegisterView<Amount>,
    game_count: RegisterView<u64>,
    lobby_owner: RegisterView<AccountOwner>,
    active_games: MapView<ChainId, u64>, // Track created game chains
}

// src/game_state.rs
pub struct GameState {
    players: RegisterView<Vec<AccountOwner>>,
    eliminated: RegisterView<Vec<AccountOwner>>,
    current_round: RegisterView<u64>,
    // ... all game-specific fields
}
```

### 2. Create Game Chain from Lobby
```rust
impl LobbyContract {
    async fn start_game(&mut self) {
        let players = self.collect_waiting_players().await;
        let entry_fee = *self.state.entry_fee.get();
        
        // Create temporary multi-owner game chain
        let game_chain_id = self.runtime.open_chain(
            ChainOwnership::multi_owner(players.iter().map(|(owner, _)| *owner).collect()),
            Balance::ZERO
        );
        
        // Transfer total entry fees to game chain
        let total_fees = entry_fee.saturating_mul(players.len() as u128);
        self.runtime.transfer(
            AccountOwner::CHAIN,
            Account { chain_id: game_chain_id, owner: AccountOwner::CHAIN },
            total_fees
        );
        
        // Send initialization message to game chain
        self.runtime
            .prepare_message(Message::InitializeGame {
                players: players.iter().map(|(owner, _)| *owner).collect(),
                entry_fee,
                platform_fee_recipient: *self.state.lobby_owner.get(),
            })
            .send_to(game_chain_id);
        
        // Clear waiting players
        self.clear_waiting_list().await;
    }
}
```

### 3. Game Chain Handles Initialization
```rust
impl GameContract {
    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::InitializeGame { players, entry_fee, platform_fee_recipient } => {
                // This now actually runs on the NEW game chain!
                self.state.players.set(players.clone());
                self.state.entry_fee.set(entry_fee);
                self.state.platform_fee_recipient.set(Some(platform_fee_recipient));
                
                // Select random questioner
                let first_questioner = players[random_value(0, players.len() - 1)];
                self.state.current_questioner.set(Some(first_questioner));
                
                self.state.current_round.set(1);
                self.state.status.set(STATUS_ACTIVE);
            }
        }
    }
}
```

### 4. Prize Distribution via Messages
```rust
async fn auto_distribute_prizes(&mut self) {
    let players = self.state.players.get().clone();
    let eliminated = self.state.eliminated.get().clone();
    let entry_fee = *self.state.entry_fee.get();
    
    // Calculate prizes
    let survivors: Vec<_> = players.iter()
        .filter(|p| !eliminated.contains(p))
        .collect();
    
    let total_pool = entry_fee.saturating_mul(players.len() as u128);
    let prize_pool = total_pool.saturating_mul(95).saturating_div(100);
    let prize_per_survivor = prize_pool.saturating_div(survivors.len() as u128);
    
    // Send prizes to player chains (cross-chain!)
    for survivor in survivors {
        self.runtime
            .prepare_message(Message::DistributePrize {
                winner: *survivor,
                amount: Amount::from_attos(prize_per_survivor),
            })
            .send_to(self.get_player_chain_id(survivor).await);
    }
    
    // Send platform fee to lobby chain
    let platform_fee = total_pool.saturating_mul(5).saturating_div(100);
    self.runtime
        .prepare_message(Message::PlatformFee {
            amount: Amount::from_attos(platform_fee),
        })
        .send_to(self.state.lobby_chain_id.get());
}
```

## Advanced Features

### Leaderboard (Global State on Lobby Chain)
```rust
// Lobby chain tracks global statistics
pub struct LobbyState {
    // ...existing fields
    global_games_played: MapView<AccountOwner, u64>,
    global_games_won: MapView<AccountOwner, u64>,
    global_total_winnings: MapView<AccountOwner, Amount>,
}

// Game chain sends results to lobby at end
Message::GameResults {
    winners: Vec<AccountOwner>,
    prize_per_winner: Amount,
}
```

### Tournament Mode (Chain Hierarchy)
```
Tournament Lobby Chain
       │
       ├─ Round 1 Games (8 games, 50 players each)
       │   └─ Winners → Round 2
       ├─ Round 2 Games (4 games)
       │   └─ Winners → Round 3
       └─ Finals (1 game, top 50 players)
```

### Tiered Lobbies (Multiple Persistent Lobbies)
```
Free Lobby Chain (0.001 tokens)
Bronze Lobby Chain (0.01 tokens)
Silver Lobby Chain (0.1 tokens)
Gold Lobby Chain (1 token)
Diamond Lobby Chain (10 tokens)
```



# Microchain Architecture: Current vs Improved

## Executive Summary

Your current implementation violates the core principle of Linera's microchain architecture by using a **single chain that morphs from lobby to game**. The improved architecture uses **separate chains for lobby and games**, enabling proper horizontal scaling and concurrent gameplay.

## Critical Issues in Current Architecture

### ❌ Issue 1: State Pollution
```rust
// Current: Single state with everything mixed
pub struct MajorulesState {
    pub is_lobby: RegisterView<bool>,  // ❌ Shouldn't need this flag!
    pub waiting_players: MapView<...>, // Lobby data
    pub players: RegisterView<...>,     // Game data
    pub eliminated: RegisterView<...>,  // Game data
    // 20+ more fields...
}
```

**Problems:**
- Lobby and game data mixed on same chain
- `is_lobby` flag is a code smell
- Can't have multiple concurrent games
- State grows unbounded

### ❌ Issue 2: Unused Cross-Chain Messages
```rust
Message::InitializeGame { ... } => {
    // ❌ This code NEVER runs!
    // You define the message but never send it
}
```

You prepared for cross-chain messaging but then bypassed it:
```rust
async fn start_game_from_lobby(&mut self) {
    // ❌ Transforms same chain instead of creating new one
    self.state.is_lobby.set(false);
    // ... initializes game on SAME chain
}
```

### ❌ Issue 3: No Horizontal Scalability
```
Current Architecture:
┌────────────┐
│   Chain    │ ← Lobby
└────────────┘
      ↓ (transforms)
┌────────────┐
│   Chain    │ ← Game 1
└────────────┘
      ↓ (can't start Game 2!)
```

**Consequences:**
- Only ONE game can run at a time
- Lobby goes offline during games
- Can't run concurrent tournaments

### ❌ Issue 4: Prize Distribution Bug
```rust
async fn auto_distribute_prizes(&mut self) {
    // ❌ Transfers prizes to GAME CHAIN, not PLAYER CHAINS!
    let to_account = Account {
        chain_id: game_chain,  // ❌ Should be player_chain!
        owner: *player,
    };
    self.runtime.transfer(AccountOwner::CHAIN, to_account, prize_share);
}
```

Players receive prizes on the **game chain**, not their **personal chains** where they can actually use the tokens!

---

## Improved Architecture

### ✅ Pattern: Persistent Lobby + Temporary Games

```
Lobby Chain (Persistent Multi-Owner)
    │
    ├─→ Game Chain 1 (Temp Multi-Owner) → Winners
    ├─→ Game Chain 2 (Temp Multi-Owner) → Winners
    ├─→ Game Chain 3 (Temp Multi-Owner) → Winners
    └─→ Game Chain N (Temp Multi-Owner) → Winners
```

### ✅ Benefit 1: Clean State Separation

```rust
// Lobby state - only lobby concerns
pub struct LobbyState {
    pub waiting_players: MapView<AccountOwner, ChainId>,
    pub entry_fee: RegisterView<Amount>,
    pub game_count: RegisterView<u64>,
    pub lobby_owner: RegisterView<AccountOwner>,
    pub active_games: MapView<ChainId, Timestamp>,
}

// Game state - only game concerns
pub struct GameState {
    pub players: RegisterView<Vec<AccountOwner>>,
    pub eliminated: RegisterView<Vec<AccountOwner>>,
    pub current_round: RegisterView<u64>,
    // ... only game-related fields
}
```

No more `is_lobby` flag! The state type itself tells you what kind of chain it is.

### ✅ Benefit 2: Actual Cross-Chain Messaging

```rust
// Lobby creates game
async fn start_game(state: &mut LobbyState, runtime: &ContractRuntime) {
    // 1. Create new game chain
    let game_chain_id = runtime.open_chain(
        ChainOwnership::multi_owner(players.clone()),
        Balance::ZERO
    );
    
    // 2. Transfer entry fees to game chain
    runtime.transfer(
        AccountOwner::CHAIN,
        Account { chain_id: game_chain_id, owner: AccountOwner::CHAIN },
        total_fees
    );
    
    // 3. Send initialization message
    runtime.prepare_message(Message::InitializeGame { ... })
        .send_to(game_chain_id);
}

// Game receives message and initializes
async fn execute_message(&mut self, message: Message) {
    match message {
        Message::InitializeGame { players, ... } => {
            // ✅ This actually runs now!
            self.state.players.set(players);
            self.state.current_round.set(1);
            // ...
        }
    }
}
```

### ✅ Benefit 3: Concurrent Games

```rust
// Multiple games can run simultaneously
Lobby Chain
    │
    ├─→ Game #1 (Round 2, 7 survivors)  } Concurrent!
    ├─→ Game #2 (Round 1, 10 players)   } Concurrent!
    └─→ Game #3 (Round 3, 3 survivors)  } Concurrent!
```

Players can:
- Join multiple lobbies
- Play multiple games simultaneously
- Check leaderboards across all games

### ✅ Benefit 4: Correct Prize Distribution

```rust
async fn distribute_prizes(state: &mut GameState, runtime: &ContractRuntime) {
    for survivor in &survivors {
        // Get player's PERSONAL chain ID
        let player_chain = state.player_chains.get(survivor).await
            .expect("Player chain not found");
        
        // ✅ Send to player's chain, not game chain!
        runtime.prepare_message(Message::DistributePrize {
            winner: *survivor,
            amount: prize_per_survivor,
        })
        .send_to(player_chain);  // ✅ Correct!
    }
    
    // Send platform fee to lobby
    runtime.prepare_message(Message::PlatformFee { amount })
        .send_to(lobby_chain_id);
}
```

---

## Migration Path

### Step 1: Split State Types

**Before:**
```rust
pub struct MajorulesState {
    pub is_lobby: RegisterView<bool>,
    // 20 fields...
}
```

**After:**
```rust
pub struct LobbyState {
    // 5 lobby-specific fields
}

pub struct GameState {
    // 15 game-specific fields
}

pub enum MajorulesContract {
    Lobby { state: LobbyState, runtime: ContractRuntime<Self> },
    Game { state: GameState, runtime: ContractRuntime<Self> },
}
```

### Step 2: Implement Game Chain Creation

```rust
// When enough players join lobby
async fn start_game(state: &mut LobbyState, runtime: &ContractRuntime) {
    let players = collect_waiting_players(state).await;
    
    // Create new multi-owner game chain
    let game_chain_id = runtime.open_chain(
        ChainOwnership::multi_owner(players.clone()),
        Balance::ZERO
    );
    
    // Transfer collected entry fees
    let total_fees = entry_fee.saturating_mul(players.len() as u128);
    runtime.transfer(
        AccountOwner::CHAIN,
        Account { chain_id: game_chain_id, owner: AccountOwner::CHAIN },
        total_fees
    );
    
    // Initialize game via message
    runtime.prepare_message(Message::InitializeGame {
        players: players.clone(),
        player_chains: player_chain_mappings,
        entry_fee,
        lobby_chain_id: runtime.chain_id(),
        platform_fee_recipient: *state.lobby_owner.get(),
    })
    .send_to(game_chain_id);
    
    // Clear lobby
    clear_waiting_players(state).await;
}
```

### Step 3: Fix Prize Distribution

```rust
// Store player chain mappings when game starts
pub struct GameState {
    // ...
    pub player_chains: MapView<AccountOwner, ChainId>,
}

// Use them for prize distribution
async fn distribute_prizes(state: &mut GameState, runtime: &ContractRuntime) {
    for survivor in &survivors {
        let player_chain = state.player_chains.get(survivor).await.unwrap();
        
        runtime.prepare_message(Message::DistributePrize {
            winner: *survivor,
            amount: prize,
        })
        .send_to(player_chain);  // ✅ To player's chain!
    }
}
```

### Step 4: Handle Messages Properly

```rust
impl Contract for MajorulesContract {
    async fn execute_message(&mut self, message: Message) {
        match self {
            Self::Lobby { state, runtime } => {
                match message {
                    Message::GameResults { winners, prize_per_winner } => {
                        // Update global leaderboard
                        update_leaderboard(state, winners, prize_per_winner).await;
                    }
                    Message::PlatformFee { amount } => {
                        // Transfer to lobby owner
                        let owner = *state.lobby_owner.get();
                        runtime.transfer(
                            AccountOwner::CHAIN,
                            Account { chain_id: runtime.chain_id(), owner },
                            amount
                        );
                    }
                    _ => {}
                }
            }
            Self::Game { state, runtime } => {
                match message {
                    Message::InitializeGame { players, player_chains, ... } => {
                        // ✅ This now actually runs!
                        state.players.set(players);
                        for (player, chain) in player_chains {
                            state.player_chains.insert(&player, chain).unwrap();
                        }
                        // ...
                    }
                    _ => {}
                }
            }
        }
    }
}
```

---

## Advanced Features Enabled by Proper Architecture

### 1. Tournament Mode
```
Tournament Lobby
    │
    ├─→ Bracket 1 (8 games) → 8 winners
    ├─→ Bracket 2 (4 games) → 4 winners
    ├─→ Bracket 3 (2 games) → 2 winners
    └─→ Finals (1 game) → 1 champion
```

### 2. Tiered Lobbies
```
Free Lobby (0.001 tokens)
    ├─→ Game #1, #2, #3...

Bronze Lobby (0.01 tokens)
    ├─→ Game #10, #11, #12...

Gold Lobby (1 token)
    ├─→ Game #100, #101, #102...
```

### 3. Global Leaderboard
```rust
// Lobby chain aggregates stats from all games
pub struct LobbyState {
    // ...
    pub global_games_played: MapView<AccountOwner, u64>,
    pub global_games_won: MapView<AccountOwner, u64>,
    pub global_winnings: MapView<AccountOwner, Amount>,
    pub global_eliminations: MapView<AccountOwner, u64>,
}

// Games send results back to lobby
Message::GameResults {
    winners: Vec<AccountOwner>,
    prize_per_winner: Amount,
}
```

### 4. Spectator Mode
```rust
// Anyone can query game chains
query {
  game(chainId: "0xabc...") {
    currentRound
    survivors
    question
    optionA
    optionB
    optionC
  }
}
```

### 5. Replay System
```rust
// Game chains preserve full history
pub struct GameState {
    // ...
    pub round_history: MapView<u64, RoundData>,
}

#[derive(Serialize, Deserialize)]
pub struct RoundData {
    pub question: String,
    pub options: [String; 3],
    pub votes: Vec<(AccountOwner, u8)>,
    pub eliminated: Vec<AccountOwner>,
}
```

---

## Testing Strategy

### Current (Broken)
```rust
#[test]
fn test_game() {
    let mut chain = validator.new_chain().await;
    
    // Create application
    chain.create_application(module_id, (), 0, vec![]).await;
    
    // Players join
    chain.add_block(|b| b.with_operation(app_id, Operation::JoinLobby)).await;
    chain.add_block(|b| b.with_operation(app_id, Operation::JoinLobby)).await;
    
    // ❌ Game starts on SAME chain - can't test concurrent games!
}
```

### Improved (Correct)
```rust
#[test]
fn test_concurrent_games() {
    let mut lobby_chain = validator.new_chain().await;
    
    // Create lobby
    let lobby_app = lobby_chain.create_application(
        module_id,
        (),
        InitializationArgument::Lobby { ... },
        vec![]
    ).await;
    
    // Players 1-3 join
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    
    // ✅ Game 1 created on NEW chain automatically
    let game1_chain_id = get_latest_game_chain(lobby_chain).await;
    
    // Players 4-6 join lobby again
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    lobby_chain.add_block(|b| b.with_operation(lobby_app, Operation::JoinLobby)).await;
    
    // ✅ Game 2 created on ANOTHER new chain
    let game2_chain_id = get_latest_game_chain(lobby_chain).await;
    
    // ✅ Both games run concurrently!
    assert_ne!(game1_chain_id, game2_chain_id);
    assert_ne!(game1_chain_id, lobby_chain.id());
}
```

---

## Implementation Checklist

### Phase 1: State Separation
- [ ] Create `LobbyState` struct
- [ ] Create `GameState` struct
- [ ] Create `MajorulesContract` enum wrapper
- [ ] Update `load()` to return correct variant
- [ ] Split `instantiate()` by variant

### Phase 2: Cross-Chain Messaging
- [ ] Implement `start_game()` with `runtime.open_chain()`
- [ ] Transfer entry fees to game chain
- [ ] Send `InitializeGame` message
- [ ] Handle `InitializeGame` in game contract
- [ ] Store player chain mappings

### Phase 3: Prize Distribution
- [ ] Fix `auto_distribute_prizes()` to use player chains
- [ ] Send `DistributePrize` messages to player chains
- [ ] Send `PlatformFee` to lobby chain
- [ ] Send `GameResults` to lobby for leaderboard

### Phase 4: Testing
- [ ] Test concurrent game creation
- [ ] Test cross-chain prize distribution
- [ ] Test platform fee transfer
- [ ] Test leaderboard updates

### Phase 5: Advanced Features
- [ ] Global leaderboard on lobby chain
- [ ] Tournament bracket system
- [ ] Tiered lobbies (Free, Bronze, Gold)
- [ ] Spectator mode queries
- [ ] Replay system

---

## Key Takeaways

### What You Did Wrong ❌
1. **Used single chain** that morphs from lobby → game
2. **Defined cross-chain messages** but never sent them
3. **Mixed lobby and game state** in one struct
4. **Distributed prizes to game chain** instead of player chains
5. **Can't run concurrent games** from one lobby

### What You Should Do ✅
1. **Separate chains**: Lobby (persistent) + Games (temporary)
2. **Actually use messages**: Send `InitializeGame` to new chains
3. **Split state types**: `LobbyState` vs `GameState`
4. **Fix prize flow**: Game → Player Chains (not Game → Game)
5. **Enable concurrency**: Multiple games from one lobby

### The Linera Way 🚀
> "An arbitrary number of microchains can coexist in a Linera network... Creating a new microchain only takes one transaction on an existing chain."

Your game should **embrace this**! Each game session is a new microchain. This is what makes Linera special - horizontal scaling by design.

---

## References

- [Linera Microchains](https://docs.linera.io/developers/core_concepts/microchains)
- [Temporary Chains Pattern](https://docs.linera.io/developers/advanced_topics/common_patterns#using-temporary-chains-to-scale-applications)
- [Cross-Chain Messaging](https://docs.linera.io/developers/advanced_topics/messages)
- [Hex Game Example](https://github.com/linera-io/linera-protocol/tree/main/examples/hex-game) - Similar pattern!


linera_sdk::contract
Struct ContractRuntime Copy item path
Source
Search
Settings
Help

Summary
pub struct ContractRuntime<Application>
where
    Application: Contract,
{ /* private fields */ }
The common runtime to interface with the host executing the contract.

It automatically caches read-only values received from the host.

Implementations
Source
impl<Application> ContractRuntime<Application>
where
    Application: Contract,
Source
pub fn key_value_store(&self) -> KeyValueStore
Returns the key-value store to interface with storage.

Source
pub fn root_view_storage_context(&self) -> ViewStorageContext
Returns a storage context suitable for a root view.

Source
impl<Application> ContractRuntime<Application>
where
    Application: Contract,
Source
pub fn application_parameters(&mut self) -> Application::Parameters
Returns the application parameters provided when the application was created.

Source
pub fn application_id(&mut self) -> ApplicationId<Application::Abi>
Returns the ID of the current application.

Source
pub fn application_creator_chain_id(&mut self) -> ChainId
Returns the chain ID of the current application creator.

Source
pub fn chain_id(&mut self) -> ChainId
Returns the ID of the current chain.

Source
pub fn block_height(&mut self) -> BlockHeight
Returns the height of the current block that is executing.

Source
pub fn system_time(&mut self) -> Timestamp
Retrieves the current system time, i.e. the timestamp of the block in which this is called.

Source
pub fn chain_balance(&mut self) -> Amount
Returns the current chain balance.

Source
pub fn owner_balance(&mut self, owner: AccountOwner) -> Amount
Returns the balance of one of the accounts on this chain.

Source
pub fn chain_ownership(&mut self) -> ChainOwnership
Retrieves the owner configuration for the current chain.

Source
pub fn http_request(&mut self, request: Request) -> Response
Makes an HTTP request as an oracle and returns the HTTP response.

Should only be used with queries where it is very likely that all validators will receive the same response, otherwise most block proposals will fail.

Cannot be used in fast blocks: A block using this call should be proposed by a regular owner, not a super owner.

Source
pub fn assert_before(&mut self, timestamp: Timestamp)
Panics if the current time at block validation is >= timestamp. Note that block validation happens at or after the block timestamp, but isn’t necessarily the same.

Cannot be used in fast blocks: A block using this call should be proposed by a regular owner, not a super owner.

Source
pub fn read_data_blob(&mut self, hash: DataBlobHash) -> Vec<u8> ⓘ
Reads a data blob with the given hash from storage.

Source
pub fn assert_data_blob_exists(&mut self, hash: DataBlobHash)
Asserts that a data blob with the given hash exists in storage.

Source
impl<Application> ContractRuntime<Application>
where
    Application: Contract,
Source
pub fn authenticated_signer(&mut self) -> Option<AccountOwner>
Returns the authenticated signer for this execution, if there is one.

Source
pub fn message_is_bouncing(&mut self) -> Option<bool>
Returns true if the incoming message was rejected from the original destination and is now bouncing back, or None if not executing an incoming message.

Source
pub fn message_origin_chain_id(&mut self) -> Option<ChainId>
Returns the chain ID where the incoming message originated from, or None if not executing an incoming message.

Source
pub fn authenticated_caller_id(&mut self) -> Option<ApplicationId>
Returns the authenticated caller ID, if the caller configured it and if the current context is executing a cross-application call.

Source
pub fn check_account_permission(
    &mut self,
    owner: AccountOwner,
) -> Result<(), AccountPermissionError>
Verifies that the current execution context authorizes operations on a given account.

Source
pub fn send_message(
    &mut self,
    destination: ChainId,
    message: Application::Message,
)
Schedules a message to be sent to this application on another chain.

Source
pub fn prepare_message(
    &mut self,
    message: Application::Message,
) -> MessageBuilder<Application::Message>
Returns a MessageBuilder to prepare a message to be sent.

Source
pub fn transfer(
    &mut self,
    source: AccountOwner,
    destination: Account,
    amount: Amount,
)
Transfers an amount of native tokens from source owner account (or the current chain’s balance) to destination.

Source
pub fn claim(&mut self, source: Account, destination: Account, amount: Amount)
Claims an amount of native tokens from a source account to a destination account.

Source
pub fn call_application<A: ContractAbi + Send>(
    &mut self,
    authenticated: bool,
    application: ApplicationId<A>,
    call: &A::Operation,
) -> A::Response
Calls another application.

Source
pub fn emit(&mut self, name: StreamName, value: &Application::EventValue) -> u32
Adds a new item to an event stream. Returns the new event’s index in the stream.

Source
pub fn read_event(
    &mut self,
    chain_id: ChainId,
    name: StreamName,
    index: u32,
) -> Application::EventValue
Reads an event from a stream. Returns the event’s value.

Fails the block if the event doesn’t exist.

Source
pub fn subscribe_to_events(
    &mut self,
    chain_id: ChainId,
    application_id: ApplicationId,
    name: StreamName,
)
Subscribes this application to an event stream.

Source
pub fn unsubscribe_from_events(
    &mut self,
    chain_id: ChainId,
    application_id: ApplicationId,
    name: StreamName,
)
Unsubscribes this application from an event stream.

Source
pub fn query_service<A: ServiceAbi + Send>(
    &mut self,
    application_id: ApplicationId<A>,
    query: A::Query,
) -> A::QueryResponse
Queries an application service as an oracle and returns the response.

Should only be used with queries where it is very likely that all validators will compute the same result, otherwise most block proposals will fail.

Cannot be used in fast blocks: A block using this call should be proposed by a regular owner, not a super owner.

Source
pub fn open_chain(
    &mut self,
    chain_ownership: ChainOwnership,
    application_permissions: ApplicationPermissions,
    balance: Amount,
) -> ChainId
Opens a new chain, configuring it with the provided chain_ownership, application_permissions and initial balance (debited from the current chain).

Source
pub fn close_chain(&mut self) -> Result<(), CloseChainError>
Closes the current chain. Returns an error if the application doesn’t have permission to do so.

Source
pub fn change_application_permissions(
    &mut self,
    application_permissions: ApplicationPermissions,
) -> Result<(), ChangeApplicationPermissionsError>
Changes the application permissions for the current chain.

Source
pub fn create_application<Abi, Parameters, InstantiationArgument>(
    &mut self,
    module_id: ModuleId,
    parameters: &Parameters,
    argument: &InstantiationArgument,
    required_application_ids: Vec<ApplicationId>,
) -> ApplicationId<Abi>
where
    Abi: ContractAbi,
    Parameters: Serialize,
    InstantiationArgument: Serialize,
Creates a new on-chain application, based on the supplied module and parameters.

Source
pub fn create_data_blob(&mut self, bytes: Vec<u8>) -> DataBlobHash
Creates a new data blob and returns its hash.

Source
pub fn publish_module(
    &mut self,
    contract: Bytecode,
    service: Bytecode,
    vm_runtime: VmRuntime,
) -> ModuleId
Publishes a module with contract and service bytecode and returns the module ID.

Source
pub fn validation_round(&mut self) -> Option<u32>
Returns the round in which this block was validated.

Trait Implementations
Source
impl<Application> Debug for ContractRuntime<Application>
where
    Application: Contract + Debug,
    Application::Parameters: Debug,
    Application::Abi: Debug,
Source
fn fmt(&self, f: &mut Formatter<'_>) -> Result
Formats the value using the given formatter. Read more
Auto Trait Implementations



# Linera Tutorial: Building Your First Interactive Application

This tutorial will teach you the core concepts of Linera blockchain development through hands-on examples. You'll learn about single/multi-owner chains, cross-chain messaging, and randomness by building a simple counter application.

## Table of Contents
1. [Prerequisites](#prerequisites)
2. [Understanding the Counter Application](#understanding-the-counter-application)
3. [Single-Owner vs Multi-Owner Chains](#single-owner-vs-multi-owner-chains)
4. [Cross-Chain Messaging](#cross-chain-messaging)
5. [Randomness](#randomness)
6. [Common Errors and Solutions](#common-errors-and-solutions)

---

## Prerequisites

### Install Linera CLI
```bash
cargo install linera-service@0.15.6 --locked
```

### Setup Your Wallet
Create a persistent wallet directory:
```bash
cd /home/uzo/mindblown
mkdir -p .linera_wallet
export LINERA_WALLET="$PWD/.linera_wallet/wallet.json"
export LINERA_KEYSTORE="$PWD/.linera_wallet/keystore.json"
export LINERA_STORAGE="rocksdb:$PWD/.linera_wallet/wallet.db"

mkdir -p .linera_wallet-main
export LINERA_WALLET="$PWD/.linera_wallet-main/wallet.json"
export LINERA_KEYSTORE="$PWD/.linera_wallet-main/keystore.json"
export LINERA_STORAGE="rocksdb:$PWD/.linera_wallet-main/wallet.db"
```

### Start Local Network
```bash
# Start the network (keep this terminal open)
linera net up --with-faucet
```

In a new terminal, initialize your wallet:
```bash
export LINERA_WALLET="/home/uzo/mindblown/.linera_wallet/wallet.json"
export LINERA_KEYSTORE="/home/uzo/mindblown/.linera_wallet/keystore.json"
export LINERA_STORAGE="rocksdb:/home/uzo/mindblown/.linera_wallet/wallet.db"

linera wallet init --faucet=http://localhost:8080
linera wallet request-chain --faucet=http://localhost:8080

linera wallet request-chain --faucet=https://faucet.testnet-conway.linera.net/
```

---

## Understanding the Counter Application

### Project Structure

A Linera application consists of:
- `src/lib.rs` - Application ABI (defines operations and messages)
- `src/state.rs` - Application state
- `src/contract.rs` - Contract logic (can modify state)
- `src/service.rs` - Service logic (read-only, GraphQL queries)

### Create Your Project

```bash
linera project new pickn
cd pickn
```

### Application State (`src/state.rs`)

```rust
use linera_sdk::views::{linera_views, RegisterView, RootView, ViewStorageContext};

#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = ViewStorageContext)]
pub struct PicknState {
    pub value: RegisterView<u64>,
}
```

**Key Concepts:**
- `RegisterView<T>` - Stores a single value of type T
- `RootView` - Marks this as the top-level application state
- `SimpleObject` - Exposes state to GraphQL queries

### Application ABI (`src/lib.rs`)

```rust
use async_graphql::{Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{ContractAbi, ServiceAbi},
};
use serde::{Deserialize, Serialize};

pub struct PicknAbi;

impl ContractAbi for PicknAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for PicknAbi {
    type Query = Request;
    type QueryResponse = Response;
}

#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    Increment { value: u64 },
}
```

**Key Concepts:**
- `Operation` - Actions users can perform (like transactions)
- `GraphQLMutationRoot` - Auto-generates GraphQL mutations

### Build and Deploy

```bash
# Build WASM binaries
cargo build --target wasm32-unknown-unknown --release

# Deploy to your chain
linera publish-and-create \
  target/wasm32-unknown-unknown/release/majorules_{contract,service}.wasm \
  --json-argument 10

linera publish-and-create \
  target/wasm32-unknown-unknown/release/pickn_{contract,service}.wasm \
  --json-argument '{"entry_fee": 10}'
```

This creates your app with an initial counter value of `10`.

**Save the Application ID** printed in the output!

### Test Your Application

Start the GraphQL service:
```bash
linera service --port 8081
```

Open `http://localhost:8081` in your browser and run:

**Query the counter:**
```graphql
{
  value
}
```

**Increment the counter:**
```graphql
mutation {
  increment(value: 5)
}
```

---

## Single-Owner vs Multi-Owner Chains

### Understanding Chain Ownership

**Single-Owner Chain:**
- Only ONE person can propose blocks
- Fast and simple
- Perfect for personal wallets or individual game states

**Multi-Owner Chain:**
- MULTIPLE specified people can propose blocks
- Great for game lobbies, tournaments, betting pools
- Each owner needs to sign blocks with their private key

### Use Case: Game Lobby

Imagine a 2-player game. Instead of running it on one player's chain, create a **multi-owner chain** where both players can make moves.

### Create a Multi-Owner Chain

**Step 1: Check your wallet**
```bash
linera wallet show
```

Note your public key (looks like `0xf9cd...`).

**Step 2: Generate a second key (simulating second player)**
```bash
linera keygen
```

**Step 3: Create multi-owner chain**
```bash
linera open-multi-owner-chain \
  --owners <YOUR_PUBLIC_KEY> <SECOND_PUBLIC_KEY> \
  --multi-leader-rounds 10
```

Example:
```bash
linera open-multi-owner-chain \
  --owners 0xf9cd67e7f63468d123f235829e574b48e5ae9f615b185ae8458f21423836449f 0xf477dab7a98aaf1fec83606ddafaa434126e5cb6e85578dbd8a9afe7f47b77fc \
  --multi-leader-rounds 10
```

This outputs a new Chain ID (e.g., `0de6c618...`).

**Step 4: Assign your key to the chain**
```bash
linera assign \
  --owner <YOUR_PUBLIC_KEY> \
  --chain-id <NEW_CHAIN_ID>
```

Example:
```bash
linera assign \
  --owner 0xf9cd67e7f63468d123f235829e574b48e5ae9f615b185ae8458f21423836449f \
  --chain-id 0de6c618154e89de8b1436091c23d5db280bfbf6945040b7162d8ee4a040333e
```

**Step 5: Set as default and deploy**
```bash
linera wallet set-default <NEW_CHAIN_ID>

linera publish-and-create \
  target/wasm32-unknown-unknown/release/pickn_{contract,service}.wasm \
  --json-argument 0
```

Now both owners can increment this counter!

### Key Takeaways

- **Single-owner**: Your personal chain, only you control it
- **Multi-owner**: Shared chain, multiple people can propose blocks
- **Use `assign`** to link your private key to a multi-owner chain
- **Game lobbies** = Multi-owner chains with only invited players

---

## Cross-Chain Messaging

Cross-chain messaging allows applications on different chains to communicate asynchronously. This is essential for multi-player games where each player has their own chain.

### How It Works

1. **Chain A** sends a message → **Chain B's inbox**
2. When **Chain B** creates its next block, it processes the message
3. The message handler executes and can send messages back

### Example: Ping-Pong Counter

We'll create a ping-pong game:
- Chain A sends "Ping" to Chain B
- Chain B increments its counter and sends "Pong" back
- Chain A receives "Pong" and increments its counter

### Code Changes

#### 1. Define Message Type (`src/lib.rs`)

Add after the `Operation` enum:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    Ping,
    Pong { value: u64 },
}
```

Also add a new operation to send ping:

```rust
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    Increment { value: u64 },
    SendPing { destination: linera_sdk::linera_base_types::ChainId },
}
```

#### 2. Update Contract (`src/contract.rs`)

**Update imports:**
```rust
use pickn::{Message, Operation};
```

**Update Message type:**
```rust
impl Contract for PicknContract {
    type Message = Message;  // Changed from ()
    // ...
}
```

**Handle SendPing operation:**
```rust
async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
    match operation {
        Operation::Increment { value } => {
            self.state.value.set(self.state.value.get() + value);
        }
        Operation::SendPing { destination } => {
            self.runtime
                .prepare_message(Message::Ping)
                .send_to(destination);
        }
    }
}
```

**Handle incoming messages:**
```rust
async fn execute_message(&mut self, message: Self::Message) {
    match message {
        Message::Ping => {
            // Increment when we receive a Ping
            let new_value = self.state.value.get() + 1;
            self.state.value.set(new_value);

            // Send Pong back to sender
            let sender = self.runtime.message_origin_chain_id()
                .expect("Message origin should exist");
            self.runtime
                .prepare_message(Message::Pong { value: new_value })
                .send_to(sender);
        }
        Message::Pong { value } => {
            // Increment by the received value
            self.state.value.set(self.state.value.get() + value);
        }
    }
}
```

### Deploy and Test

**Rebuild:**
```bash
cargo build --target wasm32-unknown-unknown --release
```

**Deploy:**
```bash
linera publish-and-create \
  target/wasm32-unknown-unknown/release/pickn_{contract,service}.wasm \
  --json-argument 0
```

**Get your chain IDs:**
```bash
linera wallet show
```

**Send a Ping from Chain 1 to Chain 2:**

In GraphiQL:
```graphql
mutation {
  sendPing(destination: "<CHAIN_2_ID>")
}
```

Example:
```graphql
mutation {
  sendPing(destination: "0de6c618154e89de8b1436091c23d5db280bfbf6945040b7162d8ee4a040333e")
}
```

**What happens:**
1. Chain 1 sends Ping → Chain 2
2. Chain 2 receives Ping, counter goes 0→1, sends Pong(1) back
3. Chain 1 receives Pong(1), counter goes 0→1

### Key API Methods

- `runtime.prepare_message(msg)` - Prepare a message to send
- `.send_to(chain_id)` - Send to a specific chain
- `runtime.message_origin_chain_id()` - Get sender's chain ID
- `execute_message()` - Handle incoming messages

### Message Flow Diagram

```
Chain A (value: 0)                    Chain B (value: 0)
      │                                      │
      │──────────Ping──────────>            │
      │                                      │ (receives Ping)
      │                                      │ (increment: 0→1)
      │            <──────Pong(1)────────────│
      │ (receives Pong)                      │
      │ (increment: 0→1)                     │
      │                                      │
   value: 1                              value: 1
```

### Use Cases for Cross-Chain Messaging

- **Player moves** in a game
- **Token transfers** between chains
- **Notifications** when something happens
- **Cross-chain function calls**

---

## Randomness

In blockchain applications, generating random numbers is tricky because Wasm doesn't have access to the operating system's random number generator. Linera requires you to implement a custom RNG.

### Why Custom RNG?

WebAssembly runs in a sandboxed environment and can't access OS-level randomness. We need to:
1. Provide a custom `getrandom` implementation
2. Use the `rand` crate with our custom RNG
3. Optionally seed with timestamps for non-deterministic behavior

### Example: Random Reward in Ping-Pong

We'll modify our ping-pong example so that when a chain receives a Ping, it sends back a Pong with a **random value (1-100)** instead of the counter value.

### Implementation

#### Step 1: Add Dependencies

Add to `Cargo.toml`:

```toml
getrandom = { version = "0.2.12", default-features = false, features = ["custom"] }
rand = "0.8.5"
```

#### Step 2: Create `src/random.rs`

```rust
use std::sync::{Mutex, OnceLock};
use rand::{rngs::StdRng, Rng, SeedableRng};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    // Fixed seed for deterministic testing
    // For production games, use: runtime.system_time().micros()
    let seed = [0u8; 32];
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock")
        .fill(buf);
    Ok(())
}

getrandom::register_custom_getrandom!(custom_getrandom);

/// Generate a random number between min and max (inclusive)
pub fn random_value(min: u64, max: u64) -> u64 {
    let seed = [0u8; 32]; // Use timestamp in production
    let mut rng = RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock");

    rng.gen_range(min..=max)
}
```

**Key Points:**
- `register_custom_getrandom!` - Registers our custom RNG with the `getrandom` crate
- `OnceLock<Mutex<StdRng>>` - Thread-safe, lazily initialized RNG
- `seed` - Currently fixed for testing; use `runtime.system_time().micros()` for production

#### Step 3: Update `src/contract.rs`

**Add imports:**

```rust
mod state;
mod random;  // Add this

use self::random::random_value;  // Add this
```

**Modify the Ping message handler:**

```rust
async fn execute_message(&mut self, message: Self::Message) {
    match message {
        Message::Ping => {
            // Increment when we receive a Ping
            let new_value = self.state.value.get() + 1;
            self.state.value.set(new_value);

            // Generate random reward between 1-100
            let random_reward = random_value(1, 100);

            // Send Pong back with random value
            let sender = self.runtime.message_origin_chain_id()
                .expect("Message origin should exist");
            self.runtime
                .prepare_message(Message::Pong { value: random_reward })
                .send_to(sender);
        }
        Message::Pong { value } => {
            // Increment by the received random value
            self.state.value.set(self.state.value.get() + value);
        }
    }
}
```

### Test It!

**Build and deploy:**

```bash
cd pickn
cargo build --target wasm32-unknown-unknown --release

linera publish-and-create \
  target/wasm32-unknown-unknown/release/pickn_{contract,service}.wasm \
  --json-argument 0
```

**Send a Ping:**

```graphql
mutation {
  sendPing(destination: "<OTHER_CHAIN_ID>")
}
```

**What happens:**
1. Chain A sends Ping → Chain B
2. Chain B receives Ping, increments counter (0→1)
3. Chain B generates random number (e.g., 42)
4. Chain B sends Pong(42) → Chain A
5. Chain A receives Pong(42), increments by 42 (0→42)

### Using Timestamps for True Randomness

For production games, seed with the block timestamp:

```rust
pub fn random_value_with_timestamp(min: u64, max: u64, timestamp: u64) -> u64 {
    let mut seed = [0u8; 32];
    seed[..8].copy_from_slice(&timestamp.to_le_bytes());

    let mut rng = StdRng::from_seed(seed);
    rng.gen_range(min..=max)
}
```

Then in your contract:

```rust
let timestamp = self.runtime.system_time().micros();
let random_reward = random_value_with_timestamp(1, 100, timestamp);
```

### Use Cases

- **Dice rolls** - Generate 1-6 for games
- **Card shuffling** - Randomly order a deck
- **Loot drops** - Random rewards in games
- **Random matchmaking** - Pair players randomly
- **Prize distribution** - Randomly select winners

### Important Notes

⚠️ **Determinism**: With a fixed seed, the same operations will produce the same random numbers across all validators. This ensures consensus.

✅ **Non-determinism**: Use timestamps or block heights as seeds for truly random behavior.

🎲 **Range**: Use `gen_range(min..=max)` for inclusive ranges (1-6 for a dice roll).

---

## Common Errors and Solutions

### Error: "Connection refused" when publishing

**Problem:**
```
Grpc error: tcp connect error: Connection refused
```

**Solution:**
The Linera network isn't running. Start it:
```bash
linera net up --with-faucet
```

Keep this terminal open!

---

### Error: "Keystore already exists"

**Problem:**
```
Error is Keystore already exists: /tmp/.tmp.../keystore_0.json
```

**Solution:**
Wallet was already initialized. Skip the `wallet init` step or clean up:
```bash
rm -rf /tmp/.tmp*
```

---

### Error: "client is not configured to propose on chain"

**Problem:**
```
client is not configured to propose on chain 0de6c618...
```

**Solution:**
You need to assign your key to the chain:
```bash
linera assign \
  --owner <YOUR_PUBLIC_KEY> \
  --chain-id <CHAIN_ID>
```
linera assign \
  --owner 0xf9cd67e7f63468d123f235829e574b48e5ae9f615b185ae8458f21423836449f \
  --chain-id 6ca8c4d9ce04545a3d4727cc960f03544cd34813008500184ddbeb99b7f239f4
---

### Error: "no method named `message_id` found"

**Problem:**
```
error[E0599]: no method named `message_id` found
```

**Solution:**
Use `message_origin_chain_id()` instead:
```rust
let sender = self.runtime.message_origin_chain_id()
    .expect("Message origin should exist");
```

Check the [ContractRuntime docs](https://docs.rs/linera-sdk/latest/linera_sdk/contract/struct.ContractRuntime.html) for correct method names.

---

### Error: Resource temporarily unavailable (RocksDB lock)

**Problem:**
```
RocksDB error: IO error: While lock file: .../LOCK: Resource temporarily unavailable
```

**Solution:**
Another linera process is using the database. Kill it:
```bash
pkill -9 linera
```

Then retry your command.

---

## Native Token Transfers

Linera has a native token system built into the chain level. You can transfer tokens between accounts without needing a separate token contract.

### Understanding Accounts

In Linera, an `Account` consists of:
- `chain_id` - The chain where the account exists
- `owner` - The AccountOwner (can be a user or `AccountOwner::CHAIN` for chain balance)

### Token Transfer API

The runtime provides these methods for native token operations:

```rust
// Transfer tokens
runtime.transfer(
    source: AccountOwner,      // Who sends (user or AccountOwner::CHAIN)
    destination: Account,       // Where to send (chain + owner)
    amount: Amount             // How much to send
)

// Check balances
runtime.owner_balance(owner: AccountOwner) -> Amount
runtime.chain_balance() -> Amount
```

### Amount Arithmetic

The `Amount` type represents token amounts with 18 decimal places. Key operations:

```rust
// Creation
Amount::ZERO                    // 0 tokens
Amount::ONE                     // 1 token
Amount::from_tokens(10)         // 10 tokens
Amount::from_attos(1000)        // 1000 attotokens (smallest unit)

// Arithmetic
amount.saturating_mul(5)        // Multiply by u128
amount.saturating_add(other)    // Add two Amounts
amount.saturating_sub(other)    // Subtract two Amounts
amount.saturating_div(other)    // Divide, returns u128 ratio

// Common patterns
let total = entry_fee.saturating_mul(player_count);
let share = total.saturating_div(Amount::from_attos(survivor_count));
let final_amount = Amount::from_attos(share);
```

### Example: Entry Fee Collection

Here's how to collect entry fees when players join a game lobby:

#### 1. Define Initialization with Entry Fee

```rust
// In src/lib.rs
#[derive(Debug, Deserialize, Serialize)]
pub struct InitializationArgument {
    pub entry_fee: Amount,
}
```

#### 2. Set Entry Fee During Instantiation

```rust
// In src/contract.rs
impl Contract for GameContract {
    type InstantiationArgument = InitializationArgument;

    async fn instantiate(&mut self, argument: Self::InstantiationArgument) {
        self.runtime.application_parameters();

        // Set the entry fee for this lobby
        self.state.entry_fee.set(argument.entry_fee);
        self.state.is_lobby.set(true);
        // ... other initialization
    }
}
```

#### 3. Collect Entry Fee on Join

```rust
Operation::JoinLobby => {
    let player = self.runtime
        .authenticated_signer()
        .expect("Player must be authenticated");

    // Get the entry fee
    let entry_fee = *self.state.entry_fee.get();

    // Transfer from player to chain balance
    let to_account = Account {
        chain_id: self.runtime.chain_id(),
        owner: AccountOwner::CHAIN,  // Send to chain balance
    };

    self.runtime.transfer(player, to_account, entry_fee);

    // Add player to lobby
    self.state.waiting_players.insert(&player, player_chain)
        .expect("Failed to add player");
}
```

#### 4. Refund on Leave

```rust
Operation::LeaveLobby => {
    let player = self.runtime
        .authenticated_signer()
        .expect("Player must be authenticated");

    let player_chain = self.state.waiting_players
        .get(&player)
        .await
        .expect("Failed to check waiting_players")
        .expect("Not in lobby");

    let entry_fee = *self.state.entry_fee.get();

    // Remove player first (prevent re-entrancy)
    self.state.waiting_players.remove(&player)
        .expect("Failed to remove player");

    // Refund from chain balance to player
    let to_account = Account {
        chain_id: player_chain,
        owner: player,
    };

    self.runtime.transfer(AccountOwner::CHAIN, to_account, entry_fee);
}
```

#### 5. Distribute Prizes

```rust
Operation::ClaimPrize => {
    let caller = self.runtime
        .authenticated_signer()
        .expect("Caller must be authenticated");

    // Verify game is finished and caller is survivor
    assert_eq!(*self.state.status.get(), STATUS_FINISHED);
    assert!(!eliminated.contains(&caller), "You were eliminated");

    // Mark as claimed to prevent double-claim
    self.state.prize_claimed.insert(&caller, true)
        .expect("Failed to mark claimed");

    // Calculate prize distribution
    let survivors_count = (players.len() - eliminated.len()) as u128;
    let entry_fee = *self.state.entry_fee.get();
    let total_players = players.len() as u128;

    // Total pool = all entry fees
    let total_collected = entry_fee.saturating_mul(total_players);

    // Prize pool = 95% (5% platform fee)
    let total_times_95 = total_collected.saturating_mul(95);
    let ratio_95_percent = total_times_95.saturating_div(Amount::from_attos(100));
    let total_prize_pool = Amount::from_attos(ratio_95_percent);

    // Each survivor gets equal share
    let ratio_per_survivor = total_prize_pool.saturating_div(Amount::from_attos(survivors_count));
    let prize_share = Amount::from_attos(ratio_per_survivor);

    // Transfer prize
    let to_account = Account {
        chain_id: self.runtime.chain_id(),
        owner: caller,
    };

    self.runtime.transfer(AccountOwner::CHAIN, to_account, prize_share);
}
```

### Key Patterns

**Pattern 1: Collect fees to chain balance**
```rust
runtime.transfer(player, Account { chain_id, owner: AccountOwner::CHAIN }, amount)
```

**Pattern 2: Refund from chain balance**
```rust
runtime.transfer(AccountOwner::CHAIN, Account { chain_id, owner: player }, amount)
```

**Pattern 3: Calculate percentage**
```rust
let ninety_five_percent = total.saturating_mul(95)
    .saturating_div(Amount::from_attos(100));
let result = Amount::from_attos(ninety_five_percent);
```

**Pattern 4: Divide among N people**
```rust
let per_person_ratio = total.saturating_div(Amount::from_attos(n as u128));
let per_person = Amount::from_attos(per_person_ratio);
```

### Testing Token Transfers

When creating test applications:

```rust
#[tokio::test]
async fn test_entry_fee() {
    let (validator, module_id) =
        TestValidator::with_current_module::<GameAbi, (), InitializationArgument>().await;

    let mut chain = validator.new_chain().await;

    let initial_state = InitializationArgument {
        entry_fee: Amount::from_tokens(1), // 1 token entry fee
    };

    let application_id = chain
        .create_application(module_id, (), initial_state, vec![])
        .await;

    // Test join lobby (transfers 1 token from player to chain)
    chain.add_block(|block| {
        block.with_operation(application_id, Operation::JoinLobby);
    }).await;
}
```

---

## Debugging Tips

### Common Type Errors

**Error: `mismatched types: expected AccountOwner, found Account`**

```rust
// ❌ Wrong - runtime.transfer takes AccountOwner as source
let from = Account { chain_id, owner: player };
runtime.transfer(from, to_account, amount);

// ✅ Correct - Use AccountOwner directly
runtime.transfer(player, to_account, amount);
```

**Error: `no method named as_u128 found for Amount`**

```rust
// ❌ Wrong - Amount doesn't have as_u128()
let value = amount.as_u128();

// ✅ Correct - Use saturating_div to get u128 ratio
let ratio = amount.saturating_div(Amount::from_attos(divisor));
let result = Amount::from_attos(ratio);
```

**Error: `cannot divide Amount by integer`**

```rust
// ❌ Wrong - Can't divide Amount directly by integer
let half = total / 2;

// ✅ Correct - Use saturating_div
let ratio = total.saturating_div(Amount::from_attos(2));
let half = Amount::from_attos(ratio);
```

### Test Type Mismatches

**Error: `expected u64, found InitializationArgument`**

When you change `InstantiationArgument` from `u64` to a custom struct:

```rust
// Update all tests from:
let initial_state = 10u64;

// To:
let initial_state = InitializationArgument {
    entry_fee: Amount::from_tokens(1),
};

// Also update TestValidator type:
TestValidator::with_current_module::<Abi, (), InitializationArgument>().await
```

### MapView vs RegisterView

Remember the difference in async/sync operations:

```rust
// RegisterView - synchronous
self.state.value.set(10);              // No .await
let value = *self.state.value.get();   // No .await

// MapView - async operations
self.state.players.insert(&key, value)?;  // No .await (insert is sync!)
let value = self.state.players.get(&key).await?;  // .await needed
let exists = self.state.players.contains_key(&key).await?;  // .await needed
```

### Build Warnings

If you see unused import warnings:

```rust
// Remove unused imports
use linera_sdk::{
    linera_base_types::{ChainId, Timestamp},  // ❌ Not used
    // ...
};

// Keep only what you use
use linera_sdk::{
    linera_base_types::{Account, AccountOwner, Amount},
    // ...
};
```

### Common Assert Patterns

```rust
// Check lobby state
assert!(*self.state.is_lobby.get(), "Not a lobby chain");

// Check game state
assert!(!*self.state.is_lobby.get(), "Still in lobby");

// Check player eligibility
assert!(players.contains(&caller), "Not in this game");
assert!(!eliminated.contains(&caller), "You were eliminated");

// Check claim status
let already_claimed = self.state.prize_claimed
    .contains_key(&caller)
    .await
    .expect("Failed to check prize_claimed");
assert!(!already_claimed, "Already claimed");
```

---

## Next Steps

Now that you understand the basics, try building:

1. **A 2-player game** using multi-owner chains
2. **A token transfer system** using cross-chain messaging
3. **A dice game** using randomness (coming soon)

### Useful Resources

- [Linera Documentation](https://docs.linera.io)
- [Linera SDK Reference](https://docs.rs/linera-sdk)
- [Example Applications](https://github.com/linera-io/linera-protocol/tree/main/examples)
- [Linera Discord](https://discord.gg/linera)

---

## Summary

**What you learned:**

✅ **Basic Application Structure**
- State, Contract, Service, ABI
- Building and deploying with WASM
- GraphQL queries and mutations

✅ **Chain Ownership**
- Single-owner chains for personal use
- Multi-owner chains for collaboration
- Opening and assigning keys to chains

✅ **Cross-Chain Messaging**
- Sending messages between chains
- Handling incoming messages
- Building interactive multi-chain apps

✅ **Common Pitfalls**
- Network not running
- Wallet already exists
- RocksDB locks
- Method name errors

**Next:** Randomness for game mechanics!


Build me a landing page for my web3 product, it should show that it is launching on december 25th, also it should have pages for differnet data, also it should show a lot of quotes and motivating people to look forwward to the project and even invest in it, and mostly show a lot of info of what the product does , use images too - here is the product detail TaskerOnChain