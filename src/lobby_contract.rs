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
                
                // Create single-owner player chain for the user
                let player_chain_id = runtime.open_chain(
                    linera_sdk::linera_base_types::ChainOwnership::single(caller),
                    linera_sdk::linera_base_types::ApplicationPermissions::default(),
                    Amount::ZERO,
                );

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

                // Check for auto-match when 2 players in queue
                let queue_count = state.waiting_players.count().await.unwrap_or(0);
                if queue_count >= 2 {
                    // Get first two players from queue
                    let mut players = Vec::new();
                    {
                        state.waiting_players.for_each_index_value(|owner, entry| {
                            if players.len() < 2 {
                                players.push((owner.clone(), entry.into_owned()));
                            }
                            Ok(())
                        }).await.unwrap_or(());
                    }

                    if players.len() == 2 {
                        let (player1_owner, player1_entry) = players[0].clone();
                        let (player2_owner, player2_entry) = players[1].clone();

                        // Remove both players from queue
                        state.waiting_players.remove(&player1_owner).ok();
                        state.waiting_players.remove(&player2_owner).ok();

                        // Create multi-owner battle chain
                        Self::create_battle_chain(state, runtime, player1_entry, player2_entry).await;
                    }
                }
            }

            Message::BattleCompleted { winner, loser, rounds_played: _, total_stake, battle_stats: _ } => {
                // Update platform revenue
                let platform_fee_bps = state.platform_fee_bps.get();
                let total_attos = u128::from(total_stake);
                let fee_attos = total_attos.saturating_mul(*platform_fee_bps as u128) / 10000;
                let platform_fee = Amount::from_attos(fee_attos);
                
                let current_revenue = state.total_platform_revenue.get();
                state.total_platform_revenue.set(current_revenue.saturating_add(platform_fee));

                // Update player stats in their chains
                if let Some(winner_chain) = Self::get_player_chain(&winner, state).await {
                    runtime.prepare_message(Message::UpdatePlayerStats {
                        player: winner,
                        won: true,
                        xp_gained: 100,
                    }).with_authentication().send_to(winner_chain);
                }

                if let Some(loser_chain) = Self::get_player_chain(&loser, state).await {
                    runtime.prepare_message(Message::UpdatePlayerStats {
                        player: loser,
                        won: false,
                        xp_gained: 25,
                    }).with_authentication().send_to(loser_chain);
                }
            }

            Message::BattleResult { winner, loser, winner_payout, xp_gained, battle_stats, battle_chain } => {
                // Forward battle result to appropriate player chain
                // Determine which player this result is for based on winner/loser and payout
                let target_player = if winner_payout > Amount::ZERO { winner } else { loser };
                
                if let Some(player_chain) = Self::get_player_chain(&target_player, state).await {
                    // Forward the battle result to the player's chain
                    runtime.prepare_message(Message::BattleResult {
                        winner,
                        loser,
                        winner_payout,
                        xp_gained,
                        battle_stats,
                        battle_chain,
                    }).with_authentication().send_to(player_chain);
                }
            }

            Message::PlayerStatsResponse { player, stats } => {
                // Update global leaderboard with player stats
                state.player_stats.insert(&player, crate::state::PlayerGlobalStats {
                    total_battles: stats.total_battles,
                    wins: stats.wins,
                    losses: stats.losses,
                    win_rate: stats.win_rate,
                    elo_rating: stats.elo_rating,
                    total_earnings: stats.total_earnings,
                    total_damage_dealt: stats.total_damage_dealt,
                    total_damage_taken: stats.total_damage_taken,
                    total_crits: stats.total_crits,
                    total_dodges: stats.total_dodges,
                    best_streak: stats.best_streak,
                    current_streak: stats.current_streak,
                    highest_crit: stats.highest_crit,
                })
                    .expect("Failed to update player stats");
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

        // Create multi-owner battle chain
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
        };

        state.active_battles.insert(&battle_chain_id, battle_metadata)
            .expect("Failed to track battle");
    }
}