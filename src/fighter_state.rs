// Copyright (c) Fighter Game
// SPDX-License-Identifier: Apache-2.0

use async_graphql::SimpleObject;
use fighter_game::{Battle, BattleConfig, Fighter, Tournament};
use linera_sdk::{
    linera_base_types::AccountOwner,
    views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext},
};

/// The application state for the Fighter Game
#[derive(RootView, SimpleObject)]
#[graphql(complex)]
#[view(context = ViewStorageContext)]
pub struct FighterGameState {
    /// Global battle configuration
    pub config: RegisterView<BattleConfig>,
    
    /// All registered fighters by owner
    pub fighters: MapView<AccountOwner, Fighter>,
    
    /// Active battles by battle ID
    pub battles: MapView<u64, Battle>,
    
    /// Battle counter for generating unique IDs
    pub battle_counter: RegisterView<u64>,
    
    /// Active tournaments by tournament ID
    pub tournaments: MapView<u64, Tournament>,
    
    /// Tournament counter for generating unique IDs
    pub tournament_counter: RegisterView<u64>,
    
    /// Leaderboard: top fighters by XP (owner -> xp)
    pub leaderboard: MapView<AccountOwner, u64>,
    
    /// Matchmaking queue by tier
    pub matchmaking_queue: MapView<u8, Vec<AccountOwner>>,
    
    /// Platform earnings from fees
    pub platform_balance: RegisterView<u128>,
    
    /// Total battles completed
    pub total_battles: RegisterView<u64>,
    
    /// Total XP distributed
    pub total_xp_distributed: RegisterView<u64>,
}
