#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::{Amount, ApplicationPermissions, ChainOwnership},
    views::{RootView, View},
    Contract, ContractRuntime,
};
use matchmaking_chain::{
    BattleMetadata, MatchmakingAbi, Message, Operation, QueueEntry,
};
use prediction_chain::Operation as PredictionOperation;
use self::state::{MatchmakingState, PendingBattle};
use std::collections::BTreeMap;

/// Matchmaking Contract
pub struct MatchmakingContract {
    pub state: MatchmakingState,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(MatchmakingContract);

impl WithContractAbi for MatchmakingContract {
    type Abi = MatchmakingAbi;
}

impl MatchmakingContract {
    /// Create a new battle chain with multi-owner ownership
    async fn create_battle_chain(&mut self, pending: PendingBattle) {
        let battle_app_id = self.state.battle_app_id.get()
            .clone()
            .expect("Battle app ID not configured");

        // Create multi-owner chain ownership with both players
        // Each player gets equal weight (100)
        let mut owners = BTreeMap::new();
        owners.insert(pending.player1.player_owner, 100);
        owners.insert(pending.player2.player_owner, 100);

        let chain_ownership = ChainOwnership {
            super_owners: Default::default(), // No super owners for battle chains
            owners,
            multi_leader_rounds: 10, // Allow 10 rounds of multi-leader consensus
            open_multi_leader_rounds: false, // Only the two players can propose
            timeout_config: Default::default(), // Use default timeouts
        };

        // Configure application permissions
        // Only allow the battle application to execute operations
        let application_permissions = ApplicationPermissions {
            execute_operations: Some(vec![battle_app_id]),
            mandatory_applications: vec![],
            close_chain: vec![battle_app_id], // Battle app can close chain when battle ends
            change_application_permissions: vec![],
            call_service_as_oracle: None, // No oracle calls needed
            make_http_requests: None, // No HTTP requests needed
        };

        // Calculate total stake to fund the new chain
        let total_stake = pending.player1.stake.saturating_add(pending.player2.stake);

        // Create the battle chain!
        let battle_chain_id = self.runtime.open_chain(
            chain_ownership,
            application_permissions,
            total_stake, // Initial balance for the battle chain
        );

        // Battle chain initialization happens via deployment parameters
        // The battle application will be auto-deployed to the new chain with the
        // application_id from required_application_ids set during chain creation
        log::info!(
            "Battle chain created at {:?} - will be initialized via deployment parameters",
            battle_chain_id
        );

        // Create prediction market automatically
        if let Some(prediction_app) = self.state.prediction_app_id.get().as_ref() {
            let create_market_op = PredictionOperation::CreateMarket {
                battle_chain: battle_chain_id,
                player1_chain: pending.player1.player_chain,
                player2_chain: pending.player2.player_chain,
            };

            // Call prediction chain synchronously to create market
            let result: Result<(), prediction_chain::PredictionError> = self.runtime.call_application(
                true,  // authenticated call
                *prediction_app,
                &create_market_op,
            );

            match result {
                Ok(_) => log::info!(
                    "Created prediction market for battle {:?}",
                    battle_chain_id
                ),
                Err(e) => log::warn!(
                    "Failed to create prediction market for battle {:?}: {:?}",
                    battle_chain_id, e
                ),
            }
        }

        // Store battle metadata
        let metadata = crate::BattleMetadata {
            player1: pending.player1.player_chain,
            player2: pending.player2.player_chain,
            stake: total_stake,
            started_at: self.runtime.system_time(),
        };

        self.state.active_battles.insert(&battle_chain_id, metadata)
            .expect("Failed to store battle metadata");

        // Notify both players of battle creation
        let battle_msg = Message::BattleCreated {
            battle_chain: battle_chain_id,
            opponent: pending.player2.player_chain,
        };
        self.runtime
            .prepare_message(battle_msg.clone())
            .with_authentication()
            .send_to(pending.player1.player_chain);

        let battle_msg = Message::BattleCreated {
            battle_chain: battle_chain_id,
            opponent: pending.player1.player_chain,
        };
        self.runtime
            .prepare_message(battle_msg)
            .with_authentication()
            .send_to(pending.player2.player_chain);
    }
}

impl Contract for MatchmakingContract {
    type Message = Message;
    type Parameters = Amount; // Minimum stake
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = MatchmakingState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let min_stake = self.runtime.application_parameters();
        let now = self.runtime.system_time();

        self.state.next_offer_id.set(0);
        self.state.completed_battles.set(Vec::new());
        self.state.total_battles.set(0);
        self.state.min_stake.set(min_stake);
        self.state.battle_app_id.set(None);
        self.state.battle_token_app.set(None);
        self.state.prediction_app_id.set(None);
        self.state.platform_fee_bps.set(300); // 3% default
        self.state.treasury_owner.set(None);
        self.state.created_at.set(now);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::JoinQueue { player_chain, player_owner, character, stake } => {
                let now = self.runtime.system_time();

                let entry = QueueEntry {
                    player_chain,
                    player_owner,
                    character,
                    stake,
                    joined_at: now,
                };

                self.state.add_waiting_player(entry)
                    .expect("Failed to add player to queue");

                // Automatic matchmaking: Try to find an opponent
                if let Some((opponent_chain, opponent_entry)) = self.state.find_match(&player_chain).await {
                    log::info!(
                        "Match found! {:?} vs {:?}",
                        player_chain,
                        opponent_chain
                    );

                    // Get the newly added player entry
                    let player_entry = self.state.waiting_players.get(&player_chain).await
                        .expect("View error")
                        .expect("Player entry should exist");

                    // Create pending battle
                    let offer_id = *self.state.next_offer_id.get();
                    self.state.next_offer_id.set(offer_id + 1);

                    let pending = PendingBattle {
                        offer_id,
                        player1: player_entry,
                        player2: opponent_entry.clone(),
                        created_at: now,
                        player1_confirmed: false,
                        player2_confirmed: false,
                    };

                    self.state.pending_battles.insert(&offer_id, pending)
                        .expect("Failed to insert pending battle");

                    // Remove both players from waiting queue
                    self.state.waiting_players.remove(&player_chain)
                        .expect("Failed to remove player");
                    self.state.waiting_players.remove(&opponent_chain)
                        .expect("Failed to remove opponent");

                    // Send battle offer notifications to both players
                    let offer_msg_p1 = Message::BattleOffer {
                        offer_id,
                        opponent_chain,
                        stake: opponent_entry.stake,
                    };
                    let offer_msg_p2 = Message::BattleOffer {
                        offer_id,
                        opponent_chain: player_chain,
                        stake,
                    };

                    self.runtime
                        .prepare_message(offer_msg_p1)
                        .with_authentication()
                        .send_to(player_chain);

                    self.runtime
                        .prepare_message(offer_msg_p2)
                        .with_authentication()
                        .send_to(opponent_chain);

                    log::info!("Battle offer {} created and sent to both players", offer_id);
                } else {
                    log::info!(
                        "Player {:?} added to queue, waiting for opponent...",
                        player_chain
                    );
                }
            }

            Operation::LeaveQueue { player_chain } => {
                let _ = self.state.remove_waiting_player(&player_chain).await;
            }

            Operation::CreateBattleOffer { player1_chain, player2_chain } => {
                let now = self.runtime.system_time();

                // Get both players from queue
                let player1 = self.state.waiting_players.get(&player1_chain).await
                    .expect("View error")
                    .expect("Player 1 not in queue");
                let player2 = self.state.waiting_players.get(&player2_chain).await
                    .expect("View error")
                    .expect("Player 2 not in queue");

                // Create pending battle
                let offer_id = *self.state.next_offer_id.get();
                self.state.next_offer_id.set(offer_id + 1);

                let pending = PendingBattle {
                    offer_id,
                    player1: player1.clone(),
                    player2: player2.clone(),
                    created_at: now,
                    player1_confirmed: false,
                    player2_confirmed: false,
                };

                self.state.pending_battles.insert(&offer_id, pending)
                    .expect("Failed to insert pending battle");

                // Remove from waiting queue
                self.state.waiting_players.remove(&player1_chain)
                    .expect("Failed to remove player 1");
                self.state.waiting_players.remove(&player2_chain)
                    .expect("Failed to remove player 2");

                // Send battle offer notifications to both players
                let offer_msg_p1 = Message::BattleOffer {
                    offer_id,
                    opponent_chain: player2_chain,
                    stake: player1.stake,
                };
                let offer_msg_p2 = Message::BattleOffer {
                    offer_id,
                    opponent_chain: player1_chain,
                    stake: player2.stake,
                };

                self.runtime
                    .prepare_message(offer_msg_p1)
                    .with_authentication()
                    .send_to(player1_chain);

                self.runtime
                    .prepare_message(offer_msg_p2)
                    .with_authentication()
                    .send_to(player2_chain);
            }

            Operation::ConfirmBattleOffer { offer_id, player_chain } => {
                let mut pending = self.state.pending_battles.get(&offer_id).await
                    .expect("View error")
                    .expect("Battle offer not found");

                // Mark confirmation
                if pending.player1.player_chain == player_chain {
                    pending.player1_confirmed = true;
                } else if pending.player2.player_chain == player_chain {
                    pending.player2_confirmed = true;
                } else {
                    panic!("Caller not part of this battle offer");
                }

                // Check if both confirmed
                if pending.player1_confirmed && pending.player2_confirmed {
                    // Both confirmed - create battle chain!
                    self.create_battle_chain(pending.clone()).await;

                    // Remove pending battle
                    self.state.pending_battles.remove(&offer_id)
                        .expect("Failed to remove pending battle");
                } else {
                    // Update pending battle with confirmation
                    self.state.pending_battles.insert(&offer_id, pending)
                        .expect("Failed to update pending battle");
                }
            }

            Operation::RecordBattleCompletion { battle_chain } => {
                // Remove from active battles
                self.state.active_battles.remove(&battle_chain)
                    .expect("Failed to remove active battle");

                // Add to completed
                let mut completed = self.state.completed_battles.get().clone();
                completed.push(battle_chain);
                self.state.completed_battles.set(completed);

                // Increment total battles
                let total = *self.state.total_battles.get();
                self.state.total_battles.set(total + 1);
            }

            Operation::UpdateReferences { battle_app_id, battle_token_app, treasury_owner } => {
                self.state.battle_app_id.set(Some(battle_app_id));
                self.state.battle_token_app.set(Some(battle_token_app));
                self.state.treasury_owner.set(Some(treasury_owner));
            }
        }
    }

    async fn execute_message(&mut self, _message: Message) {
        // Handle match found notifications
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
