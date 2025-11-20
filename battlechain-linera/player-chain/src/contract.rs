#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use battlechain_shared_types::{CharacterClass, CharacterNFT};
use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use battle_token::BattleTokenAbi;
use player_chain::{Message, Operation, PlayerChainAbi, PlayerChainError};
use self::state::PlayerChainState;

/// Player Chain Contract
pub struct PlayerChainContract {
    pub state: PlayerChainState,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(PlayerChainContract);

impl WithContractAbi for PlayerChainContract {
    type Abi = PlayerChainAbi;
}

impl Contract for PlayerChainContract {
    type Message = Message;
    type Parameters = Option<ApplicationId<BattleTokenAbi>>;
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = PlayerChainState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let battle_token_app = self.runtime.application_parameters();
        let chain_ownership = self.runtime.chain_ownership();
        let _owner = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("Chain must have owner")
            .clone();
        let now = self.runtime.system_time();

        // Initialize state (owner is tracked via chain ownership)
        self.state.characters.set(Vec::new());
        self.state.battle_token_app.set(battle_token_app);
        self.state.battle_balance.set(Amount::ZERO);
        self.state.locked_battle.set(Amount::ZERO);
        self.state.total_battles.set(0);
        self.state.wins.set(0);
        self.state.losses.set(0);
        self.state.active_battles.set(Vec::new());
        self.state.created_at.set(now);
        self.state.last_active.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let now = self.runtime.system_time();
        self.state.last_active.set(now);

        // SECURITY: Check if contract is paused (skip for admin operations)
        match operation {
            Operation::Pause | Operation::Unpause | Operation::TransferAdmin { .. } => {
                // Admin operations allowed even when paused
            }
            _ => {
                if *self.state.paused.get() {
                    panic!("Contract is paused");
                }
            }
        }

        match operation {
            Operation::Initialize { battle_token_app } => {
                self.state.battle_token_app.set(Some(battle_token_app));

                // Set admin to the chain owner on initialization
                if let Some(owner) = self.runtime.authenticated_signer() {
                    self.state.admin.set(Some(owner));
                }
                self.state.paused.set(false);
            }

            Operation::CreateCharacter { nft_id, class } => {
                // SECURITY: Rate limiting check
                const CHARACTER_COOLDOWN_MICROS: u64 = 60_000_000; // 1 minute
                let last_creation = *self.state.last_character_creation.get();
                if last_creation.micros() > 0 {
                    let elapsed = now.micros().saturating_sub(last_creation.micros());
                    if elapsed < CHARACTER_COOLDOWN_MICROS {
                        panic!("Rate limit: Must wait {} seconds between character creations",
                            (CHARACTER_COOLDOWN_MICROS - elapsed) / 1_000_000);
                    }
                }
                self.state.last_character_creation.set(now);

                // SECURITY: Validate NFT ID non-empty
                if nft_id.is_empty() {
                    panic!("NFT ID cannot be empty");
                }
                let mut chars = self.state.characters.get().clone();
                let new_char = CharacterNFT::new(nft_id, class, now);

                // Validate character state after creation
                if let Err(e) = new_char.validate() {
                    log::error!("Character validation failed after creation: {}", e);
                    panic!("Invalid character state: {}", e);
                }

                chars.push(new_char);
                self.state.characters.set(chars);
            }

            Operation::JoinBattle { battle_chain, character_nft, stake } => {
                // SECURITY: Rate limiting check
                const BATTLE_JOIN_COOLDOWN_MICROS: u64 = 10_000_000; // 10 seconds
                let last_join = *self.state.last_battle_join.get();
                if last_join.micros() > 0 {
                    let elapsed = now.micros().saturating_sub(last_join.micros());
                    if elapsed < BATTLE_JOIN_COOLDOWN_MICROS {
                        panic!("Rate limit: Must wait {} seconds between battle joins",
                            (BATTLE_JOIN_COOLDOWN_MICROS - elapsed) / 1_000_000);
                    }
                }
                self.state.last_battle_join.set(now);

                // SECURITY: Track this battle chain for message authentication
                let _ = self.state.known_battle_chains.insert(&battle_chain, true);

                // Store which character is in this battle
                self.state.battle_characters.insert(&battle_chain, character_nft.clone())
                    .expect("Failed to store character for battle");

                // Mark character as in battle
                let mut chars = self.state.characters.get().clone();
                if let Some(character) = chars.iter_mut().find(|c| c.nft_id == character_nft) {
                    character.in_battle = true;
                }
                self.state.characters.set(chars);

                // Lock stake
                let _ = self.state.lock_battle(battle_chain, stake);

                // Add to active battles
                let mut active = self.state.active_battles.get().clone();
                active.push(battle_chain);
                self.state.active_battles.set(active);
            }

            Operation::UpdateAfterBattle { battle_chain, won, reward } => {
                // Unlock stake
                let _ = self.state.unlock_battle(&battle_chain).await;

                // Remove from active battles
                let mut active = self.state.active_battles.get().clone();
                active.retain(|c| c != &battle_chain);
                self.state.active_battles.set(active);

                // Record result
                self.state.record_battle_result(won);

                // Add reward if won
                if won && reward > Amount::ZERO {
                    let new_balance = self.state.battle_balance.get()
                        .try_add(reward)
                        .unwrap_or(*self.state.battle_balance.get());
                    self.state.battle_balance.set(new_balance);
                }
            }

            Operation::Pause => {
                // SECURITY: Only admin can pause
                let caller = self.runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                let admin = self.state.admin.get().as_ref()
                    .expect("Admin not set");
                if &caller != admin {
                    panic!("Only admin can pause the contract");
                }
                self.state.paused.set(true);
                log::warn!("Player chain paused by admin: {:?}", admin);
            }

            Operation::Unpause => {
                // SECURITY: Only admin can unpause
                let caller = self.runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                let admin = self.state.admin.get().as_ref()
                    .expect("Admin not set");
                if &caller != admin {
                    panic!("Only admin can unpause the contract");
                }
                self.state.paused.set(false);
                log::info!("Player chain unpaused by admin: {:?}", admin);
            }

            Operation::TransferAdmin { new_admin } => {
                // SECURITY: Only current admin can transfer admin rights
                let caller = self.runtime.authenticated_signer()
                    .expect("Operation must be authenticated");
                let admin = self.state.admin.get().as_ref()
                    .expect("Admin not set").clone();
                if caller != admin {
                    panic!("Only admin can transfer admin rights");
                }
                self.state.admin.set(Some(new_admin));
                log::info!("Admin transferred from {:?} to {:?}", admin, new_admin);
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::BattleInvite { battle_chain: _, stake_required: _ } => {
                // Handle battle invite - could auto-join if auto_play enabled
                // TODO: Implement auto-join logic based on player preferences
            }

            Message::BattleResult {
                winner,
                loser: _,
                winner_payout,
                rounds_played,
            } => {
                // SECURITY: Validate message sender is a known battle chain
                let sender_chain = match self.runtime.message_origin_chain_id() {
                    Some(chain) => chain,
                    None => {
                        log::error!("BattleResult message has no origin chain");
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
                            "SECURITY: Unauthorized BattleResult from unknown chain: {:?}",
                            sender_chain
                        );
                        return; // Reject message from unknown battle chain
                    }
                }

                let battle_chain = sender_chain;

                // Determine if this player won or lost
                let player_owner = self.runtime.authenticated_signer();
                let won = player_owner.map(|o| o == winner).unwrap_or(false);

                // === CHARACTER PROGRESSION ===
                // Get character ID from battle
                let character_id = match self.state.battle_characters.get(&battle_chain).await {
                    Ok(Some(id)) => id,
                    _ => {
                        // No character tracked - skip progression but continue with other updates
                        String::new()
                    }
                };

                // Update character progression if we have a character ID
                if !character_id.is_empty() {
                    let mut chars = self.state.characters.get().clone();
                    let mut should_remove_character = false;

                    // Update character in a separate scope to avoid borrow conflicts
                    {
                        let character_opt = chars.iter_mut().find(|c| c.nft_id == character_id);

                        if let Some(character) = character_opt {
                            // Mark character as no longer in battle
                            character.in_battle = false;

                            if won {
                                // Award XP based on rounds played
                                let xp_reward = 100 + (rounds_played as u64 * 10);
                                character.xp += xp_reward;

                                // Check for level up
                                let xp_needed = 100 * (character.level as u64);
                                if character.xp >= xp_needed {
                                    character.level += 1;
                                    character.xp = character.xp.saturating_sub(xp_needed);

                                    // Update stats for new level
                                    character.hp_max += 10;
                                    character.current_hp = character.hp_max; // Full heal on level up
                                    character.min_damage += 1;
                                    character.max_damage += 2;

                                    log::info!(
                                        "Character {} leveled up to level {}!",
                                        character.nft_id,
                                        character.level
                                    );
                                }
                            } else {
                                // Lose a life (permadeath mechanic)
                                character.lives = character.lives.saturating_sub(1);

                                if character.lives == 0 {
                                    // Character is permanently dead - will be removed after validation
                                    should_remove_character = true;
                                    log::warn!(
                                        "Character {} has died (permadeath)!",
                                        character.nft_id
                                    );
                                } else {
                                    log::info!(
                                        "Character {} has {} lives remaining",
                                        character.nft_id,
                                        character.lives
                                    );
                                }
                            }

                            // Validate character state after modifications
                            if !should_remove_character {
                                if let Err(e) = character.validate() {
                                    log::error!("Character validation failed after battle update: {}", e);
                                    log::error!("Character: {:#?}", character);
                                    panic!("Invalid character state after battle: {}", e);
                                }
                            }
                        }
                    } // character reference dropped here

                    // Now safe to call retain() since character reference is dropped
                    if should_remove_character {
                        chars.retain(|c| c.nft_id != character_id);
                    }

                    self.state.characters.set(chars);

                    // Remove character from battle tracking
                    let _ = self.state.battle_characters.remove(&battle_chain);
                }
                // === END CHARACTER PROGRESSION ===

                // Unlock stake
                let _ = self.state.unlock_battle(&battle_chain).await;

                // Record result
                self.state.record_battle_result(won);

                // Remove from active battles
                let mut active = self.state.active_battles.get().clone();
                active.retain(|c| c != &battle_chain);
                self.state.active_battles.set(active);

                // Add payout if won
                if won && winner_payout > Amount::ZERO {
                    let new_balance = self.state.battle_balance.get()
                        .try_add(winner_payout)
                        .unwrap_or(*self.state.battle_balance.get());
                    self.state.battle_balance.set(new_balance);
                }
            }

            Message::LockStakeRequest {
                matchmaking_chain: _,
                battle_chain,
                stake_amount,
            } => {
                // Lock stake for upcoming battle
                if let Err(_e) = self.state.lock_battle(battle_chain, stake_amount) {
                    // Failed to lock - could send rejection message back
                    return;
                }

                // Add to active battles
                let mut active = self.state.active_battles.get().clone();
                if !active.contains(&battle_chain) {
                    active.push(battle_chain);
                    self.state.active_battles.set(active);
                }

                // TODO: Send confirmation message back to matchmaking chain
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
