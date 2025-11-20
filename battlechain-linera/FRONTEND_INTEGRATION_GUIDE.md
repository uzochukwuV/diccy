# 🎨 BattleChain Frontend Integration Guide

**Complete guide for connecting React frontend to Linera smart contracts**

---

## 📋 Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Linera GraphQL Pattern](#linera-graphql-pattern)
3. [Smart Contract Service Structure](#smart-contract-service-structure)
4. [Frontend Setup](#frontend-setup)
5. [Integration Examples by Chain](#integration-examples-by-chain)
6. [Complete Component Examples](#complete-component-examples)
7. [Best Practices](#best-practices)

---

## 🏗️ Architecture Overview

### Linera Smart Contract Pattern

Every Linera application follows the **Contract-Service** pattern:

```
┌─────────────────────────────────────┐
│     Smart Contract (WASM)           │
├─────────────────────────────────────┤
│  CONTRACT                           │
│  - Executes operations              │
│  - Modifies blockchain state        │
│  - Sends/receives messages          │
│                                     │
│  SERVICE                            │
│  - Read-only GraphQL queries        │
│  - No state modification            │
│  - Exposes state to frontend        │
└─────────────────────────────────────┘
         ▲               ▼
         │    GraphQL    │
         │               │
┌────────┴───────────────┴─────────────┐
│     React Frontend                   │
│  - Apollo Client                     │
│  - Queries (read state)              │
│  - Mutations (execute operations)    │
└──────────────────────────────────────┘
```

### BattleChain Architecture

```
                    Frontend (React)
                          │
        ┌─────────────────┼─────────────────┐
        │                 │                 │
   Player Chain    Matchmaking Chain   Registry Chain
        │                 │                 │
        └────── Battle Chain ──────┬────────┘
                     │             │
              Prediction Chain  Battle Token
```

---

## 🔌 Linera GraphQL Pattern

### How It Works

**From Microcard Analysis:**

1. **Service Definition** (Rust):
```rust
use async_graphql::{Request, Response, Schema, EmptySubscription, Object};
use linera_sdk::graphql::GraphQLMutationRoot;

impl ServiceAbi for MyAbi {
    type Query = Request;        // GraphQL queries
    type QueryResponse = Response;
}

#[derive(GraphQLMutationRoot)]  // Auto-generates mutations
pub enum Operation {
    CreateCharacter { nft_id: String, class: CharacterClass },
    JoinQueue { character: String, stake: Amount },
}

// Service query implementation
#[Object]
impl QueryRoot {
    async fn get_character(&self, id: String) -> Option<Character> {
        self.state.characters.get(&id).await.ok().flatten()
    }

    async fn get_balance(&self) -> Amount {
        *self.state.balance.get()
    }
}
```

2. **Frontend** (React/TypeScript):
```typescript
import { gql, useQuery, useMutation } from '@apollo/client';

// Query example
const GET_CHARACTER = gql`
  query GetCharacter($id: String!) {
    getCharacter(id: $id) {
      nftId
      class
      level
      hp
    }
  }
`;

// Mutation example
const CREATE_CHARACTER = gql`
  mutation CreateCharacter($nftId: String!, $class: String!) {
    createCharacter(nftId: $nftId, class: $class)
  }
`;
```

---

## 📊 Smart Contract Service Structure

### Pattern from Microcard

**service.rs** structure:

```rust
pub struct MyService {
    state: Arc<MyState>,
    runtime: Arc<ServiceRuntime<Self>>,
}

impl Service for MyService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = MyState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        MyService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, query: Self::Query) -> Self::QueryResponse {
        Schema::build(
            QueryRoot {
                state: self.state.clone(),
                runtime: self.runtime.clone(),
            },
            MyOperation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
        .finish()
        .execute(query)
        .await
    }
}

#[Object]
impl QueryRoot {
    async fn my_custom_query(&self) -> MyData {
        // Read from self.state
        self.state.my_field.get().clone()
    }
}
```

### What Each Chain Needs

#### ✅ Already Have:
- **Battle Chain**: Has service with combat state queries
- **Player Chain**: Has character and balance queries
- **Registry Chain**: Has leaderboard queries
- **Prediction Chain**: Has market queries
- **Matchmaking Chain**: Has queue/battle queries
- **Battle Token**: Has balance/allowance queries

#### ❌ Need to Add:
- More query helpers for frontend convenience
- Aggregate queries (e.g., "get player dashboard")
- Optimized queries for common UI patterns

---

## 🚀 Frontend Setup

### Current Setup Analysis

Your `web-frontend/` already has:
- ✅ `@apollo/client` - GraphQL client
- ✅ `@linera/client` - Linera SDK
- ✅ `GraphQLProvider` component
- ✅ React 19

### Required Changes

**1. Update GraphQLProvider for Multi-Chain**

```typescript
// web-frontend/src/contexts/LineraContext.tsx
import React, { createContext, useContext, useState, useEffect } from 'react';
import { ApolloClient, InMemoryCache, HttpLink } from '@apollo/client';
import * as linera from '@linera/client';

interface LineraContextType {
  wallet: any;
  clients: {
    player: ApolloClient<any>;
    battle: ApolloClient<any>;
    matchmaking: ApolloClient<any>;
    registry: ApolloClient<any>;
    prediction: ApolloClient<any>;
    token: ApolloClient<any>;
  };
  chainId: string;
  applicationIds: {
    player: string;
    battle: string;
    matchmaking: string;
    registry: string;
    prediction: string;
    token: string;
  };
}

export const LineraContext = createContext<LineraContextType | null>(null);

export function LineraProvider({ children }: { children: React.ReactNode }) {
  const [context, setContext] = useState<LineraContextType | null>(null);

  useEffect(() => {
    async function init() {
      // Initialize Linera wallet
      await linera.default();
      const faucet = new linera.Faucet(
        'https://faucet.testnet-conway.linera.net'
      );
      const wallet = await faucet.createWallet();
      const chainId = await faucet.claimChain(wallet);

      // Application IDs (from deployment)
      const appIds = {
        player: process.env.REACT_APP_PLAYER_APP_ID!,
        battle: process.env.REACT_APP_BATTLE_APP_ID!,
        matchmaking: process.env.REACT_APP_MATCHMAKING_APP_ID!,
        registry: process.env.REACT_APP_REGISTRY_APP_ID!,
        prediction: process.env.REACT_APP_PREDICTION_APP_ID!,
        token: process.env.REACT_APP_TOKEN_APP_ID!,
      };

      // Create Apollo clients for each chain
      const port = process.env.REACT_APP_LINERA_PORT || 8080;
      const createClient = (appId: string) =>
        new ApolloClient({
          link: new HttpLink({
            uri: `http://localhost:${port}/chains/${chainId}/applications/${appId}`,
          }),
          cache: new InMemoryCache(),
        });

      setContext({
        wallet,
        chainId,
        applicationIds: appIds,
        clients: {
          player: createClient(appIds.player),
          battle: createClient(appIds.battle),
          matchmaking: createClient(appIds.matchmaking),
          registry: createClient(appIds.registry),
          prediction: createClient(appIds.prediction),
          token: createClient(appIds.token),
        },
      });
    }

    init();
  }, []);

  if (!context) {
    return <div>Connecting to Linera...</div>;
  }

  return (
    <LineraContext.Provider value={context}>
      {children}
    </LineraContext.Provider>
  );
}

export function useLinera() {
  const context = useContext(LineraContext);
  if (!context) {
    throw new Error('useLinera must be used within LineraProvider');
  }
  return context;
}
```

**2. Add .env File**

```bash
# web-frontend/.env
REACT_APP_LINERA_PORT=8080
REACT_APP_PLAYER_APP_ID=<your_player_app_id>
REACT_APP_BATTLE_APP_ID=<your_battle_app_id>
REACT_APP_MATCHMAKING_APP_ID=<your_matchmaking_app_id>
REACT_APP_REGISTRY_APP_ID=<your_registry_app_id>
REACT_APP_PREDICTION_APP_ID=<your_prediction_app_id>
REACT_APP_TOKEN_APP_ID=<your_token_app_id>
```

---

## 🎮 Integration Examples by Chain

### 1. Player Chain

**Queries:**
```typescript
// web-frontend/src/graphql/player.ts
import { gql } from '@apollo/client';

export const GET_CHARACTERS = gql`
  query GetCharacters {
    getCharacters {
      nftId
      class
      level
      xp
      hp
      attack
      defense
      speed
      lives
      wins
      losses
    }
  }
`;

export const GET_BATTLE_BALANCE = gql`
  query GetBattleBalance {
    getBattleBalance
  }
`;

export const GET_PLAYER_STATS = gql`
  query GetPlayerStats {
    getPlayerStats {
      totalBattles
      wins
      losses
      winRate
      totalEarnings
    }
  }
`;
```

**Mutations:**
```typescript
export const CREATE_CHARACTER = gql`
  mutation CreateCharacter($nftId: String!, $class: String!) {
    createCharacter(nftId: $nftId, class: $class)
  }
`;

export const JOIN_BATTLE = gql`
  mutation JoinBattle($battleChain: String!, $characterNft: String!, $stake: String!) {
    joinBattle(
      battleChain: $battleChain
      characterNft: $characterNft
      stake: $stake
    )
  }
`;
```

**Component Example:**
```tsx
// web-frontend/src/components/CharacterSelector.tsx
import React from 'react';
import { useQuery, useMutation } from '@apollo/client';
import { useLinera } from '../contexts/LineraContext';
import { GET_CHARACTERS, CREATE_CHARACTER } from '../graphql/player';

export function CharacterSelector() {
  const { clients } = useLinera();
  const { data, loading, error } = useQuery(GET_CHARACTERS, {
    client: clients.player,
  });

  const [createCharacter] = useMutation(CREATE_CHARACTER, {
    client: clients.player,
    refetchQueries: [{ query: GET_CHARACTERS }],
  });

  const handleCreate = async (nftId: string, characterClass: string) => {
    try {
      await createCharacter({
        variables: { nftId, class: characterClass },
      });
      alert('Character created!');
    } catch (err) {
      console.error('Failed to create character:', err);
    }
  };

  if (loading) return <div>Loading characters...</div>;
  if (error) return <div>Error: {error.message}</div>;

  return (
    <div className="grid grid-cols-3 gap-4">
      {data.getCharacters.map((char: any) => (
        <div key={char.nftId} className="bg-surface rounded-xl p-4">
          <h3 className="text-xl font-bold">{char.class}</h3>
          <p>Level {char.level}</p>
          <p>HP: {char.hp}</p>
          <p>ATK: {char.attack} | DEF: {char.defense}</p>
          <p className="text-primary">
            W/L: {char.wins}/{char.losses}
          </p>
        </div>
      ))}
    </div>
  );
}
```

### 2. Matchmaking Chain

**Queries:**
```typescript
// web-frontend/src/graphql/matchmaking.ts
import { gql } from '@apollo/client';

export const GET_MATCHMAKING_STATS = gql`
  query GetMatchmakingStats {
    getMatchmakingStats {
      waitingPlayers
      pendingBattles
      activeBattles
      totalBattles
    }
  }
`;

export const GET_PENDING_BATTLES = gql`
  query GetPendingBattles {
    getPendingBattles {
      offerId
      player1 {
        playerChain
        playerOwner
        character {
          class
          level
          hp
        }
        stake
      }
      player2 {
        playerChain
        playerOwner
        character {
          class
          level
          hp
        }
        stake
      }
      createdAt
      player1Confirmed
      player2Confirmed
    }
  }
`;
```

**Mutations:**
```typescript
export const JOIN_QUEUE = gql`
  mutation JoinQueue(
    $playerChain: String!
    $playerOwner: String!
    $character: CharacterSnapshotInput!
    $stake: String!
  ) {
    joinQueue(
      playerChain: $playerChain
      playerOwner: $playerOwner
      character: $character
      stake: $stake
    )
  }
`;

export const CONFIRM_BATTLE = gql`
  mutation ConfirmBattle($offerId: String!) {
    confirmBattle(offerId: $offerId)
  }
`;
```

**Component Example:**
```tsx
// web-frontend/src/components/MatchmakingQueue.tsx
import React, { useState } from 'react';
import { useQuery, useMutation } from '@apollo/client';
import { useLinera } from '../contexts/LineraContext';
import {
  GET_MATCHMAKING_STATS,
  GET_PENDING_BATTLES,
  JOIN_QUEUE,
  CONFIRM_BATTLE,
} from '../graphql/matchmaking';

export function MatchmakingQueue() {
  const { clients, chainId } = useLinera();
  const [selectedCharacter, setSelectedCharacter] = useState(null);
  const [stake, setStake] = useState('100');

  const { data: stats } = useQuery(GET_MATCHMAKING_STATS, {
    client: clients.matchmaking,
    pollInterval: 2000, // Refresh every 2 seconds
  });

  const { data: battles } = useQuery(GET_PENDING_BATTLES, {
    client: clients.matchmaking,
    pollInterval: 2000,
  });

  const [joinQueue] = useMutation(JOIN_QUEUE, {
    client: clients.matchmaking,
  });

  const [confirmBattle] = useMutation(CONFIRM_BATTLE, {
    client: clients.matchmaking,
  });

  const handleJoinQueue = async () => {
    if (!selectedCharacter) return;

    await joinQueue({
      variables: {
        playerChain: chainId,
        playerOwner: /* get from wallet */,
        character: selectedCharacter,
        stake,
      },
    });
  };

  return (
    <div className="space-y-6">
      <div className="bg-surface rounded-xl p-6">
        <h2 className="text-2xl font-bold mb-4">Matchmaking Queue</h2>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <div>
            <p className="text-on-surface-muted">Waiting Players</p>
            <p className="text-3xl font-bold text-primary">
              {stats?.getMatchmakingStats.waitingPlayers || 0}
            </p>
          </div>
          <div>
            <p className="text-on-surface-muted">Pending Battles</p>
            <p className="text-3xl font-bold text-primary">
              {stats?.getMatchmakingStats.pendingBattles || 0}
            </p>
          </div>
          <div>
            <p className="text-on-surface-muted">Active Battles</p>
            <p className="text-3xl font-bold text-primary">
              {stats?.getMatchmakingStats.activeBattles || 0}
            </p>
          </div>
          <div>
            <p className="text-on-surface-muted">Total Battles</p>
            <p className="text-3xl font-bold">
              {stats?.getMatchmakingStats.totalBattles || 0}
            </p>
          </div>
        </div>
      </div>

      {/* Pending Battle Offers */}
      <div className="space-y-4">
        <h3 className="text-xl font-bold">Battle Offers</h3>
        {battles?.getPendingBattles.map((battle: any) => (
          <div key={battle.offerId} className="bg-surface rounded-xl p-4">
            <div className="flex justify-between items-center">
              <div>
                <p className="font-bold">
                  {battle.player1.character.class} vs {battle.player2.character.class}
                </p>
                <p className="text-sm text-on-surface-muted">
                  Stake: {battle.player1.stake} BATTLE
                </p>
              </div>
              <button
                onClick={() => confirmBattle({ variables: { offerId: battle.offerId } })}
                className="bg-primary text-black px-4 py-2 rounded-lg font-bold"
              >
                {battle.player1Confirmed && battle.player2Confirmed
                  ? 'Starting...'
                  : 'Confirm'}
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
```

### 3. Battle Chain

**Queries:**
```typescript
// web-frontend/src/graphql/battle.ts
import { gql } from '@apollo/client';

export const GET_BATTLE_STATE = gql`
  query GetBattleState {
    getBattleState {
      status
      currentRound
      maxRounds
      player1 {
        owner
        chain
        character {
          class
          level
          hp
        }
        currentHp
        comboStack
      }
      player2 {
        owner
        chain
        character {
          class
          level
          hp
        }
        currentHp
        comboStack
      }
      winner
      roundResults {
        round
        player1Damage
        player2Damage
        player1CriticalHit
        player2CriticalHit
      }
    }
  }
`;
```

**Mutations:**
```typescript
export const SUBMIT_TURN = gql`
  mutation SubmitTurn(
    $round: Int!
    $turn: Int!
    $stance: String!
    $useSpecial: Boolean!
  ) {
    submitTurn(
      round: $round
      turn: $turn
      stance: $stance
      useSpecial: $useSpecial
    )
  }
`;

export const EXECUTE_ROUND = gql`
  mutation ExecuteRound {
    executeRound
  }
`;
```

**Component Example:**
```tsx
// web-frontend/src/components/BattleArena.tsx
import React, { useState, useEffect } from 'react';
import { useQuery, useMutation } from '@apollo/client';
import { useLinera } from '../contexts/LineraContext';
import { GET_BATTLE_STATE, SUBMIT_TURN, EXECUTE_ROUND } from '../graphql/battle';

export function BattleArena({ battleChainId }: { battleChainId: string }) {
  const { clients } = useLinera();
  const [selectedStance, setSelectedStance] = useState('Balanced');
  const [useSpecial, setUseSpecial] = useState(false);

  const { data, loading, refetch } = useQuery(GET_BATTLE_STATE, {
    client: clients.battle, // Need to create client for specific battle chain
    pollInterval: 1000, // Real-time updates
  });

  const [submitTurn] = useMutation(SUBMIT_TURN, {
    client: clients.battle,
    onCompleted: () => refetch(),
  });

  const [executeRound] = useMutation(EXECUTE_ROUND, {
    client: clients.battle,
    onCompleted: () => refetch(),
  });

  const battle = data?.getBattleState;

  const handleSubmitTurn = async () => {
    await submitTurn({
      variables: {
        round: battle.currentRound,
        turn: 0, // Calculate based on current turn
        stance: selectedStance,
        useSpecial,
      },
    });
  };

  if (loading) return <div>Loading battle...</div>;
  if (!battle) return <div>Battle not found</div>;

  return (
    <div className="battle-arena">
      {/* Battle Header */}
      <div className="bg-surface rounded-xl p-6 mb-4">
        <h2 className="text-2xl font-bold">
          Round {battle.currentRound} / {battle.maxRounds}
        </h2>
        <p className="text-on-surface-muted">
          Status: {battle.status}
        </p>
      </div>

      {/* Players */}
      <div className="grid grid-cols-2 gap-4 mb-6">
        {/* Player 1 */}
        <div className="bg-surface rounded-xl p-6">
          <h3 className="text-xl font-bold mb-2">
            {battle.player1.character.class}
          </h3>
          <div className="space-y-2">
            <div>
              <p className="text-sm text-on-surface-muted">Health</p>
              <div className="h-4 bg-surface-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500"
                  style={{
                    width: `${(battle.player1.currentHp / battle.player1.character.hp) * 100}%`,
                  }}
                />
              </div>
              <p className="text-sm">
                {battle.player1.currentHp} / {battle.player1.character.hp}
              </p>
            </div>
            <p>Combo: {battle.player1.comboStack}x</p>
          </div>
        </div>

        {/* Player 2 */}
        <div className="bg-surface rounded-xl p-6">
          <h3 className="text-xl font-bold mb-2">
            {battle.player2.character.class}
          </h3>
          <div className="space-y-2">
            <div>
              <p className="text-sm text-on-surface-muted">Health</p>
              <div className="h-4 bg-surface-muted rounded-full overflow-hidden">
                <div
                  className="h-full bg-green-500"
                  style={{
                    width: `${(battle.player2.currentHp / battle.player2.character.hp) * 100}%`,
                  }}
                />
              </div>
              <p className="text-sm">
                {battle.player2.currentHp} / {battle.player2.character.hp}
              </p>
            </div>
            <p>Combo: {battle.player2.comboStack}x</p>
          </div>
        </div>
      </div>

      {/* Turn Submission */}
      {battle.status === 'InProgress' && (
        <div className="bg-surface rounded-xl p-6 space-y-4">
          <h3 className="text-lg font-bold">Your Turn</h3>

          {/* Stance Selection */}
          <div>
            <p className="text-sm text-on-surface-muted mb-2">Select Stance</p>
            <div className="grid grid-cols-4 gap-2">
              {['Aggressive', 'Defensive', 'Balanced', 'Reckless'].map((stance) => (
                <button
                  key={stance}
                  onClick={() => setSelectedStance(stance)}
                  className={`px-4 py-2 rounded-lg font-medium ${
                    selectedStance === stance
                      ? 'bg-primary text-black'
                      : 'bg-surface-muted text-on-surface-muted'
                  }`}
                >
                  {stance}
                </button>
              ))}
            </div>
          </div>

          {/* Special Move */}
          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="special"
              checked={useSpecial}
              onChange={(e) => setUseSpecial(e.target.checked)}
              className="w-4 h-4"
            />
            <label htmlFor="special" className="text-sm">
              Use Special Move
            </label>
          </div>

          {/* Submit Button */}
          <button
            onClick={handleSubmitTurn}
            className="w-full bg-primary text-black py-3 rounded-lg font-bold hover:bg-primary-light"
          >
            Submit Turn
          </button>
        </div>
      )}

      {/* Round History */}
      <div className="mt-6 bg-surface rounded-xl p-6">
        <h3 className="text-lg font-bold mb-4">Battle Log</h3>
        <div className="space-y-2">
          {battle.roundResults.map((result: any, idx: number) => (
            <div key={idx} className="text-sm">
              <span className="text-primary font-bold">Round {result.round}:</span>
              <span className="ml-2">
                P1: {result.player1Damage} dmg
                {result.player1CriticalHit && ' (CRIT!)'}
              </span>
              <span className="mx-2">vs</span>
              <span>
                P2: {result.player2Damage} dmg
                {result.player2CriticalHit && ' (CRIT!)'}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Winner Display */}
      {battle.winner && (
        <div className="mt-6 bg-primary text-black rounded-xl p-6 text-center">
          <h2 className="text-3xl font-bold">Winner!</h2>
          <p className="text-xl mt-2">{battle.winner}</p>
        </div>
      )}
    </div>
  );
}
```

### 4. Registry Chain (Leaderboards)

**Queries:**
```typescript
// web-frontend/src/graphql/registry.ts
import { gql } from '@apollo/client';

export const GET_LEADERBOARD = gql`
  query GetLeaderboard($limit: Int!) {
    getLeaderboard(limit: $limit) {
      characterId
      owner
      ownerChain
      class
      level
      eloRating
      wins
      losses
      winRate
      winStreak
      bestWinStreak
      totalDamageDealt
      totalEarnings
    }
  }
`;

export const GET_REGISTRY_STATS = gql`
  query GetRegistryStats {
    getGlobalStats {
      totalCharacters
      totalBattles
      totalVolume
    }
  }
`;
```

**Component Example:**
```tsx
// web-frontend/src/components/Leaderboard.tsx
import React from 'react';
import { useQuery } from '@apollo/client';
import { useLinera } from '../contexts/LineraContext';
import { GET_LEADERBOARD } from '../graphql/registry';

export function Leaderboard() {
  const { clients } = useLinera();
  const { data, loading } = useQuery(GET_LEADERBOARD, {
    client: clients.registry,
    variables: { limit: 20 },
    pollInterval: 10000, // Update every 10 seconds
  });

  if (loading) return <div>Loading leaderboard...</div>;

  return (
    <div className="bg-surface rounded-xl p-6">
      <h2 className="text-2xl font-bold mb-6">🏆 Top Players</h2>
      <div className="space-y-3">
        {data?.getLeaderboard.map((player: any, idx: number) => (
          <div
            key={player.characterId}
            className="flex items-center gap-4 p-4 bg-surface-muted rounded-lg"
          >
            <span className="text-2xl font-bold text-primary w-8">
              {idx + 1}
            </span>
            <div className="flex-grow">
              <p className="font-bold">{player.class}</p>
              <p className="text-sm text-on-surface-muted">
                Level {player.level} • ELO {player.eloRating}
              </p>
            </div>
            <div className="text-right">
              <p className="font-bold text-primary">
                {player.wins}W - {player.losses}L
              </p>
              <p className="text-sm text-on-surface-muted">
                {(player.winRate * 100).toFixed(1)}% WR
              </p>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
```

### 5. Prediction Chain

**Queries:**
```typescript
// web-frontend/src/graphql/prediction.ts
import { gql } from '@apollo/client';

export const GET_MARKET = gql`
  query GetMarket($marketId: String!) {
    getMarket(marketId: $marketId) {
      marketId
      battleChain
      player1Chain
      player2Chain
      status
      player1Pool
      player2Pool
      totalPool
      createdAt
      closedAt
      settledAt
      winner
      platformFeeBps
    }
  }
`;

export const GET_MY_BETS = gql`
  query GetMyBets($bettor: String!) {
    getMyBets(bettor: $bettor) {
      marketId
      bettor
      side
      amount
      timestamp
      claimed
    }
  }
`;
```

**Mutations:**
```typescript
export const PLACE_BET = gql`
  mutation PlaceBet($marketId: String!, $side: String!, $amount: String!) {
    placeBet(marketId: $marketId, side: $side, amount: $amount)
  }
`;

export const CLAIM_WINNINGS = gql`
  mutation ClaimWinnings($marketId: String!, $bettorChain: String!) {
    claimWinnings(marketId: $marketId, bettorChain: $bettorChain)
  }
`;
```

**Component Example:**
```tsx
// web-frontend/src/components/PredictionMarket.tsx
import React, { useState } from 'react';
import { useQuery, useMutation } from '@apollo/client';
import { useLinera } from '../contexts/LineraContext';
import { GET_MARKET, PLACE_BET } from '../graphql/prediction';

export function PredictionMarket({ marketId }: { marketId: string }) {
  const { clients, chainId } = useLinera();
  const [betSide, setBetSide] = useState<'Player1' | 'Player2'>('Player1');
  const [betAmount, setBetAmount] = useState('10');

  const { data, loading, refetch } = useQuery(GET_MARKET, {
    client: clients.prediction,
    variables: { marketId },
    pollInterval: 2000,
  });

  const [placeBet] = useMutation(PLACE_BET, {
    client: clients.prediction,
    onCompleted: () => refetch(),
  });

  const market = data?.getMarket;

  const calculateOdds = (side: 'Player1' | 'Player2') => {
    if (!market) return 0;
    const total = parseFloat(market.totalPool);
    const pool = parseFloat(
      side === 'Player1' ? market.player1Pool : market.player2Pool
    );
    if (pool === 0) return 2.0;
    return total / pool;
  };

  const handlePlaceBet = async () => {
    await placeBet({
      variables: {
        marketId,
        side: betSide,
        amount: betAmount,
      },
    });
    alert('Bet placed!');
  };

  if (loading) return <div>Loading market...</div>;
  if (!market) return <div>Market not found</div>;

  return (
    <div className="bg-surface rounded-xl p-6 space-y-4">
      <div>
        <h3 className="text-xl font-bold mb-2">Prediction Market</h3>
        <p className="text-sm text-on-surface-muted">
          Status: {market.status}
        </p>
      </div>

      {/* Odds Display */}
      <div className="grid grid-cols-2 gap-4">
        <div className="bg-surface-muted rounded-lg p-4">
          <p className="text-sm text-on-surface-muted">Player 1</p>
          <p className="text-3xl font-bold text-primary">
            {calculateOdds('Player1').toFixed(2)}x
          </p>
          <p className="text-sm text-on-surface-muted">
            Pool: {market.player1Pool} BATTLE
          </p>
        </div>
        <div className="bg-surface-muted rounded-lg p-4">
          <p className="text-sm text-on-surface-muted">Player 2</p>
          <p className="text-3xl font-bold text-primary">
            {calculateOdds('Player2').toFixed(2)}x
          </p>
          <p className="text-sm text-on-surface-muted">
            Pool: {market.player2Pool} BATTLE
          </p>
        </div>
      </div>

      {/* Betting Interface */}
      {market.status === 'Open' && (
        <div className="space-y-4 pt-4 border-t border-white/10">
          <div>
            <p className="text-sm text-on-surface-muted mb-2">Bet On</p>
            <div className="grid grid-cols-2 gap-2">
              <button
                onClick={() => setBetSide('Player1')}
                className={`py-2 rounded-lg font-medium ${
                  betSide === 'Player1'
                    ? 'bg-primary text-black'
                    : 'bg-surface-muted text-on-surface-muted'
                }`}
              >
                Player 1
              </button>
              <button
                onClick={() => setBetSide('Player2')}
                className={`py-2 rounded-lg font-medium ${
                  betSide === 'Player2'
                    ? 'bg-primary text-black'
                    : 'bg-surface-muted text-on-surface-muted'
                }`}
              >
                Player 2
              </button>
            </div>
          </div>

          <div>
            <p className="text-sm text-on-surface-muted mb-2">Amount</p>
            <input
              type="number"
              value={betAmount}
              onChange={(e) => setBetAmount(e.target.value)}
              className="w-full bg-surface-muted border border-white/10 rounded-lg px-4 py-2"
              placeholder="0.00"
            />
          </div>

          <button
            onClick={handlePlaceBet}
            className="w-full bg-primary text-black py-3 rounded-lg font-bold hover:bg-primary-light"
          >
            Place Bet
          </button>

          <p className="text-xs text-on-surface-muted text-center">
            Potential winnings: {(parseFloat(betAmount) * calculateOdds(betSide)).toFixed(2)} BATTLE
          </p>
        </div>
      )}

      {/* Market Closed */}
      {market.status === 'Closed' && (
        <div className="text-center text-on-surface-muted">
          <p>Betting closed - Battle in progress</p>
        </div>
      )}

      {/* Market Settled */}
      {market.status === 'Settled' && market.winner && (
        <div className="text-center">
          <p className="text-xl font-bold text-primary">Winner: {market.winner}</p>
        </div>
      )}
    </div>
  );
}
```

---

## 📱 Complete Component Examples

### App.tsx - Main Application

```tsx
// web-frontend/src/App.tsx
import React from 'react';
import { LineraProvider } from './contexts/LineraContext';
import { BrowserRouter, Routes, Route } from 'react-router-dom';
import { Dashboard } from './pages/Dashboard';
import { Battle } from './pages/Battle';
import { Leaderboard } from './pages/Leaderboard';
import { Marketplace } from './pages/Marketplace';

function App() {
  return (
    <LineraProvider>
      <BrowserRouter>
        <div className="min-h-screen bg-background text-on-surface">
          <Routes>
            <Route path="/" element={<Dashboard />} />
            <Route path="/battle/:battleId" element={<Battle />} />
            <Route path="/leaderboard" element={<Leaderboard />} />
            <Route path="/marketplace" element={<Marketplace />} />
          </Routes>
        </div>
      </BrowserRouter>
    </LineraProvider>
  );
}

export default App;
```

### Dashboard Page

```tsx
// web-frontend/src/pages/Dashboard.tsx
import React from 'react';
import { CharacterSelector } from '../components/CharacterSelector';
import { MatchmakingQueue } from '../components/MatchmakingQueue';
import { PlayerStats } from '../components/PlayerStats';
import { ActiveBattles } from '../components/ActiveBattles';

export function Dashboard() {
  return (
    <div className="container mx-auto px-4 py-8">
      <h1 className="text-4xl font-bold mb-8">BattleChain Dashboard</h1>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Left Column */}
        <div className="lg:col-span-2 space-y-6">
          <CharacterSelector />
          <MatchmakingQueue />
          <ActiveBattles />
        </div>

        {/* Right Column */}
        <div className="space-y-6">
          <PlayerStats />
        </div>
      </div>
    </div>
  );
}
```

---

## ✅ Best Practices

### 1. **Error Handling**
```typescript
const { data, loading, error } = useQuery(QUERY, {
  client: clients.player,
  onError: (error) => {
    console.error('Query failed:', error);
    // Show user-friendly error message
    toast.error('Failed to load data');
  },
});
```

### 2. **Polling for Real-time Updates**
```typescript
const { data } = useQuery(GET_BATTLE_STATE, {
  pollInterval: 1000, // Update every second for active battles
  skip: !battleActive, // Don't poll if battle isn't active
});
```

### 3. **Optimistic Updates**
```typescript
const [submitTurn] = useMutation(SUBMIT_TURN, {
  optimisticResponse: {
    submitTurn: {
      __typename: 'TurnResult',
      success: true,
    },
  },
  update(cache, { data }) {
    // Update cache immediately for instant UI feedback
  },
});
```

### 4. **Loading States**
```typescript
if (loading) {
  return (
    <div className="animate-pulse">
      <div className="h-4 bg-surface-muted rounded w-3/4 mb-2" />
      <div className="h-4 bg-surface-muted rounded w-1/2" />
    </div>
  );
}
```

### 5. **Type Safety**
```typescript
// Generate types from GraphQL schema
// Use codegen: npm install -D @graphql-codegen/cli
// graphql-codegen.yml:
// schema: http://localhost:8080/chains/${CHAIN_ID}/applications/${APP_ID}
// generates:
//   src/generated/graphql.ts:
//     plugins:
//       - typescript
//       - typescript-operations
//       - typescript-react-apollo
```

---

## 🎯 Next Steps

1. **Add Query Helpers to Smart Contracts**
   - Add convenience queries to each chain's service.rs
   - Example: `getPlayerDashboard()` that returns all player data at once

2. **Build UI Components**
   - Character selection and creation
   - Battle arena with real-time updates
   - Matchmaking queue interface
   - Leaderboards
   - Prediction market betting interface

3. **Add Subscriptions** (Optional)
   - Real-time battle updates via WebSocket
   - Live leaderboard changes
   - Queue updates

4. **Testing**
   - Unit tests for components
   - Integration tests with mock GraphQL
   - E2E tests with Cypress

5. **Deployment**
   - Deploy frontend to Vercel/Netlify
   - Configure environment variables
   - Set up CI/CD pipeline

---

**Ready to build the frontend!** 🚀

Start with the `LineraProvider` context, then build out components one chain at a time.
