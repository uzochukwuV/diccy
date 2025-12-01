#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
mod random;
mod battle_contract;
mod lobby_contract;
mod player_contract;

use linera_sdk::{
    linera_base_types::{WithContractAbi, Amount},
    views::{RootView, View},
    Contract, ContractRuntime,
};

use majorules::{Operation, Message, InitializationArgument, ChainVariant};

use self::state::{LobbyState, PlayerState, BattleState};
use self::lobby_contract::LobbyContract;
use self::player_contract::PlayerContract;

/// Multi-variant Contract - routes to appropriate chain implementation
pub struct MajorulesContract {
    pub variant: ChainVariant,
    pub lobby_state: Option<LobbyState>,
    pub player_state: Option<PlayerState>,
    pub battle_state: Option<BattleState>,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(MajorulesContract);

impl WithContractAbi for MajorulesContract {
    type Abi = majorules::MajorulesAbi;
}

impl MajorulesContract {
    /// Detect chain variant from stored state
    async fn detect_chain_variant(runtime: &ContractRuntime<Self>) -> ChainVariant {
        // Try to load each state type and check variant field
        if let Ok(lobby_state) = LobbyState::load(runtime.root_view_storage_context()).await {
            let variant_str = lobby_state.variant.get();
            if !variant_str.is_empty() {
                match variant_str.as_str() {
                    "Lobby" => return ChainVariant::Lobby,
                    "Player" => return ChainVariant::Player,
                    "Battle" => return ChainVariant::Battle,
                    _ => {}
                }
            }
        }
        
        // Default to Lobby for uninitialized chains
        ChainVariant::Lobby
    }
}

impl Contract for MajorulesContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = InitializationArgument;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        // Try to detect chain variant from stored state
        let variant = Self::detect_chain_variant(&runtime).await;

        // Load appropriate state, defaulting to empty state for new chains
        match variant {
            ChainVariant::Lobby => {
                let lobby_state = LobbyState::load(runtime.root_view_storage_context()).await.expect("Failed to load lobby state");
                Self { variant, lobby_state: Some(lobby_state), player_state: None, battle_state: None, runtime }
            }
            ChainVariant::Player => {
                let player_state = PlayerState::load(runtime.root_view_storage_context()).await.expect("Failed to load player state");
                Self { variant, lobby_state: None, player_state: Some(player_state), battle_state: None, runtime }
            }
            ChainVariant::Battle => {
                let battle_state = BattleState::load(runtime.root_view_storage_context()).await.expect("Failed to load battle state");
                Self { variant, lobby_state: None, player_state: None, battle_state: Some(battle_state), runtime }
            }
            ChainVariant::Prediction => {
                // Prediction markets are handled by lobby, redirect to lobby
                let lobby_state = LobbyState::load(runtime.root_view_storage_context()).await.expect("Failed to load lobby state");
                Self { variant: ChainVariant::Lobby, lobby_state: Some(lobby_state), player_state: None, battle_state: None, runtime }
            }
        }
    }

    async fn instantiate(&mut self, argument: Self::InstantiationArgument) {
        self.runtime.application_parameters();
        
        self.variant = argument.variant.clone();
        
        match argument.variant {
            ChainVariant::Lobby => {
                if let Some(ref mut state) = self.lobby_state {
                    state.variant.set("Lobby".to_string());
                    state.value.set(0);
                    state.treasury_owner.set(argument.treasury_owner);
                    state.platform_fee_bps.set(argument.platform_fee_bps.unwrap_or(500));
                    state.battle_count.set(0);
                    state.total_platform_revenue.set(Amount::ZERO);
                    state.battle_token_balance.set(Amount::ZERO);
                    state.market_count.set(0);
                    state.total_betting_volume.set(Amount::ZERO);
                    state.betting_leaderboard.set(Vec::new());
                }
            }
            ChainVariant::Player => {
                if let Some(ref mut state) = self.player_state {
                    state.variant.set("Player".to_string());
                    state.value.set(0);
                    state.character_count.set(0);
                    state.battle_token_balance.set(Amount::ZERO);
                    state.in_battle.set(false);
                    state.current_battle_chain.set(None);
                    state.last_active.set(self.runtime.system_time());
                    state.player_stats.set(crate::state::PlayerGlobalStats::default());
                }
            }
            ChainVariant::Battle => {
                if let Some(ref mut state) = self.battle_state {
                    state.variant.set("Battle".to_string());
                    state.value.set(0);
                    state.status.set(crate::state::BattleStatus::WaitingForPlayers);
                    state.current_round.set(0);
                    state.max_rounds.set(10);
                    state.winner.set(None);
                    state.round_results.set(Vec::new());
                    state.random_counter.set(0);
                    state.lobby_chain_id.set(None);
                    state.platform_fee_bps.set(300);
                    state.treasury_owner.set(None);
                    state.started_at.set(None);
                    state.completed_at.set(None);
                }
            }
            ChainVariant::Prediction => {
                // Prediction markets handled by lobby, treat as lobby
            }
        }
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match self.variant {
            ChainVariant::Lobby => {
                if let Some(ref mut state) = self.lobby_state {
                    LobbyContract::execute_operation(state, &mut self.runtime, operation).await;
                }
            }
            ChainVariant::Player => {
                if let Some(ref mut state) = self.player_state {
                    PlayerContract::execute_operation(state, &mut self.runtime, operation).await;
                }
            }
            ChainVariant::Battle => {
                if let Some(ref mut state) = self.battle_state {
                    battle_contract::handle_battle_operation(operation, state, &mut self.runtime).await;
                }
            }
            ChainVariant::Prediction => {
                // Prediction operations handled by lobby
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        // Handle InstantiateChain message first
        if let Message::InstantiateChain { variant, treasury_owner, platform_fee_bps } = message {
            let init_arg = InitializationArgument {
                variant,
                treasury_owner,
                platform_fee_bps,
            };
            self.instantiate(init_arg).await;
            return;
        }
        
        match self.variant {
            ChainVariant::Lobby => {
                if let Some(ref mut state) = self.lobby_state {
                    LobbyContract::execute_message(state, &mut self.runtime, message).await;
                }
            }
            ChainVariant::Player => {
                if let Some(ref mut state) = self.player_state {
                    PlayerContract::execute_message(state, &mut self.runtime, message).await;
                }
            }
            ChainVariant::Battle => {
                if let Some(ref mut state) = self.battle_state {
                    battle_contract::handle_battle_message(message, state, &mut self.runtime).await;
                }
            }
            ChainVariant::Prediction => {
                // Prediction messages handled by lobby
            }
        }
    }

    async fn store(self) {
        if let Some(mut state) = self.lobby_state {
            state.save().await.expect("Failed to save lobby state");
        }
        if let Some(mut state) = self.player_state {
            state.save().await.expect("Failed to save player state");
        }
        if let Some(mut state) = self.battle_state {
            state.save().await.expect("Failed to save battle state");
        }

    }
}