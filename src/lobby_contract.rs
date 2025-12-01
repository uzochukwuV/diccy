use linera_sdk::{
    linera_base_types::{Amount, AccountOwner, ChainId},
    ContractRuntime,
};

use majorules::{Operation, Message};
use crate::state::LobbyState;

pub struct LobbyContract;

impl LobbyContract {
    pub async fn execute_operation(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        operation: Operation,
    ) {
        match operation {
            Operation::Increment { value } => {
                state.value.set(state.value.get() + value);
            }

            Operation::CreatePlayerChain => {
                let caller = runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                
                // Create single-owner player chain with proper instantiation
                let player_chain_id = runtime.open_chain(
                    linera_sdk::linera_base_types::ChainOwnership::single(caller),
                    linera_sdk::linera_base_types::ApplicationPermissions::default(),
                    Amount::ZERO,
                );
                
                // Initialize as Player chain via instantiation argument
                let init_arg = majorules::InitializationArgument {
                    variant: majorules::ChainVariant::Player,
                    treasury_owner: None,
                    platform_fee_bps: None,
                };
                
                runtime.prepare_message(majorules::Message::InstantiateChain {
                    variant: init_arg.variant.clone(),
                    treasury_owner: init_arg.treasury_owner,
                    platform_fee_bps: init_arg.platform_fee_bps,
                }).with_authentication().send_to(player_chain_id);

                // Register player's chain ID
                state.character_registry.insert(
                    &caller.to_string(), 
                    crate::state::CharacterRegistryEntry {
                        character_id: String::new(),
                        owner: caller,
                        owner_chain: player_chain_id,
                        class: crate::state::CharacterClass::Warrior,
                        level: 1,
                        created_at: runtime.system_time(),
                        total_battles: 0,
                        wins: 0,
                        losses: 0,
                        is_alive: true,
                        lives_remaining: 3,
                    }
                ).expect("Failed to register player chain");

                // Initialize player chain with lobby reference
                let lobby_chain_id = runtime.chain_id();
                runtime.prepare_message(Message::InitializePlayerChain {
                    lobby_chain_id,
                    owner: caller,
                }).with_authentication().send_to(player_chain_id);
            }

            Operation::LeaveQueue => {
                let caller = runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                
                // Remove from queue
                state.waiting_players.remove(&caller).ok();
                
                // Decrement counter
                if *state.value.get() > 0 {
                    state.value.set(state.value.get() - 1);
                }
            }

            Operation::UpdateLeaderboard { player } => {
                // Update player stats from their player chain
                if let Some(player_chain) = Self::get_player_chain(&player, state).await {
                    // Send message to player chain to get updated stats
                    runtime.prepare_message(Message::RequestPlayerStats { player })
                        .with_authentication()
                        .send_to(player_chain);
                }
            }
            
            Operation::PlaceBet { market_id, predicted_winner, amount } => {
                let caller = runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                    
                Self::place_bet(state, runtime, caller, market_id, predicted_winner, amount).await;
            }
            
            Operation::CloseMarket { market_id } => {
                Self::close_market(state, runtime, market_id).await;
            }

            _ => {
                // Ignore operations not relevant to lobby
            }
        }
    }

    pub async fn execute_message(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        message: Message,
    ) {
        match message {
            Message::RequestJoinQueue { player, player_chain, character_snapshot, stake } => {
                // Verify message comes from the player's chain
                let sender_chain = runtime.message_origin_chain_id()
                    .expect("Message must have origin");
                if sender_chain != player_chain {
                    return; // Reject unauthorized requests
                }

                // Check if already in queue
                if state.waiting_players.contains_key(&player).await.unwrap_or(false) {
                    return; // Already in queue
                }

                // Validate stake
                if stake <= Amount::ZERO {
                    return; // Invalid stake
                }

                // Player chain provides character data
                let now = runtime.system_time();
                let queue_entry = crate::state::PlayerQueueEntry {
                    player,
                    player_chain,
                    character_id: character_snapshot.nft_id.clone(),
                    character_snapshot: crate::state::CharacterSnapshot {
                        nft_id: character_snapshot.nft_id,
                        class: match character_snapshot.class {
                            majorules::CharacterClass::Warrior => crate::state::CharacterClass::Warrior,
                            majorules::CharacterClass::Mage => crate::state::CharacterClass::Mage,
                            _ => crate::state::CharacterClass::Warrior,
                        },
                        level: character_snapshot.level,
                        hp_max: character_snapshot.hp_max,
                        min_damage: character_snapshot.min_damage,
                        max_damage: character_snapshot.max_damage,
                        crit_chance: character_snapshot.crit_chance,
                        crit_multiplier: character_snapshot.crit_multiplier,
                        dodge_chance: character_snapshot.dodge_chance,
                        defense: character_snapshot.defense,
                        attack_bps: character_snapshot.attack_bps,
                        defense_bps: character_snapshot.defense_bps,
                        crit_bps: character_snapshot.crit_bps,
                    },
                    stake,
                    joined_at: now,
                };

                state.waiting_players.insert(&player, queue_entry)
                    .expect("Failed to add player to queue");

                // Check for ELO-based matchmaking
                let queue_count = state.waiting_players.count().await.unwrap_or(0);
                if queue_count >= 2 {
                    Self::attempt_elo_matchmaking(state, runtime).await;
                }
            }

            Message::BattleResultWithElo { player, opponent: _, won, payout: _, xp_gained, elo_change, battle_stats: _, battle_chain } => {
                // Verify message comes from a valid battle chain
                let sender_chain = runtime.message_origin_chain_id()
                    .expect("Message must have origin");
                
                // Check if this battle chain exists in our active battles
                if !state.active_battles.contains_key(&sender_chain).await.unwrap_or(false) {
                    return; // Reject unauthorized battle results
                }
                
                // Forward ELO update directly to player chain (lobby doesn't store stats)
                if let Some(player_chain) = Self::get_player_chain(&player, state).await {
                    runtime.prepare_message(Message::UpdatePlayerStats {
                        player,
                        won,
                        xp_gained,
                        elo_change,
                        battle_chain,
                    }).with_authentication().send_to(player_chain);
                }
            }
            
            Message::BattleCompleted { winner, loser, rounds_played, total_stake, battle_stats: _ } => {
                let sender_chain = runtime.message_origin_chain_id()
                    .expect("Message must have origin");
                    
                // Handle battle completion separately from prediction market
                Self::handle_battle_completion(state, runtime, sender_chain, winner, loser, rounds_played, total_stake).await;
            }



            Message::PlayerStatsResponse { player, stats } => {
                // Use player stats for matchmaking (don't store permanently)
                // This is used temporarily for ELO-based matchmaking
            }

            _ => {
                // Ignore other message types
            }
        }
    }

    async fn get_player_chain(player: &AccountOwner, state: &LobbyState) -> Option<ChainId> {
        if let Ok(Some(entry)) = state.character_registry.get(&player.to_string()).await {
            Some(entry.owner_chain)
        } else {
            None
        }
    }

    async fn create_battle_chain(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        player1: crate::state::PlayerQueueEntry,
        player2: crate::state::PlayerQueueEntry,
    ) {
        use linera_sdk::linera_base_types::{ChainOwnership, ApplicationPermissions};

        // Create multi-owner battle chain with proper instantiation
        let battle_chain_id = runtime.open_chain(
            ChainOwnership::multiple(
                vec![
                    (player1.player, 1u64),
                    (player2.player, 1u64),
                ].into_iter(),
                10, // multi_leader_rounds
                Default::default(), // timeout_config
            ),
            ApplicationPermissions::default(),
            Amount::ZERO,
        );
        
        // Initialize as Battle chain via instantiation argument
        let init_arg = majorules::InitializationArgument {
            variant: majorules::ChainVariant::Battle,
            treasury_owner: Some(state.treasury_owner.get().unwrap()),
            platform_fee_bps: Some(*state.platform_fee_bps.get()),
        };
        
        runtime.prepare_message(majorules::Message::InstantiateChain {
            variant: init_arg.variant.clone(),
            treasury_owner: init_arg.treasury_owner,
            platform_fee_bps: init_arg.platform_fee_bps,
        }).with_authentication().send_to(battle_chain_id);

        // Send initialization message to battle chain
        let participant1 = majorules::BattleParticipant::new(
            player1.player,
            player1.player_chain,
            majorules::CharacterSnapshot {
                nft_id: player1.character_snapshot.nft_id,
                class: match player1.character_snapshot.class {
                    crate::state::CharacterClass::Warrior => majorules::CharacterClass::Warrior,
                    crate::state::CharacterClass::Mage => majorules::CharacterClass::Mage,
                    _ => majorules::CharacterClass::Warrior,
                },
                level: player1.character_snapshot.level,
                hp_max: player1.character_snapshot.hp_max,
                min_damage: player1.character_snapshot.min_damage,
                max_damage: player1.character_snapshot.max_damage,
                crit_chance: player1.character_snapshot.crit_chance,
                crit_multiplier: player1.character_snapshot.crit_multiplier,
                dodge_chance: player1.character_snapshot.dodge_chance,
                defense: player1.character_snapshot.defense,
                attack_bps: player1.character_snapshot.attack_bps,
                defense_bps: player1.character_snapshot.defense_bps,
                crit_bps: player1.character_snapshot.crit_bps,
            },
            player1.stake,
        );

        let participant2 = majorules::BattleParticipant::new(
            player2.player,
            player2.player_chain,
            majorules::CharacterSnapshot {
                nft_id: player2.character_snapshot.nft_id,
                class: match player2.character_snapshot.class {
                    crate::state::CharacterClass::Warrior => majorules::CharacterClass::Warrior,
                    crate::state::CharacterClass::Mage => majorules::CharacterClass::Mage,
                    _ => majorules::CharacterClass::Warrior,
                },
                level: player2.character_snapshot.level,
                hp_max: player2.character_snapshot.hp_max,
                min_damage: player2.character_snapshot.min_damage,
                max_damage: player2.character_snapshot.max_damage,
                crit_chance: player2.character_snapshot.crit_chance,
                crit_multiplier: player2.character_snapshot.crit_multiplier,
                dodge_chance: player2.character_snapshot.dodge_chance,
                defense: player2.character_snapshot.defense,
                attack_bps: player2.character_snapshot.attack_bps,
                defense_bps: player2.character_snapshot.defense_bps,
                crit_bps: player2.character_snapshot.crit_bps,
            },
            player2.stake,
        );

        let lobby_chain_id = runtime.chain_id();
        let platform_fee_bps = *state.platform_fee_bps.get();
        let treasury_owner = state.treasury_owner.get().unwrap();
        
        runtime.prepare_message(Message::InitializeBattle {
            player1: participant1,
            player2: participant2,
            lobby_chain_id,
            platform_fee_bps,
            treasury_owner,
        }).with_authentication().send_to(battle_chain_id);

        // Track active battle
        let battle_metadata = crate::state::BattleMetadata {
            battle_chain: battle_chain_id,
            player1: player1.player,
            player2: player2.player,
            total_stake: player1.stake.saturating_add(player2.stake),
            created_at: runtime.system_time(),
            status: crate::state::BattleStatus::InProgress,
            has_prediction_market: true,
        };

        state.active_battles.insert(&battle_chain_id, battle_metadata)
            .expect("Failed to track battle");
            
        // Create prediction market separately
        let market_id = Self::create_prediction_market_in_lobby(state, runtime, battle_chain_id, player1.player_chain, player2.player_chain).await;
        
        // Link battle to market for tracking
        state.battle_to_market.insert(&battle_chain_id, market_id)
            .expect("Failed to link battle to market");
    }
    
    /// Attempt ELO-based matchmaking by requesting player stats
    async fn attempt_elo_matchmaking(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
    ) {
        // For now, use simple level-based matching from character snapshots
        // In full implementation, would request ELO from player chains first
        let mut players_with_level = Vec::new();
        
        state.waiting_players.for_each_index_value(|owner, entry| {
            let level = entry.character_snapshot.level;
            players_with_level.push((owner.clone(), entry.into_owned(), level));
            Ok(())
        }).await.unwrap_or(());
        
        // Sort by character level as ELO proxy
        players_with_level.sort_by_key(|(_, _, level)| *level);
        
        // Find best match pairs (closest levels)
        for i in 0..players_with_level.len() {
            for j in (i + 1)..players_with_level.len() {
                let (_, _, level1) = &players_with_level[i];
                let (_, _, level2) = &players_with_level[j];
                
                // Match players within 10 levels for fair games
                let level_diff = if level1 > level2 { level1 - level2 } else { level2 - level1 };
                
                if level_diff <= 10 {
                    let (player1_owner, player1_entry, _) = players_with_level[i].clone();
                    let (player2_owner, player2_entry, _) = players_with_level[j].clone();
                    
                    // Remove both players from queue
                    state.waiting_players.remove(&player1_owner).ok();
                    state.waiting_players.remove(&player2_owner).ok();
                    
                    // Create battle
                    Self::create_battle_chain(state, runtime, player1_entry, player2_entry).await;
                    return; // Match found, exit
                }
            }
        }
        
        // If no close level match found and queue has been waiting too long, match anyway
        if players_with_level.len() >= 2 {
            let now = runtime.system_time();
            let oldest_wait = players_with_level.iter()
                .map(|(_, entry, _)| now.delta_since(entry.joined_at).as_micros() / 1_000_000)
                .max()
                .unwrap_or(0);
            
            // After 60 seconds, match regardless of level difference
            if oldest_wait >= 60 {
                let (player1_owner, player1_entry, _) = players_with_level[0].clone();
                let (player2_owner, player2_entry, _) = players_with_level[1].clone();
                
                state.waiting_players.remove(&player1_owner).ok();
                state.waiting_players.remove(&player2_owner).ok();
                
                Self::create_battle_chain(state, runtime, player1_entry, player2_entry).await;
            }
        }
    }
    
    /// Create prediction market in lobby for battle
    async fn create_prediction_market_in_lobby(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        battle_chain: ChainId,
        player1_chain: ChainId,
        player2_chain: ChainId,
    ) -> u64 {
        // Generate unique market ID
        let current_market_count = state.market_count.get();
        let market_id = current_market_count + 1;
        state.market_count.set(market_id);
        
        // Create market with separate lifecycle from battle
        let market = crate::state::Market {
            market_id,
            battle_chain,
            player1_chain,
            player2_chain,
            status: crate::state::MarketStatus::Open,
            total_pool: Amount::ZERO,
            player1_pool: Amount::ZERO,
            player2_pool: Amount::ZERO,
            winner_chain: None,
            created_at: runtime.system_time(),
            closed_at: None,
            settled_at: None,
        };
        
        // Store market separately from battle tracking
        state.prediction_markets.insert(&market_id, market)
            .expect("Failed to create prediction market");
            
        market_id
    }
    
    /// Place bet on battle outcome
    async fn place_bet(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        bettor: AccountOwner,
        market_id: u64,
        predicted_winner: ChainId,
        amount: Amount,
    ) {
        // Get market and validate
        if let Ok(Some(mut market)) = state.prediction_markets.get(&market_id).await {
            if market.status != crate::state::MarketStatus::Open {
                return; // Market closed
            }
            
            // Create bet
            let bet = crate::state::Bet {
                bettor,
                market_id,
                predicted_winner,
                amount,
                odds_at_bet: 10000, // 1:1 odds for simplicity
                placed_at: runtime.system_time(),
                claimed: false,
            };
            
            // Update market pools
            market.total_pool = market.total_pool.saturating_add(amount);
            if predicted_winner == market.player1_chain {
                market.player1_pool = market.player1_pool.saturating_add(amount);
            } else {
                market.player2_pool = market.player2_pool.saturating_add(amount);
            }
            
            // Store bet and update market
            state.bets.insert(&(market_id, bettor), bet)
                .expect("Failed to place bet");
            state.prediction_markets.insert(&market_id, market)
                .expect("Failed to update market");
                
            // Update total volume
            let current_volume = state.total_betting_volume.get();
            state.total_betting_volume.set(current_volume.saturating_add(amount));
        }
    }
    
    /// Handle battle completion with separate tracking
    async fn handle_battle_completion(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        battle_chain: ChainId,
        winner: AccountOwner,
        _loser: AccountOwner,
        rounds_played: u8,
        total_stake: Amount,
    ) {
        // Get battle metadata before removing
        if let Ok(Some(battle_metadata)) = state.active_battles.get(&battle_chain).await {
            // Update platform revenue
            let platform_fee_bps = state.platform_fee_bps.get();
            let total_attos = u128::from(total_stake);
            let fee_attos = total_attos.saturating_mul(*platform_fee_bps as u128) / 10000;
            let platform_fee = Amount::from_attos(fee_attos);
            
            let current_revenue = state.total_platform_revenue.get();
            state.total_platform_revenue.set(current_revenue.saturating_add(platform_fee));
            
            // Get prediction market info if exists
            let (market_id, betting_volume) = if let Ok(Some(market_id)) = state.battle_to_market.get(&battle_chain).await {
                let volume = if let Ok(Some(market)) = state.prediction_markets.get(&market_id).await {
                    market.total_pool
                } else {
                    Amount::ZERO
                };
                (Some(market_id), volume)
            } else {
                (None, Amount::ZERO)
            };
            
            // Create completed battle record
            let completed_record = crate::state::CompletedBattleRecord {
                battle_chain,
                player1: battle_metadata.player1,
                player2: battle_metadata.player2,
                winner,
                total_stake,
                rounds_played,
                created_at: battle_metadata.created_at,
                completed_at: runtime.system_time(),
                prediction_market_id: market_id,
                total_betting_volume: betting_volume,
            };
            
            // Move from active to completed
            state.completed_battles.insert(&battle_chain, completed_record)
                .expect("Failed to record completed battle");
            state.active_battles.remove(&battle_chain).ok();
            
            // Handle prediction market settlement separately
            if let Some(market_id) = market_id {
                Self::settle_prediction_market(state, runtime, market_id, winner).await;
            }
        }
    }
    
    /// Settle prediction market separately from battle
    async fn settle_prediction_market(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        market_id: u64,
        winner: AccountOwner,
    ) {
        if let Ok(Some(mut market)) = state.prediction_markets.get(&market_id).await {
            // Determine winner chain from battle result
            // Find winner chain by comparing with battle participants
            let winner_chain = if let Ok(Some(battle)) = state.active_battles.get(&market.battle_chain).await {
                if winner == battle.player1 {
                    market.player1_chain
                } else {
                    market.player2_chain
                }
            } else {
                market.player1_chain // fallback
            };
            
            market.status = crate::state::MarketStatus::Settled;
            market.winner_chain = Some(winner_chain);
            market.settled_at = Some(runtime.system_time());
            
            state.prediction_markets.insert(&market_id, market)
                .expect("Failed to settle market");
                
            // TODO: Distribute winnings to bettors
        }
    }
    
    /// Close market when battle starts
    async fn close_market(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        market_id: u64,
    ) {
        if let Ok(Some(mut market)) = state.prediction_markets.get(&market_id).await {
            market.status = crate::state::MarketStatus::Closed;
            market.closed_at = Some(runtime.system_time());
            
            state.prediction_markets.insert(&market_id, market)
                .expect("Failed to close market");
        }
    }
}