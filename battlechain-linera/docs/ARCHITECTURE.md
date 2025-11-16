# BattleChain Architecture

## System Overview

BattleChain is built on **Linera Protocol's microchains architecture**, where each entity (player, battle, market) runs on its own blockchain. This design provides:

- **Maximum Performance** - Single-owner chains have zero contention
- **Scalability** - Chains run in parallel, no global bottleneck
- **User Ownership** - Players truly own their chain and data
- **Fast Transactions** - Sub-second finality on single-owner chains

## Microchains Design

### 1. Player Chain (Single-Owner)

**Purpose**: NFT character ownership and personal inventory

**Ownership**: Single player (maximum performance)

**State**:
```rust
pub struct PlayerState {
    characters: MapView<String, CharacterSnapshot>,
    battle_stakes: MapView<ChainId, Amount>,
    total_battles: u64,
    wins: u64,
    losses: u64,
    battle_token_balance: Amount,
}
```

**Operations**:
- `MintCharacter` - Create new NFT character
- `LevelUpCharacter` - Increase character level
- `LockStake` - Lock tokens for battle
- `UnlockStake` - Unlock tokens after battle

**Cross-chain Messages**:
- Receives `BattleResult` from Battle Chain
- Receives `LockStakeRequest` from Matchmaking
- Receives `WinningsPayout` from Prediction Market

**Performance**: ~1000+ TPS (single-owner, no contention)

### 2. Battle Chain (Multi-Owner)

**Purpose**: Turn-based combat engine

**Ownership**: Two players (both must sign)

**State**:
```rust
pub struct BattleState {
    player1: BattleParticipant,
    player2: BattleParticipant,
    current_turn: u8,
    current_round: u8,
    battle_status: BattleStatus,
    total_stake: Amount,
    random_counter: u64,  // For randomness generation
}
```

**Combat Flow**:
1. Players submit stance choices
2. Speed determines turn order
3. Calculate damage with modifiers
4. Check for crit/dodge/counter
5. Update HP and combo stacks
6. Repeat until winner

**Randomness**:
- Uses `system_time().micros() + counter + XOR` for seeding
- Deterministic but unpredictable
- No external oracle needed

**Operations**:
- `SubmitStance` - Choose battle stance
- `UseSpecialAbility` - Activate class ability
- `FinalizeBattle` - End battle and distribute rewards

**Cross-chain Messages**:
- Sends `BattleResult` to Player Chains
- Sends `BattleCompleted` to Matchmaking
- Sends `BattleEnded` to Prediction Market
- Sends `BattleCompleted` to Registry

**Performance**: ~100 TPS (multi-owner consensus required)

### 3. Matchmaking Chain (Public)

**Purpose**: Queue management and battle coordination

**Ownership**: Public/Admin

**State**:
```rust
pub struct MatchmakingState {
    waiting_players: MapView<ChainId, QueueEntry>,
    pending_battles: MapView<u64, PendingBattle>,
    active_battles: MapView<ChainId, BattleMetadata>,
    battle_app_id: ApplicationId,
}
```

**Matchmaking Flow**:
1. Player joins queue with character and stake
2. Matchmaker creates battle offer between 2 players
3. Both players must confirm
4. Battle chain created (multi-owner)
5. Players notified

**Operations**:
- `JoinQueue` - Enter matchmaking
- `LeaveQueue` - Exit queue
- `CreateBattleOffer` - Propose match between 2 players
- `ConfirmBattleOffer` - Accept proposed match
- `RecordBattleCompletion` - Update after battle ends

**Future Enhancements**:
- Skill-based matchmaking (ELO matching)
- Automatic matching algorithm
- Tournament brackets

**Performance**: ~200 TPS (public chain, optimized for writes)

### 4. Prediction Market Chain (Public)

**Purpose**: Spectator betting on battle outcomes

**Ownership**: Public

**State**:
```rust
pub struct PredictionState {
    markets: MapView<u64, Market>,
    bets: MapView<(u64, ChainId), Bet>,
    battle_to_market: MapView<ChainId, u64>,
    total_volume: Amount,
}
```

**Market Lifecycle**:
1. **Open** - Accepting bets before battle starts
2. **Closed** - Battle started, no more bets
3. **Settled** - Battle ended, winnings distributed
4. **Cancelled** - Battle cancelled, refunds issued

**Odds Calculation**:
```
odds = (total_pool / side_pool) * 10000  // Basis points
winnings = bet_amount * odds / 10000
```

**Operations**:
- `CreateMarket` - Initialize market for battle
- `PlaceBet` - Bet on battle outcome
- `CloseMarket` - Stop accepting bets
- `SettleMarket` - Distribute winnings
- `ClaimWinnings` - Collect payout

**Platform Fee**: 3% (configurable)

**Performance**: ~500 TPS (read-optimized public chain)

### 5. Registry Chain (Public)

**Purpose**: Global leaderboards and statistics

**Ownership**: Public

**State**:
```rust
pub struct RegistryState {
    characters: MapView<String, CharacterStats>,
    battles: MapView<u64, BattleRecord>,
    owner_to_character: MapView<ChainId, String>,
    top_elo: Vec<String>,  // Top 100 characters
}
```

**Statistics Tracked**:
- **Combat**: Total battles, wins, losses, win rate
- **Performance**: Damage dealt/taken, crits, dodges
- **Economics**: Total earnings, volume wagered
- **Rating**: ELO rating (chess formula)
- **Streaks**: Current streak, best win streak

**ELO System**:
```rust
const K_FACTOR: f64 = 32.0;
expected = 1.0 / (1.0 + 10^((opponent_elo - player_elo) / 400))
new_elo = player_elo + K_FACTOR * (actual - expected)
```

**Operations**:
- `RegisterCharacter` - Add character to registry
- `UpdateCharacterStats` - Update after battle
- `RecordBattle` - Log battle to history
- `UpdateCharacterLevel` - Track progression
- `MarkCharacterDefeated` - Handle permadeath

**GraphQL Queries**:
- `topCharacters(limit)` - ELO leaderboard
- `character(id)` - Character details
- `stats` - Global statistics

**Performance**: ~1000 TPS (read-heavy workload)

### 6. Battle Token Chain (Public)

**Purpose**: Fungible token for game economy

**Implementation**: Standard fungible token

**Operations**:
- `Transfer` - Send tokens
- `Mint` - Create new tokens (admin only)
- `Burn` - Destroy tokens

**Usage**:
- Staking in battles
- Character minting fees
- Prediction market bets
- Platform fees

## Cross-Chain Message Flow

### Battle Creation Flow

```
Player 1 Chain                    Matchmaking Chain
     │                                   │
     ├──JoinQueue──────────────────────>│
     │                                   │
Player 2 Chain                          │
     │                                   │
     ├──JoinQueue──────────────────────>│
     │                                   │
     │         ┌───CreateBattleOffer────┤
     │         │                         │
     ├<────BattleOffer─────────────────┤
     │                                   │
     ├──ConfirmBattleOffer─────────────>│
     │                                   │
Player 2 Chain                          │
     │                                   │
     ├──ConfirmBattleOffer─────────────>│
     │                                   │
     │                   ┌───────────────┤
     │                   │ Create Battle Chain
     │                   │ (Multi-owner)
     │                   ▼               │
     │         Battle Chain Created      │
     │                   │               │
     ├<────BattleCreated────────────────┤
     │                                   │
```

### Battle Completion Flow

```
Battle Chain             Player Chains         Registry Chain      Prediction Market
     │                        │                      │                    │
     ├──FinalizeBattle        │                      │                    │
     │                        │                      │                    │
     ├──BattleResult─────────>│                      │                    │
     │                        │                      │                    │
     │                        ├──Update Stats        │                    │
     │                        │                      │                    │
     ├──BattleCompleted──────────────────────────────>│                    │
     │                        │                      │                    │
     │                        │             Update ELO & Stats             │
     │                        │                      │                    │
     ├──BattleEnded────────────────────────────────────────────────────────>│
     │                        │                      │                    │
     │                        │                      │           Settle Market
     │                        │                      │                    │
```

## State Management

### Linera Views System

BattleChain uses Linera's view system for efficient state management:

**RegisterView<T>**: Single value
```rust
pub current_round: RegisterView<u8>
pub total_battles: RegisterView<u64>
```

**MapView<K, V>**: Key-value storage
```rust
pub characters: MapView<String, CharacterStats>
pub bets: MapView<(u64, ChainId), Bet>
```

**Benefits**:
- Lazy loading (only load what you need)
- Efficient storage
- Type-safe access
- Automatic serialization

### State Transitions

All state changes are atomic and validated:

1. **Operation received** - Validated input
2. **State loaded** - Lazy load required views
3. **Validation** - Check preconditions
4. **State update** - Modify views
5. **Cross-chain messages** - Send notifications
6. **State saved** - Commit changes

## Security Model

### Authentication

- **Single-owner chains**: Automatic signature verification
- **Multi-owner chains**: Both owners must sign
- **Public chains**: Permissioned operations

### Input Validation

All operations validate:
- Amounts are non-zero and within bounds
- Character IDs exist
- Battle states are valid
- Ownership is correct

### Overflow Protection

All arithmetic uses saturating operations:
```rust
amount.saturating_add(other)
amount.saturating_sub(other)
```

### Cross-chain Security

Messages authenticated:
```rust
self.runtime
    .prepare_message(msg)
    .with_authentication()  // Signed by sender
    .send_to(destination);
```

## Performance Characteristics

### Transaction Throughput

| Chain Type | TPS | Latency | Notes |
|------------|-----|---------|-------|
| Player (single-owner) | 1000+ | <100ms | No contention |
| Battle (2-owner) | 100+ | <500ms | Requires both signatures |
| Matchmaking (public) | 200+ | <300ms | Write-optimized |
| Registry (public) | 1000+ | <200ms | Read-heavy |
| Prediction (public) | 500+ | <200ms | Balanced |

### State Size

| Chain | State Size | Growth Rate |
|-------|------------|-------------|
| Player | ~10 KB/player | Linear with characters |
| Battle | ~5 KB/battle | Temporary (cleared after) |
| Matchmaking | ~100 KB | Linear with queue |
| Registry | ~1 MB | Linear with battles |
| Prediction | ~500 KB | Linear with bets |

### Gas Costs

Linera uses gas for operations. Estimated costs:

| Operation | Gas | Notes |
|-----------|-----|-------|
| MintCharacter | 10k | One-time per character |
| JoinQueue | 5k | Queue entry |
| SubmitStance | 3k | Per turn |
| PlaceBet | 5k | Per bet |
| UpdateStats | 8k | Per battle completion |

## Scalability

### Horizontal Scaling

- **Player chains**: One per player (unlimited horizontal scale)
- **Battle chains**: One per active battle (parallel execution)
- **Market chains**: Can be sharded by battle ID

### Vertical Scaling

- Linera validators handle parallel chain execution
- No global state bottleneck
- Linear performance with hardware

### Future Optimizations

1. **State pruning**: Archive old battles
2. **Sharding**: Split registry by region/class
3. **Caching**: Read replicas for queries
4. **Batching**: Batch operations where possible

## Design Decisions

### Why Microchains?

**Alternative**: Single monolithic blockchain
- ❌ Global state = contention
- ❌ All users compete for same block space
- ❌ High gas fees
- ✅ Simpler to reason about

**Microchains**:
- ✅ No contention on player chains
- ✅ Parallel execution
- ✅ Lower costs
- ❌ More complex architecture

**Decision**: Microchains for performance and scalability

### Why Timestamp-based Randomness?

**Alternative**: VRF oracle chain
- ✅ Cryptographically secure
- ❌ Extra chain to maintain
- ❌ Complexity
- ❌ Latency from oracle calls

**Timestamp + Counter**:
- ✅ Instant
- ✅ Simple
- ✅ Deterministic (can verify)
- ❌ Not cryptographically random (but sufficient for gaming)

**Decision**: Timestamp-based for simplicity and performance

### Why ELO Rating?

**Alternative**: Win/loss ratio
- ✅ Simple
- ❌ Doesn't account for opponent strength
- ❌ Smurfing problems

**ELO System**:
- ✅ Accounts for opponent skill
- ✅ Battle-tested (chess, esports)
- ✅ Self-balancing
- ❌ Requires tuning K-factor

**Decision**: ELO for competitive fairness

## Future Architecture

### Planned Enhancements

1. **Tournament System**
   - Bracket-style tournaments
   - Prize pools
   - Seasonal competitions

2. **Guild Chains**
   - Multi-player organizations
   - Shared resources
   - Guild wars

3. **PvE Chains**
   - AI opponents
   - Boss battles
   - Cooperative gameplay

4. **Cross-game Integration**
   - Character transfers
   - Shared economy
   - Multi-game tournaments

## Conclusion

BattleChain's microchains architecture provides:
- **Performance**: 1000+ TPS on single-owner chains
- **Scalability**: Linear with players
- **Security**: Authenticated, validated, overflow-safe
- **User Ownership**: Players own their chains
- **Innovation**: First fully on-chain fighting game on Linera

The architecture is designed for both current gameplay and future expansion.
