#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use battlechain_shared_types::{CharacterClass, Owner};
use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::{Amount, ChainId},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use registry_chain::{
    Message, Operation, RegistryAbi, RegistryError,
};
use self::state::{RegistryState, CharacterStats, BattleRecord};

/// Registry Contract
pub struct RegistryContract {
    pub state: RegistryState,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(RegistryContract);

impl WithContractAbi for RegistryContract {
    type Abi = RegistryAbi;
}

impl Contract for RegistryContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = RegistryState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let now = self.runtime.system_time();

        // Get creator as admin
        let chain_ownership = self.runtime.chain_ownership();
        let creator = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("No super owners found")
            .clone();

        self.state.next_battle_id.set(0);
        self.state.total_characters.set(0);
        self.state.total_battles.set(0);
        self.state.total_volume.set(Amount::ZERO);
        self.state.top_elo.set(Vec::new());
        self.state.created_at.set(now);

        // SECURITY: Initialize admin and paused state
        self.state.admin.set(Some(creator));
        self.state.paused.set(false);
        log::info!("Registry initialized with admin: {:?}", creator);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        // SECURITY: Check if contract is paused (skip for admin operations)
        match operation {
            Operation::Pause | Operation::Unpause | Operation::TransferAdmin { .. } => {
                // Admin operations allowed even when paused
            }
            _ => {
                if *self.state.paused.get() {
                    return Err(RegistryError::ContractPaused);
                }
            }
        }

        match operation {
            Operation::RegisterCharacter {
                character_id,
                nft_id,
                owner,
                owner_chain,
                class,
                level,
            } => {
                // Check if already registered
                if self.state.characters.get(&character_id).await?.is_some() {
                    return Err(RegistryError::CharacterAlreadyRegistered);
                }

                let now = self.runtime.system_time();
                let stats = CharacterStats::new(
                    character_id.clone(),
                    nft_id,
                    owner,
                    owner_chain,
                    class,
                    level,
                    now,
                );

                self.state.register_character(stats)?;
                self.state.update_leaderboard(character_id).await?;
            }

            Operation::UpdateCharacterStats {
                character_id,
                won,
                damage_dealt,
                damage_taken,
                crits,
                dodges,
                highest_crit,
                earnings,
                stake,
                opponent_elo,
            } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                let now = self.runtime.system_time();
                stats.update_after_battle(
                    won,
                    damage_dealt,
                    damage_taken,
                    crits,
                    dodges,
                    highest_crit,
                    earnings,
                    stake,
                    opponent_elo,
                    now,
                );

                self.state.characters.insert(&character_id, stats)?;
                self.state.update_leaderboard(character_id).await?;
            }

            Operation::RecordBattle {
                battle_chain,
                player1_id,
                player2_id,
                winner_id,
                stake,
                rounds_played,
            } => {
                let battle_id = *self.state.next_battle_id.get();
                self.state.next_battle_id.set(battle_id + 1);

                let now = self.runtime.system_time();
                let record = BattleRecord {
                    battle_id,
                    battle_chain,
                    player1_id,
                    player2_id,
                    winner_id,
                    stake,
                    rounds_played,
                    timestamp: now,
                };

                self.state.battles.insert(&battle_id, record)?;
                self.state.battle_chain_to_id.insert(&battle_chain, battle_id)?;

                let total = *self.state.total_battles.get();
                self.state.total_battles.set(total + 1);

                let volume = *self.state.total_volume.get();
                self.state.total_volume.set(volume.saturating_add(stake));
            }

            Operation::UpdateCharacterLevel { character_id, new_level } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                stats.level = new_level;

                self.state.characters.insert(&character_id, stats)?;
            }

            Operation::MarkCharacterDefeated { character_id } => {
                let mut stats = self.state.characters.get(&character_id).await?
                    .ok_or_else(|| RegistryError::CharacterNotFound(character_id.clone()))?;

                stats.is_alive = false;
                stats.lives_remaining = 0;

                self.state.characters.insert(&character_id, stats)?;
            }

            Operation::SubscribeToBattleEvents { battle_chain_id, battle_app_id } => {
                // Subscribe to battle events from the specified battle chain
                self.runtime.subscribe_to_events(
                    battle_chain_id,
                    battle_app_id,
                    "battle_events".into(),
                );

                // SECURITY: Track this battle chain for message authentication
                self.state.known_battle_chains.insert(&battle_chain_id, true)?;

                log::info!(
                    "Registry subscribed to battle events from chain {:?}, app {:?}",
                    battle_chain_id,
                    battle_app_id
                );
            }

            Operation::Pause => {
                // SECURITY: Only admin can pause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?;
                if &caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.paused.set(true);
                log::warn!("Registry paused by admin: {:?}", admin);
            }

            Operation::Unpause => {
                // SECURITY: Only admin can unpause
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?;
                if &caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.paused.set(false);
                log::info!("Registry unpaused by admin: {:?}", admin);
            }

            Operation::TransferAdmin { new_admin } => {
                // SECURITY: Only current admin can transfer admin rights
                let caller = self.runtime.authenticated_signer()
                    .ok_or(RegistryError::NotAuthorized)?;
                let admin = self.state.admin.get().as_ref()
                    .ok_or(RegistryError::NotAuthorized)?.clone();
                if caller != admin {
                    return Err(RegistryError::NotAuthorized);
                }
                self.state.admin.set(Some(new_admin));
                log::info!("Registry admin transferred from {:?} to {:?}", admin, new_admin);
            }
        }

        Ok(())
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleCompleted {
                battle_chain,
                player1_chain,
                player2_chain,
                winner_chain,
                stake,
                rounds_played,
                player1_stats,
                player2_stats,
            } => {
                // SECURITY: Validate message sender is a known battle chain
                let sender_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        log::error!("BattleCompleted message has no origin chain");
                        return;
                    }
                };

                // Check if this is a known battle chain
                match self.state.known_battle_chains.get(&sender_chain).await {
                    Ok(Some(true)) => {
                        // Valid battle chain, continue processing
                    }
                    _ => {
                        log::error!(
                            "SECURITY: Unauthorized BattleCompleted from unknown chain: {:?}",
                            sender_chain
                        );
                        return; // Reject message from unknown battle chain
                    }
                }

                // Get character IDs from owner chains
                let player1_id = self.state.owner_to_character.get(&player1_chain).await
                    .ok().flatten();
                let player2_id = self.state.owner_to_character.get(&player2_chain).await
                    .ok().flatten();

                if let (Some(p1_id), Some(p2_id)) = (player1_id, player2_id) {
                    let winner_id = if winner_chain == player1_chain {
                        p1_id.clone()
                    } else {
                        p2_id.clone()
                    };

                    // Get opponent ELO ratings for ELO calculation
                    let p1_stats = self.state.characters.get(&p1_id).await.ok().flatten();
                    let p2_stats = self.state.characters.get(&p2_id).await.ok().flatten();

                    if let (Some(p1_elo_rating), Some(p2_elo_rating)) =
                        (p1_stats.map(|s| s.elo_rating), p2_stats.map(|s| s.elo_rating)) {

                        // Update player 1 stats
                        let p1_won = winner_chain == player1_chain;
                        let p1_earnings = if p1_won { stake } else { Amount::ZERO };
                        let _ = self.execute_operation(Operation::UpdateCharacterStats {
                            character_id: p1_id.clone(),
                            won: p1_won,
                            damage_dealt: player1_stats.damage_dealt,
                            damage_taken: player1_stats.damage_taken,
                            crits: player1_stats.crits,
                            dodges: player1_stats.dodges,
                            highest_crit: player1_stats.highest_crit,
                            earnings: p1_earnings,
                            stake,
                            opponent_elo: p2_elo_rating,
                        }).await;

                        // Update player 2 stats
                        let p2_won = winner_chain == player2_chain;
                        let p2_earnings = if p2_won { stake } else { Amount::ZERO };
                        let _ = self.execute_operation(Operation::UpdateCharacterStats {
                            character_id: p2_id.clone(),
                            won: p2_won,
                            damage_dealt: player2_stats.damage_dealt,
                            damage_taken: player2_stats.damage_taken,
                            crits: player2_stats.crits,
                            dodges: player2_stats.dodges,
                            highest_crit: player2_stats.highest_crit,
                            earnings: p2_earnings,
                            stake,
                            opponent_elo: p1_elo_rating,
                        }).await;
                    }

                    // Record battle
                    let _ = self.execute_operation(Operation::RecordBattle {
                        battle_chain,
                        player1_id: p1_id,
                        player2_id: p2_id,
                        winner_id,
                        stake,
                        rounds_played,
                    }).await;
                }
            }

            Message::CharacterRegistered { .. } => {
                // Informational only
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
