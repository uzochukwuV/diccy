// Copyright (c) Fighter Game
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{
    ComplexObject, Context, EmptySubscription, Object, Request, Response, Schema, SimpleObject,
};
use fighter_game::{Battle, Fighter, FighterTier, MatchmakingTier, Operation, Tournament};
use linera_sdk::{
    graphql::GraphQLMutationRoot as _,
    linera_base_types::{AccountOwner, WithServiceAbi},
    views::View,
    Service, ServiceRuntime,
};

use self::state::FighterGameState;

#[derive(Clone)]
pub struct FighterGameService {
    runtime: Arc<ServiceRuntime<FighterGameService>>,
    state: Arc<FighterGameState>,
}

linera_sdk::service!(FighterGameService);

impl WithServiceAbi for FighterGameService {
    type Abi = fighter_game::FighterGameAbi;
}

impl Service for FighterGameService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = FighterGameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        FighterGameService {
            runtime: Arc::new(runtime),
            state: Arc::new(state),
        }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
                state: self.state.clone(),
                runtime: self.runtime.clone(),
            },
            Operation::mutation_root(self.runtime.clone()),
            EmptySubscription,
        )
        .data(self.runtime.clone())
        .data(self.state.clone())
        .finish();
        
        schema.execute(request).await
    }
}

struct QueryRoot {
    state: Arc<FighterGameState>,
    runtime: Arc<ServiceRuntime<FighterGameService>>,
}

#[Object]
impl QueryRoot {
    /// Get fighter by owner address
    async fn fighter(&self, owner: AccountOwner) -> Option<Fighter> {
        self.state.fighters.get(&owner).await.ok().flatten()
    }
    
    /// Get all fighters (paginated)
    async fn fighters(&self, skip: Option<u32>, limit: Option<u32>) -> Vec<Fighter> {
        let skip = skip.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(50).min(100) as usize;
        
        let keys = self.state.fighters.keys().await.unwrap();
        let mut fighters = Vec::new();
        
        for (i, key) in keys.enumerate() {
            if i < skip {
                continue;
            }
            if fighters.len() >= limit {
                break;
            }
            if let Ok(Some(fighter)) = self.state.fighters.get(&key).await {
                fighters.push(fighter);
            }
        }
        
        fighters
    }
    
    /// Get battle by ID
    async fn battle(&self, battle_id: u64) -> Option<Battle> {
        self.state.battles.get(&battle_id).await.ok().flatten()
    }
    
    /// Get active battles (paginated)
    async fn active_battles(&self, limit: Option<u32>) -> Vec<Battle> {
        let limit = limit.unwrap_or(20).min(100) as usize;
        let keys = self.state.battles.keys().await.unwrap();
        let mut battles = Vec::new();
        
        for key in keys {
            if battles.len() >= limit {
                break;
            }
            if let Ok(Some(battle)) = self.state.battles.get(&key).await {
                if battle.status == fighter_game::BattleStatus::Active {
                    battles.push(battle);
                }
            }
        }
        
        battles
    }
    
    /// Get battles for a specific fighter
    async fn fighter_battles(&self, owner: AccountOwner, limit: Option<u32>) -> Vec<Battle> {
        let limit = limit.unwrap_or(20).min(100) as usize;
        let keys = self.state.battles.keys().await.unwrap();
        let mut battles = Vec::new();
        
        for key in keys {
            if battles.len() >= limit {
                break;
            }
            if let Ok(Some(battle)) = self.state.battles.get(&key).await {
                if battle.fighter1 == owner || battle.fighter2 == owner {
                    battles.push(battle);
                }
            }
        }
        
        battles
    }
    
    /// Get tournament by ID
    async fn tournament(&self, tournament_id: u64) -> Option<Tournament> {
        self.state.tournaments.get(&tournament_id).await.ok().flatten()
    }
    
    /// Get all tournaments (paginated)
    async fn tournaments(&self, skip: Option<u32>, limit: Option<u32>) -> Vec<Tournament> {
        let skip = skip.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(20).min(100) as usize;
        
        let keys = self.state.tournaments.keys().await.unwrap();
        let mut tournaments = Vec::new();
        
        for (i, key) in keys.enumerate() {
            if i < skip {
                continue;
            }
            if tournaments.len() >= limit {
                break;
            }
            if let Ok(Some(tournament)) = self.state.tournaments.get(&key).await {
                tournaments.push(tournament);
            }
        }
        
        tournaments
    }
    
    /// Get leaderboard (top fighters by XP)
    async fn leaderboard(&self, limit: Option<u32>) -> Vec<LeaderboardEntry> {
        let limit = limit.unwrap_or(50).min(100) as usize;
        
        let keys = self.state.leaderboard.keys().await.unwrap();
        let mut entries = Vec::new();
        
        for key in keys {
            if let Ok(Some(xp)) = self.state.leaderboard.get(&key).await {
                if let Ok(Some(fighter)) = self.state.fighters.get(&key).await {
                    entries.push(LeaderboardEntry {
                        rank: 0, // Will be set later
                        owner: key,
                        name: fighter.name,
                        level: fighter.level,
                        xp,
                        total_wins: fighter.total_wins,
                        total_losses: fighter.total_losses,
                        win_rate: Self::calculate_win_rate(fighter.total_wins, fighter.total_losses),
                        current_streak: fighter.current_streak,
                        tier: fighter.nft_tier,
                    });
                }
            }
        }
        
        // Sort by XP descending
        entries.sort_by(|a, b| b.xp.cmp(&a.xp));
        
        // Set ranks
        for (i, entry) in entries.iter_mut().enumerate() {
            entry.rank = (i + 1) as u32;
        }
        
        entries.into_iter().take(limit).collect()
    }
    
    /// Get fighter statistics
    async fn fighter_stats(&self, owner: AccountOwner) -> Option<FighterStats> {
        let fighter = self.state.fighters.get(&owner).await.ok()??;
        
        Some(FighterStats {
            owner,
            name: fighter.name.clone(),
            level: fighter.level,
            xp: fighter.xp,
            xp_to_next_level: fighter.xp_for_next_level().saturating_sub(fighter.xp),
            total_wins: fighter.total_wins,
            total_losses: fighter.total_losses,
            win_rate: Self::calculate_win_rate(fighter.total_wins, fighter.total_losses),
            total_battles: fighter.total_wins + fighter.total_losses,
            total_damage_dealt: fighter.total_damage_dealt,
            total_damage_taken: fighter.total_damage_taken,
            current_streak: fighter.current_streak,
            highest_streak: fighter.highest_streak,
            max_hp: fighter.max_hp,
            base_attack: fighter.base_attack,
            defense: fighter.defense,
            critical_chance: fighter.critical_chance,
            tier: fighter.nft_tier,
            matchmaking_tier: fighter.get_tier(),
        })
    }
    
    /// Get global statistics
    async fn global_stats(&self) -> GlobalStats {
        let total_battles = self.state.total_battles.get();
        let total_xp_distributed = self.state.total_xp_distributed.get();
        let platform_balance = self.state.platform_balance.get();
        
        // Count total fighters
        let fighter_keys = self.state.fighters.keys().await.unwrap();
        let total_fighters = fighter_keys.count() as u64;
        
        // Count active battles
        let battle_keys = self.state.battles.keys().await.unwrap();
        let mut active_battles = 0u64;
        for key in battle_keys {
            if let Ok(Some(battle)) = self.state.battles.get(&key).await {
                if battle.status == fighter_game::BattleStatus::Active {
                    active_battles += 1;
                }
            }
        }
        
        GlobalStats {
            total_fighters,
            total_battles,
            active_battles,
            total_xp_distributed,
            platform_balance,
        }
    }
}

impl QueryRoot {
    fn calculate_win_rate(wins: u32, losses: u32) -> f64 {
        let total = wins + losses;
        if total == 0 {
            0.0
        } else {
            (wins as f64 / total as f64) * 100.0
        }
    }
}

#[derive(SimpleObject)]
struct LeaderboardEntry {
    rank: u32,
    owner: AccountOwner,
    name: String,
    level: u32,
    xp: u64,
    total_wins: u32,
    total_losses: u32,
    win_rate: f64,
    current_streak: u32,
    tier: FighterTier,
}

#[derive(SimpleObject)]
struct FighterStats {
    owner: AccountOwner,
    name: String,
    level: u32,
    xp: u64,
    xp_to_next_level: u64,
    total_wins: u32,
    total_losses: u32,
    win_rate: f64,
    total_battles: u32,
    total_damage_dealt: u64,
    total_damage_taken: u64,
    current_streak: u32,
    highest_streak: u32,
    max_hp: u32,
    base_attack: u32,
    defense: u32,
    critical_chance: u32,
    tier: FighterTier,
    matchmaking_tier: MatchmakingTier,
}

#[derive(SimpleObject)]
struct GlobalStats {
    total_fighters: u64,
    total_battles: u64,
    active_battles: u64,
    total_xp_distributed: u64,
    platform_balance: u128,
}

#[ComplexObject]
impl FighterGameState {
    /// Get current battle configuration
    async fn config(&self) -> fighter_game::BattleConfig {
        self.config.get().clone()
    }
    
    /// Get next battle ID
    async fn next_battle_id(&self) -> u64 {
        self.battle_counter.get() + 1
    }
    
    /// Get next tournament ID
    async fn next_tournament_id(&self) -> u64 {
        self.tournament_counter.get() + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_graphql::{futures_util::FutureExt, Request};
    use linera_sdk::{util::BlockingWait, views::View, Service, ServiceRuntime};
    use serde_json::json;

    #[test]
    fn query_config() {
        let runtime = ServiceRuntime::<FighterGameService>::new();
        let state = FighterGameState::load(runtime.root_view_storage_context())
            .blocking_wait()
            .expect("Failed to read from mock key value store");

        let service = FighterGameService {
            state: Arc::new(state),
            runtime: Arc::new(runtime),
        };

        let response = service
            .handle_query(Request::new("{ config { turnTimeout } }"))
            .now_or_never()
            .expect("Query should not await anything")
            .data
            .into_json()
            .expect("Response should be JSON");

        // The default config should be present
        assert!(response.get("config").is_some());
    }
}
