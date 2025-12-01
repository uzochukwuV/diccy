use linera_sdk::{
    linera_base_types::{Amount, AccountOwner},
    ContractRuntime,
};

use majorules::{Operation, Message, CharacterSnapshot, CharacterClass};
use crate::state::PlayerState;

pub struct PlayerContract;

impl PlayerContract {
    pub async fn execute_operation(
        state: &mut PlayerState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        operation: Operation,
    ) {
        let caller = runtime.authenticated_signer()
            .expect("Operation must be authenticated");

        match operation {
            Operation::JoinQueue { character_id, stake } => {
                // Get character data and send to lobby
                if let Ok(Some(character)) = state.characters.get(&character_id).await {
                    let lobby_chain_id = state.lobby_chain_id.get().unwrap();
                    let player_chain_id = runtime.chain_id();
                    
                    runtime.prepare_message(Message::RequestJoinQueue {
                        player: caller,
                        player_chain: player_chain_id,
                        character_snapshot: CharacterSnapshot {
                            nft_id: character.nft_id,
                            class: match character.class {
                                crate::state::CharacterClass::Warrior => CharacterClass::Warrior,
                                crate::state::CharacterClass::Mage => CharacterClass::Mage,
                                _ => CharacterClass::Warrior,
                            },
                            level: character.level,
                            hp_max: character.hp_max,
                            min_damage: character.min_damage,
                            max_damage: character.max_damage,
                            crit_chance: character.crit_chance,
                            crit_multiplier: character.crit_multiplier,
                            dodge_chance: character.dodge_chance,
                            defense: character.defense,
                            attack_bps: character.attack_bps,
                            defense_bps: character.defense_bps,
                            crit_bps: character.crit_bps,
                        },
                        stake,
                    }).with_authentication().send_to(lobby_chain_id);
                }
            }

            Operation::CreatePrivateBattle { character_id, stake } => {
                // Get character data and send to lobby
                if let Ok(Some(character)) = state.characters.get(&character_id).await {
                    let lobby_chain_id = state.lobby_chain_id.get().unwrap();
                    let player_chain_id = runtime.chain_id();
                    
                    runtime.prepare_message(Message::RequestCreatePrivateBattle {
                        player: caller,
                        player_chain: player_chain_id,
                        character_snapshot: CharacterSnapshot {
                            nft_id: character.nft_id,
                            class: match character.class {
                                crate::state::CharacterClass::Warrior => CharacterClass::Warrior,
                                crate::state::CharacterClass::Mage => CharacterClass::Mage,
                                _ => CharacterClass::Warrior,
                            },
                            level: character.level,
                            hp_max: character.hp_max,
                            min_damage: character.min_damage,
                            max_damage: character.max_damage,
                            crit_chance: character.crit_chance,
                            crit_multiplier: character.crit_multiplier,
                            dodge_chance: character.dodge_chance,
                            defense: character.defense,
                            attack_bps: character.attack_bps,
                            defense_bps: character.defense_bps,
                            crit_bps: character.crit_bps,
                        },
                        stake,
                    }).with_authentication().send_to(lobby_chain_id);
                }
            }

            Operation::JoinPrivateBattle { battle_id, character_id, stake } => {
                // Get character data and send to lobby
                if let Ok(Some(character)) = state.characters.get(&character_id).await {
                    let lobby_chain_id = state.lobby_chain_id.get().unwrap();
                    let player_chain_id = runtime.chain_id();
                    
                    runtime.prepare_message(Message::RequestJoinPrivateBattle {
                        player: caller,
                        player_chain: player_chain_id,
                        battle_id,
                        character_snapshot: CharacterSnapshot {
                            nft_id: character.nft_id,
                            class: match character.class {
                                crate::state::CharacterClass::Warrior => CharacterClass::Warrior,
                                crate::state::CharacterClass::Mage => CharacterClass::Mage,
                                _ => CharacterClass::Warrior,
                            },
                            level: character.level,
                            hp_max: character.hp_max,
                            min_damage: character.min_damage,
                            max_damage: character.max_damage,
                            crit_chance: character.crit_chance,
                            crit_multiplier: character.crit_multiplier,
                            dodge_chance: character.dodge_chance,
                            defense: character.defense,
                            attack_bps: character.attack_bps,
                            defense_bps: character.defense_bps,
                            crit_bps: character.crit_bps,
                        },
                        stake,
                    }).with_authentication().send_to(lobby_chain_id);
                }
            }

            Operation::MintCharacter { character_id, class } => {
                let character_class = CharacterClass::from_str(&class).unwrap_or(CharacterClass::Warrior);
                let (hp_max, min_damage, max_damage, crit_chance) = character_class.base_stats();
                
                let character = crate::state::CharacterData {
                    nft_id: character_id.clone(),
                    owner: caller,
                    class: match character_class {
                        CharacterClass::Warrior => crate::state::CharacterClass::Warrior,
                        CharacterClass::Mage => crate::state::CharacterClass::Mage,
                        _ => crate::state::CharacterClass::Warrior,
                    },
                    level: 1,
                    xp: 0,
                    hp_max,
                    min_damage,
                    max_damage,
                    crit_chance,
                    crit_multiplier: 1500,
                    dodge_chance: 500,
                    defense: 5,
                    attack_bps: 0,
                    defense_bps: 0,
                    crit_bps: 0,
                    created_at: runtime.system_time(),
                    is_active: false,
                };

                state.characters.insert(&character_id, character)
                    .expect("Failed to mint character");
            }

            Operation::SetActiveCharacter { character_id } => {
                // Verify character exists and belongs to caller
                if let Ok(Some(character)) = state.characters.get(&character_id).await {
                    if character.owner == caller {
                        state.active_character.set(Some(character_id));
                    }
                }
            }

            _ => {
                // Ignore operations not relevant to player chain
            }
        }
    }

    pub async fn execute_message(
        state: &mut PlayerState,
        runtime: &mut ContractRuntime<crate::MajorulesContract>,
        message: Message,
    ) {
        match message {
            Message::InitializePlayerChain { lobby_chain_id, owner } => {
                // Initialize player chain with lobby reference
                state.lobby_chain_id.set(Some(lobby_chain_id));
                state.owner.set(Some(owner));
            }

            Message::UpdatePlayerStats { player, won, xp_gained } => {
                // Update player stats from battle results
                if Some(player) == *state.owner.get() {
                    let mut stats = state.player_stats.get().clone();
                    stats.total_battles += 1;
                    if won {
                        stats.wins += 1;
                        stats.current_streak += 1;
                        if stats.current_streak > stats.best_streak {
                            stats.best_streak = stats.current_streak;
                        }
                    } else {
                        stats.losses += 1;
                        stats.current_streak = 0;
                    }
                    stats.win_rate = stats.wins as f64 / stats.total_battles as f64;
                    state.player_stats.set(stats);

                    // Add XP to active character
                    if let Some(character_id) = state.active_character.get() {
                        if let Ok(Some(mut character)) = state.characters.get(character_id).await {
                            character.xp += xp_gained as u64;
                            state.characters.insert(character_id, character)
                                .expect("Failed to update character XP");
                        }
                    }
                }
            }

            Message::RequestPlayerStats { player } => {
                // Send player stats to lobby
                if Some(player) == *state.owner.get() {
                    let lobby_chain_id = state.lobby_chain_id.get().unwrap();
                    let stats = state.player_stats.get().clone();
                    
                    runtime.prepare_message(Message::PlayerStatsResponse {
                        player,
                        stats: majorules::PlayerGlobalStats {
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
                            highest_crit: stats.highest_crit,
                            current_streak: stats.current_streak,
                            best_streak: stats.best_streak,
                        },
                    }).with_authentication().send_to(lobby_chain_id);
                }
            }

            _ => {
                // Ignore other message types
            }
        }
    }
}