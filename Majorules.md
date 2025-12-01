#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;
mod random;

use linera_sdk::{
    linera_base_types::{Account, AccountOwner, Amount, ApplicationPermissions, ChainId, ChainOwnership, Timestamp, WithContractAbi},
    views::{RootView, View},
    Contract, ContractRuntime,
};

use majorules::{Message, Operation};

use self::state::{LobbyState, GameState};
use self::random::random_value;

// Game constants
const MIN_PLAYERS_TO_START: usize = 3;
const MAX_PLAYERS_PER_GAME: usize = 50;
const MAX_ROUNDS: u64 = 3;
const MAX_REVOTES: u8 = 1;
const ANSWER_TIMEOUT_MICROS: u64 = 60_000_000; // 60 seconds
const QUESTION_TIMEOUT_MICROS: u64 = 30_000_000; // 30 seconds

// Status codes
const STATUS_ACTIVE: u8 = 0;
const STATUS_FINISHED: u8 = 1;

// =============================================================================
// CONTRACT WRAPPER - DETERMINES LOBBY OR GAME
// =============================================================================

pub enum MajorulesContract {
    Lobby {
        state: LobbyState,
        runtime: ContractRuntime<Self>,
    },
    Game {
        state: GameState,
        runtime: ContractRuntime<Self>,
    },
}

linera_sdk::contract!(MajorulesContract);

impl WithContractAbi for MajorulesContract {
    type Abi = majorules::MajorulesAbi;
}

impl Contract for MajorulesContract {
    type Message = Message;
    type Parameters = ();
    type InstantiationArgument = majorules::InitializationArgument;
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        // Determine if this is a lobby or game chain based on initialization
        // We'll check if lobby_owner is set in LobbyState
        if let Ok(lobby_state) = LobbyState::load(runtime.root_view_storage_context()).await {
            // Try to check if this is actually a lobby by checking lobby_owner
            let lobby_owner = *lobby_state.lobby_owner.get();
            if lobby_owner.is_some() {
                return Self::Lobby {
                    state: lobby_state,
                    runtime
                };
            }
        }

        // Otherwise load as game
        let state = GameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        Self::Game { state, runtime }
    }

    async fn instantiate(&mut self, argument: Self::InstantiationArgument) {
        if argument.is_lobby {
            // Initialize as lobby
            if let Self::Lobby { state, runtime } = self {
                runtime.application_parameters();
                state.value.set(0);
                state.entry_fee.set(argument.entry_fee);
                state.lobby_owner.set(Some(argument.lobby_owner));
                state.game_count.set(0);
            } else {
                panic!("Wrong instantiation: expected Lobby variant");
            }
        } else {
            // Initialize as game (via message, not direct instantiation)
            panic!("Games must be initialized via InitializeGame message");
        }
    }

    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match self {
            Self::Lobby { state, runtime } => {
                Self::execute_lobby_operation(state, runtime, operation).await
            }
            Self::Game { state, runtime } => {
                Self::execute_game_operation(state, runtime, operation).await
            }
        }
    }

    async fn execute_message(&mut self, message: Self::Message) {
        match self {
            Self::Lobby { state, runtime } => {
                Self::execute_lobby_message(state, runtime, message).await
            }
            Self::Game { state, runtime } => {
                Self::execute_game_message(state, runtime, message).await
            }
        }
    }

    async fn store(self) {
        match self {
            Self::Lobby { mut state, .. } => {
                state.save().await.expect("Failed to save lobby state");
            }
            Self::Game { mut state, .. } => {
                state.save().await.expect("Failed to save game state");
            }
        }
    }
}

// =============================================================================
// LOBBY OPERATIONS
// =============================================================================

impl MajorulesContract {
    async fn execute_lobby_operation(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<Self>,
        operation: Operation,
    ) {
        match operation {
            Operation::Increment { value } => {
                state.value.set(state.value.get() + value);
            }

            Operation::JoinLobby => {
                // Player calls this from THEIR chain to join the lobby
                let player = runtime
                    .authenticated_signer()
                    .expect("Must be authenticated");
                let player_chain = runtime.chain_id();

                // Get the lobby chain ID (we need to know which lobby to join)
                // For now, this operation sends a cross-chain message
                // In production, player would specify lobby_chain_id as parameter

                // Send cross-chain message to lobby
                // Note: This operation should actually be called with lobby_chain_id parameter
                // For MVP, we assume this IS the lobby chain and handle it directly

                // Check not already joined
                assert!(
                    !state.waiting_players.contains_key(&player).await.unwrap(),
                    "Already in lobby"
                );

                // Check max players
                let current_count = state.waiting_players.count().await.unwrap();
                assert!(
                    current_count < MAX_PLAYERS_PER_GAME,
                    "Lobby is full"
                );

                // Collect entry fee
                let entry_fee = *state.entry_fee.get();
                let lobby_account = Account {
                    chain_id: runtime.chain_id(),
                    owner: AccountOwner::CHAIN,
                };
                runtime.transfer(player, lobby_account, entry_fee);

                // Add to waiting list
                state.waiting_players
                    .insert(&player, player_chain)
                    .expect("Failed to add player");

                // Auto-start if enough players
                let player_count = state.waiting_players.count().await.unwrap();
                if player_count >= MIN_PLAYERS_TO_START {
                    Self::start_game(state, runtime).await;
                }
            }

            Operation::LeaveLobby => {
                let player = runtime
                    .authenticated_signer()
                    .expect("Must be authenticated");

                let player_chain = state.waiting_players
                    .get(&player)
                    .await
                    .expect("Failed to check")
                    .expect("Not in lobby");

                // Remove from lobby
                state.waiting_players
                    .remove(&player)
                    .expect("Failed to remove");

                // Refund entry fee
                let entry_fee = *state.entry_fee.get();
                let refund_account = Account {
                    chain_id: player_chain,
                    owner: player,
                };
                runtime.transfer(AccountOwner::CHAIN, refund_account, entry_fee);
            }

            _ => panic!("Invalid operation for lobby chain"),
        }
    }

    /// Create a new game chain and initialize it
    async fn start_game(state: &mut LobbyState, runtime: &mut ContractRuntime<Self>) {
        // Collect players
        let player_indices = state.waiting_players.indices().await.unwrap();
        let players: Vec<AccountOwner> = player_indices;

        if players.is_empty() {
            return;
        }

        // Get player chain IDs
        let mut player_chain_map = Vec::new();
        for player in &players {
            let chain_id = state.waiting_players.get(player).await.unwrap().unwrap();
            player_chain_map.push((*player, chain_id));
        }

        // Create multi-owner game chain
        let game_chain_id = runtime.open_chain(
            ChainOwnership::multiple(
                players.iter().map(|owner| (*owner, 1u64)),
                10, // multi_leader_rounds
                Default::default(), // timeout_config
            ),
            ApplicationPermissions::default(), // Full permissions
            Amount::ZERO, // No initial balance (lobby keeps the fees)
        );

        // Send initialization message to game chain
        let lobby_chain_id = runtime.chain_id();
        let platform_fee_recipient = state.lobby_owner.get().expect("Lobby owner not set");
        runtime
            .prepare_message(Message::InitializeGame {
                players: players.clone(),
                player_chains: player_chain_map,
                entry_fee: *state.entry_fee.get(),
                lobby_chain_id,
                platform_fee_recipient,
            })
            .send_to(game_chain_id);

        // Track active game
        let current_time = runtime.system_time();
        state.active_games
            .insert(&game_chain_id, current_time)
            .expect("Failed to track game");

        // Increment game count
        let count = *state.game_count.get();
        state.game_count.set(count + 1);

        // Update global stats
        for player in &players {
            let current = state.games_played
                .get(player).await.unwrap_or(Some(0)).unwrap_or(0);
            state.games_played.insert(player, current + 1)
                .expect("Failed to update stats");
        }

        // Clear waiting players
        for player in &players {
            state.waiting_players.remove(player).ok();
        }
    }

    async fn execute_lobby_message(
        state: &mut LobbyState,
        runtime: &mut ContractRuntime<Self>,
        message: Message,
    ) {
        match message {
            Message::RequestJoinLobby { player, player_chain } => {
                // Player sent cross-chain message to join
                // (Alternative to JoinLobby operation)

                // Check not already joined
                if state.waiting_players.contains_key(&player).await.unwrap() {
                    return; // Already joined, ignore
                }

                // Check max players
                let current_count = state.waiting_players.count().await.unwrap();
                if current_count >= MAX_PLAYERS_PER_GAME {
                    return; // Full, ignore
                }

                // Add to waiting list (entry fee already transferred)
                state.waiting_players
                    .insert(&player, player_chain)
                    .expect("Failed to add player");

                // Auto-start if enough players
                let player_count = state.waiting_players.count().await.unwrap();
                if player_count >= MIN_PLAYERS_TO_START {
                    Self::start_game(state, runtime).await;
                }
            }

            Message::GameResults { winners, eliminated, entry_fee, total_players, player_chains } => {
                // Game finished - distribute prizes

                // Build a map of player to chain for quick lookup
                let player_chain_map: std::collections::HashMap<AccountOwner, ChainId> =
                    player_chains.into_iter().collect();

                // Calculate prizes
                let total_collected = entry_fee.saturating_mul(total_players as u128);
                let prize_pool_attos = total_collected.saturating_mul(95).saturating_div(Amount::from_attos(100));

                // Send prizes to winners
                if !winners.is_empty() {
                    let prize_per_winner_attos = prize_pool_attos / (winners.len() as u128);
                    let prize_per_winner = Amount::from_attos(prize_per_winner_attos);

                    for winner in &winners {
                        // Send prize to winner's chain
                        if let Some(winner_chain) = player_chain_map.get(winner) {
                            runtime.prepare_message(Message::DistributePrize {
                                winner: *winner,
                                amount: prize_per_winner,
                            })
                            .send_to(*winner_chain);
                        }

                        // Update leaderboard - wins
                        let current_wins = state.games_won
                            .get(winner).await.unwrap_or(Some(0)).unwrap_or(0);
                        state.games_won.insert(winner, current_wins + 1)
                            .expect("Failed to update wins");

                        // Update leaderboard - total winnings
                        let current_winnings = state.total_winnings
                            .get(winner).await.unwrap_or(Some(Amount::ZERO)).unwrap_or(Amount::ZERO);
                        state.total_winnings.insert(winner, current_winnings.saturating_add(prize_per_winner))
                            .expect("Failed to update winnings");
                    }
                }

                // Update times_eliminated for eliminated players
                for eliminated_player in &eliminated {
                    let current_eliminations = state.times_eliminated
                        .get(eliminated_player).await.unwrap_or(Some(0)).unwrap_or(0);
                    state.times_eliminated.insert(eliminated_player, current_eliminations + 1)
                        .expect("Failed to update eliminations");
                }

                // Send platform fee to lobby owner
                let platform_fee_attos = total_collected.saturating_mul(5).saturating_div(Amount::from_attos(100));
                let platform_fee = Amount::from_attos(platform_fee_attos);
                let owner = state.lobby_owner.get().expect("Lobby owner not set");
                let owner_account = Account {
                    chain_id: runtime.chain_id(),
                    owner,
                };
                runtime.transfer(AccountOwner::CHAIN, owner_account, platform_fee);
            }

            _ => {} // Ignore other messages
        }
    }
}

// =============================================================================
// GAME OPERATIONS
// =============================================================================

impl MajorulesContract {
    async fn execute_game_operation(
        state: &mut GameState,
        runtime: &mut ContractRuntime<Self>,
        operation: Operation,
    ) {
        match operation {
            Operation::Increment { value } => {
                state.value.set(state.value.get() + value);
            }

            Operation::AskQuestion {
                question,
                option_a,
                option_b,
                option_c,
                questioner_answer,
            } => {
                // Verify game is active
                assert_eq!(*state.status.get(), STATUS_ACTIVE, "Game is not active");

                let caller = runtime
                    .authenticated_signer()
                    .expect("Must be authenticated");

                // Verify question hasn't been asked
                assert!(!*state.question_asked.get(), "Question already asked");

                // Check caller is in game and not eliminated
                let players = state.players.get();
                let eliminated = state.eliminated.get();
                assert!(players.contains(&caller), "Not in this game");
                assert!(!eliminated.contains(&caller), "You are eliminated");

                // Check timeout for questioner
                let current_time = runtime.system_time();
                let round_start = *state.round_start_time.get();
                let question_deadline = Timestamp::from(round_start.micros() + QUESTION_TIMEOUT_MICROS);

                let current_questioner = state.current_questioner.get()
                    .expect("No questioner set");

                if current_time < question_deadline {
                    assert_eq!(caller, current_questioner, "Only questioner can ask now");
                }

                // Validate
                assert!(questioner_answer >= 1 && questioner_answer <= 3, "Invalid answer");
                assert!(!question.trim().is_empty(), "Empty question");
                assert!(!option_a.trim().is_empty(), "Empty option A");
                assert!(!option_b.trim().is_empty(), "Empty option B");
                assert!(!option_c.trim().is_empty(), "Empty option C");

                // Store question
                state.question.set(question);
                state.option_a.set(option_a);
                state.option_b.set(option_b);
                state.option_c.set(option_c);
                state.question_asked.set(true);

                // Store questioner's answer
                state.player_answers.insert(&caller, questioner_answer)
                    .expect("Failed to store answer");

                // Set deadline
                state.round_start_time.set(current_time);
                let deadline = Timestamp::from(current_time.micros() + ANSWER_TIMEOUT_MICROS);
                state.answer_deadline.set(deadline);
            }

            Operation::SubmitAnswer { answer } => {
                assert_eq!(*state.status.get(), STATUS_ACTIVE, "Game not active");
                assert!(*state.question_asked.get(), "No question asked");

                let current_time = runtime.system_time();
                let deadline = *state.answer_deadline.get();
                assert!(current_time < deadline, "Deadline passed");

                let caller = runtime
                    .authenticated_signer()
                    .expect("Must be authenticated");

                assert!(answer >= 1 && answer <= 3, "Invalid answer");

                let players = state.players.get();
                let eliminated = state.eliminated.get();
                assert!(players.contains(&caller), "Not in game");
                assert!(!eliminated.contains(&caller), "Eliminated");

                // Allow changing answer
                if state.player_answers.contains_key(&caller).await.unwrap() {
                    state.player_answers.remove(&caller).ok();
                }

                state.player_answers.insert(&caller, answer)
                    .expect("Failed to store answer");
            }

            Operation::ProcessRound => {
                Self::process_round(state, runtime).await;
            }

            _ => panic!("Invalid operation for game chain"),
        }
    }

    async fn process_round(state: &mut GameState, runtime: &mut ContractRuntime<Self>) {
        assert_eq!(*state.status.get(), STATUS_ACTIVE, "Game not active");
        assert!(*state.question_asked.get(), "No question asked");

        let current_time = runtime.system_time();
        let deadline = *state.answer_deadline.get();
        assert!(current_time >= deadline, "Deadline not reached");

        // Count votes
        let mut count_a = 0u64;
        let mut count_b = 0u64;
        let mut count_c = 0u64;

        let players = state.players.get().clone();
        let eliminated = state.eliminated.get().clone();

        let mut non_answerers = Vec::new();
        let mut survivors_before = Vec::new();

        for player in &players {
            if !eliminated.contains(player) {
                survivors_before.push(*player);
                if let Ok(Some(answer)) = state.player_answers.get(player).await {
                    match answer {
                        1 => count_a += 1,
                        2 => count_b += 1,
                        3 => count_c += 1,
                        _ => {},
                    }
                } else {
                    non_answerers.push(*player);
                }
            }
        }

        let total_votes = count_a + count_b + count_c;

        // RULE 1: Three-way tie
        if count_a == count_b && count_b == count_c && total_votes > 0 {
            let revote_count = *state.revote_count.get();

            if revote_count >= MAX_REVOTES {
                // Eliminate everyone
                let mut new_eliminated = eliminated.clone();
                for player in &players {
                    if !new_eliminated.contains(player) {
                        new_eliminated.push(*player);
                    }
                }
                state.eliminated.set(new_eliminated);
                Self::clear_round_state(state, &players).await;
                state.status.set(STATUS_FINISHED);
                Self::send_game_results(state, runtime).await;
                return;
            }

            // Revote
            state.revote_count.set(revote_count + 1);
            Self::clear_round_state(state, &players).await;
            Self::select_random_questioner(state, &survivors_before);
            state.round_start_time.set(runtime.system_time());
            return;
        }

        // RULE 2: Everyone picks same option
        if (count_a > 0 && count_b == 0 && count_c == 0) ||
           (count_b > 0 && count_a == 0 && count_c == 0) ||
           (count_c > 0 && count_a == 0 && count_b == 0) {
            // Only eliminate non-answerers
            let mut new_eliminated = eliminated.clone();
            for player in &non_answerers {
                new_eliminated.push(*player);
            }
            state.eliminated.set(new_eliminated.clone());

            Self::clear_round_state(state, &players).await;

            let survivors_count = players.len() - new_eliminated.len();
            let current_round = *state.current_round.get();

            if survivors_count < 3 || current_round >= MAX_ROUNDS {
                state.status.set(STATUS_FINISHED);
                Self::send_game_results(state, runtime).await;
            } else {
                state.current_round.set(current_round + 1);
                state.revote_count.set(0);
                state.round_start_time.set(runtime.system_time());
                let mut survivors = Vec::new();
                for p in &players {
                    if !new_eliminated.contains(p) {
                        survivors.push(*p);
                    }
                }
                Self::select_random_questioner(state, &survivors);
            }
            return;
        }

        // RULE 3 & 4: Normal elimination
        let min_count = count_a.min(count_b).min(count_c);
        let mut options_to_eliminate = Vec::new();

        let mut min_count_options = 0;
        if count_a == min_count && count_a > 0 { min_count_options += 1; }
        if count_b == min_count && count_b > 0 { min_count_options += 1; }
        if count_c == min_count && count_c > 0 { min_count_options += 1; }

        if min_count_options >= 2 {
            if count_a == min_count && count_a > 0 { options_to_eliminate.push(1u8); }
            if count_b == min_count && count_b > 0 { options_to_eliminate.push(2u8); }
            if count_c == min_count && count_c > 0 { options_to_eliminate.push(3u8); }
        } else {
            if count_a == min_count && count_a > 0 { options_to_eliminate.push(1u8); }
            if count_b == min_count && count_b > 0 { options_to_eliminate.push(2u8); }
            if count_c == min_count && count_c > 0 { options_to_eliminate.push(3u8); }
        }

        let mut new_eliminated = eliminated.clone();
        for player in &players {
            if !new_eliminated.contains(player) {
                let should_eliminate = if let Ok(Some(answer)) = state.player_answers.get(player).await {
                    options_to_eliminate.contains(&answer)
                } else {
                    true
                };

                if should_eliminate {
                    new_eliminated.push(*player);
                }
            }
        }

        state.eliminated.set(new_eliminated.clone());
        Self::clear_round_state(state, &players).await;

        let survivors_count = players.len() - new_eliminated.len();
        let current_round = *state.current_round.get();

        if survivors_count < 3 || current_round >= MAX_ROUNDS {
            state.status.set(STATUS_FINISHED);
            Self::send_game_results(state, runtime).await;
        } else {
            state.current_round.set(current_round + 1);
            state.revote_count.set(0);
            state.round_start_time.set(runtime.system_time());
            let mut survivors = Vec::new();
            for p in &players {
                if !new_eliminated.contains(p) {
                    survivors.push(*p);
                }
            }
            Self::select_random_questioner(state, &survivors);
        }
    }

    async fn clear_round_state(state: &mut GameState, players: &[AccountOwner]) {
        for player in players {
            if state.player_answers.contains_key(player).await.unwrap_or(false) {
                state.player_answers.remove(player).ok();
            }
        }
        state.question_asked.set(false);
        state.question.set(String::new());
        state.option_a.set(String::new());
        state.option_b.set(String::new());
        state.option_c.set(String::new());
    }

    fn select_random_questioner(state: &mut GameState, survivors: &[AccountOwner]) {
        if !survivors.is_empty() {
            let idx = random_value(0, (survivors.len() - 1) as u64) as usize;
            state.current_questioner.set(Some(survivors[idx]));
        }
    }

    async fn send_game_results(state: &mut GameState, runtime: &mut ContractRuntime<Self>) {
        let players = state.players.get().clone();
        let eliminated = state.eliminated.get().clone();
        let entry_fee = *state.entry_fee.get();
        let lobby_chain_id = state.lobby_chain_id.get().expect("Lobby chain ID not set");

        let winners: Vec<AccountOwner> = players.iter()
            .filter(|p| !eliminated.contains(p))
            .copied()
            .collect();

        // Collect player chains for prize distribution
        let mut player_chains = Vec::new();
        for player in &players {
            if let Ok(Some(chain_id)) = state.player_chains.get(player).await {
                player_chains.push((*player, chain_id));
            }
        }

        runtime.prepare_message(Message::GameResults {
            winners,
            eliminated: eliminated.clone(),
            entry_fee,
            total_players: players.len(),
            player_chains,
        })
        .send_to(lobby_chain_id);
    }

    async fn execute_game_message(
        state: &mut GameState,
        runtime: &mut ContractRuntime<Self>,
        message: Message,
    ) {
        match message {
            Message::InitializeGame {
                players,
                player_chains,
                entry_fee,
                lobby_chain_id,
                platform_fee_recipient,
            } => {
                state.value.set(0);
                state.players.set(players.clone());
                state.eliminated.set(Vec::new());
                state.current_round.set(1);
                state.entry_fee.set(entry_fee);
                state.lobby_chain_id.set(Some(lobby_chain_id));
                state.platform_fee_recipient.set(Some(platform_fee_recipient));
                state.status.set(STATUS_ACTIVE);

                // Store player chains
                for (player, chain) in player_chains {
                    state.player_chains.insert(&player, chain)
                        .expect("Failed to store player chain");
                }

                // Select random questioner
                if !players.is_empty() {
                    let idx = random_value(0, (players.len() - 1) as u64) as usize;
                    state.current_questioner.set(Some(players[idx]));
                }

                state.question_asked.set(false);
                state.question.set(String::new());
                state.option_a.set(String::new());
                state.option_b.set(String::new());
                state.option_c.set(String::new());
                state.answer_deadline.set(Timestamp::from(0));
                state.revote_count.set(0);
                state.round_start_time.set(runtime.system_time());
            }

            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use futures::FutureExt as _;
    use linera_sdk::{
        linera_base_types::Amount,
        util::BlockingWait,
        views::View,
        Contract,
        ContractRuntime,
    };

    use majorules::Operation;

    use super::{MajorulesContract, LobbyState};

    #[test]
    fn operation() {
        let entry_fee = Amount::ZERO;
        let lobby_owner = linera_sdk::linera_base_types::AccountOwner::from([1u8; 32]);
        let mut app = create_and_instantiate_app(entry_fee, lobby_owner);

        let increment = 10u64;

        let _response = app
            .execute_operation(Operation::Increment { value: increment })
            .now_or_never()
            .expect("Execution should not await");

        if let MajorulesContract::Lobby { state, .. } = app {
            assert_eq!(*state.value.get(), increment);
        } else {
            panic!("Expected Lobby variant");
        }
    }

    fn create_and_instantiate_app(
        entry_fee: Amount,
        lobby_owner: linera_sdk::linera_base_types::AccountOwner,
    ) -> MajorulesContract {
        let runtime = ContractRuntime::new().with_application_parameters(());
        let state = LobbyState::load(runtime.root_view_storage_context())
            .blocking_wait()
            .expect("Failed to load state");

        let mut contract = MajorulesContract::Lobby { state, runtime };

        let init_arg = majorules::InitializationArgument {
            entry_fee,
            lobby_owner,
            is_lobby: true,
        };

        contract
            .instantiate(init_arg)
            .now_or_never()
            .expect("Instantiation should not await");

        contract
    }
}


use async_graphql::{Request, Response};
use linera_sdk::{
    graphql::GraphQLMutationRoot,
    linera_base_types::{AccountOwner, Amount, ChainId, ContractAbi, ServiceAbi},
};
use serde::{Deserialize, Serialize};

/// Initialization data for creating a new lobby or game
#[derive(Debug, Deserialize, Serialize)]
pub struct InitializationArgument {
    /// Entry fee for this lobby/game
    pub entry_fee: Amount,
    /// Owner of the lobby (receives 5% platform fee)
    pub lobby_owner: AccountOwner,
    /// Is this a lobby chain (true) or game chain (false)
    pub is_lobby: bool,
}

pub struct MajorulesAbi;

impl ContractAbi for MajorulesAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for MajorulesAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Operations that can be executed on the contract
#[derive(Debug, Deserialize, Serialize, GraphQLMutationRoot)]
pub enum Operation {
    /// Legacy increment operation for testing
    Increment { value: u64 },

    // ========== LOBBY OPERATIONS ==========

    /// Join the lobby from player's chain (sends cross-chain message)
    /// Player must call this from their own chain
    JoinLobby,

    /// Leave the lobby before game starts
    LeaveLobby,

    // ========== GAME OPERATIONS ==========

    /// Ask a question as the questioner (Game chain only)
    AskQuestion {
        question: String,
        option_a: String,
        option_b: String,
        option_c: String,
        questioner_answer: u8,
    },

    /// Submit an answer to the current question (Game chain only)
    SubmitAnswer { answer: u8 },

    /// Process round results and eliminate minority voters (Game chain only)
    ProcessRound,
}

/// Messages sent between chains
#[derive(Debug, Deserialize, Serialize)]
pub enum Message {
    // ===== PLAYER CHAIN → LOBBY =====

    /// Player wants to join lobby (sent from player's chain)
    RequestJoinLobby {
        player: AccountOwner,
        player_chain: ChainId,
    },

    // ===== LOBBY → GAME =====

    /// Initialize new game chain with these players
    InitializeGame {
        players: Vec<AccountOwner>,
        player_chains: Vec<(AccountOwner, ChainId)>,
        entry_fee: Amount,
        lobby_chain_id: ChainId,
        platform_fee_recipient: AccountOwner,
    },

    // ===== GAME → LOBBY =====

    /// Send game results to lobby for leaderboard and prize distribution
    GameResults {
        winners: Vec<AccountOwner>,
        eliminated: Vec<AccountOwner>,
        entry_fee: Amount,
        total_players: usize,
        player_chains: Vec<(AccountOwner, ChainId)>,
    },

    // ===== LOBBY → PLAYER CHAINS =====

    /// Distribute prize to winner (sent from lobby after receiving GameResults)
    DistributePrize {
        winner: AccountOwner,
        amount: Amount,
    },

    /// Send platform fee to lobby owner
    PlatformFee {
        amount: Amount,
    },
}

use std::sync::{Mutex, OnceLock};
use rand::{rngs::StdRng, Rng, SeedableRng};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    let seed = [0u8; 32];
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock")
        .fill(buf);
    Ok(())
}

getrandom::register_custom_getrandom!(custom_getrandom);


pub fn random_value(min: u64, max: u64) -> u64 {
    let seed = [0u8; 32]; // Use timestamp in production
    let mut rng = RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock");

    rng.gen_range(min..=max)
}


use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, ChainId, Timestamp},
    views::{linera_views, MapView, RegisterView, RootView, ViewStorageContext},
};

/// State for LOBBY chains
/// Manages waiting players and spawns game chains
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct LobbyState {
    /// Legacy counter for testing
    pub value: RegisterView<u64>,

    /// Players waiting in lobby with their chain IDs
    pub waiting_players: MapView<AccountOwner, ChainId>,

    /// Entry fee for this lobby tier
    pub entry_fee: RegisterView<Amount>,

    /// Total games created by this lobby
    pub game_count: RegisterView<u64>,

    /// Lobby owner (receives 5% platform fee)
    /// Wrapped in Option because AccountOwner doesn't implement Default
    pub lobby_owner: RegisterView<Option<AccountOwner>>,

    /// Active game chains spawned by this lobby
    pub active_games: MapView<ChainId, Timestamp>,

    // ============ GLOBAL LEADERBOARD ============
    /// Total games played per player (across all games)
    pub games_played: MapView<AccountOwner, u64>,

    /// Total games won per player
    pub games_won: MapView<AccountOwner, u64>,

    /// Total eliminations per player
    pub times_eliminated: MapView<AccountOwner, u64>,

    /// Total tokens won per player
    pub total_winnings: MapView<AccountOwner, Amount>,
}

/// State for GAME chains
/// Manages individual game sessions
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct GameState {
    /// Legacy counter for testing
    pub value: RegisterView<u64>,

    /// All players in this game
    pub players: RegisterView<Vec<AccountOwner>>,

    /// Eliminated players
    pub eliminated: RegisterView<Vec<AccountOwner>>,

    /// Current round (1-3)
    pub current_round: RegisterView<u64>,

    /// Current questioner
    pub current_questioner: RegisterView<Option<AccountOwner>>,

    /// Question state
    pub question_asked: RegisterView<bool>,
    pub question: RegisterView<String>,
    pub option_a: RegisterView<String>,
    pub option_b: RegisterView<String>,
    pub option_c: RegisterView<String>,

    /// Hidden player answers (1=A, 2=B, 3=C)
    pub player_answers: MapView<AccountOwner, u8>,

    /// Timing
    pub round_start_time: RegisterView<Timestamp>,
    pub answer_deadline: RegisterView<Timestamp>,
    pub revote_count: RegisterView<u8>,

    /// Game metadata
    pub status: RegisterView<u8>,
    pub entry_fee: RegisterView<Amount>,
    /// Wrapped in Option because ChainId doesn't implement Default
    pub lobby_chain_id: RegisterView<Option<ChainId>>,
    /// Wrapped in Option because AccountOwner doesn't implement Default
    pub platform_fee_recipient: RegisterView<Option<AccountOwner>>,

    /// Player chain mappings for prize distribution
    pub player_chains: MapView<AccountOwner, ChainId>,
}


#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use std::sync::Arc;

use async_graphql::{EmptySubscription, Object, Schema};
use linera_sdk::{
    graphql::GraphQLMutationRoot, linera_base_types::{AccountOwner, Amount, WithServiceAbi}, views::View, Service,
    ServiceRuntime,
};

use majorules::Operation;

use self::state::{LobbyState, GameState};

pub enum MajorulesService {
    Lobby {
        state: LobbyState,
        runtime: Arc<ServiceRuntime<Self>>,
    },
    Game {
        state: GameState,
        runtime: Arc<ServiceRuntime<Self>>,
    },
}

linera_sdk::service!(MajorulesService);

impl WithServiceAbi for MajorulesService {
    type Abi = majorules::MajorulesAbi;
}

impl Service for MajorulesService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        // Try loading as lobby first
        if let Ok(lobby_state) = LobbyState::load(runtime.root_view_storage_context()).await {
            let lobby_owner = *lobby_state.lobby_owner.get();
            if lobby_owner.is_some() {
                return Self::Lobby {
                    state: lobby_state,
                    runtime: Arc::new(runtime),
                };
            }
        }

        // Otherwise load as game
        let state = GameState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        Self::Game {
            state,
            runtime: Arc::new(runtime),
        }
    }

    async fn handle_query(&self, query: Self::Query) -> Self::QueryResponse {
        match self {
            Self::Lobby { state, runtime } => {
                Schema::build(
                    LobbyQueryRoot {
                        value: *state.value.get(),
                        entry_fee: *state.entry_fee.get(),
                        game_count: *state.game_count.get(),
                        lobby_owner: *state.lobby_owner.get(),
                    },
                    Operation::mutation_root(runtime.clone()),
                    EmptySubscription,
                )
                .finish()
                .execute(query)
                .await
            }
            Self::Game { state, runtime } => {
                Schema::build(
                    GameQueryRoot {
                        value: *state.value.get(),
                        players: state.players.get().clone(),
                        eliminated: state.eliminated.get().clone(),
                        current_round: *state.current_round.get(),
                        current_questioner: state.current_questioner.get().clone(),
                        question_asked: *state.question_asked.get(),
                        question: state.question.get().clone(),
                        option_a: state.option_a.get().clone(),
                        option_b: state.option_b.get().clone(),
                        option_c: state.option_c.get().clone(),
                        status: *state.status.get(),
                        entry_fee: *state.entry_fee.get(),
                    },
                    Operation::mutation_root(runtime.clone()),
                    EmptySubscription,
                )
                .finish()
                .execute(query)
                .await
            }
        }
    }
}

// ============================================================================
// LOBBY QUERY ROOT
// ============================================================================

struct LobbyQueryRoot {
    value: u64,
    entry_fee: Amount,
    game_count: u64,
    lobby_owner: Option<AccountOwner>,
}

#[Object]
impl LobbyQueryRoot {
    /// Legacy counter for testing
    async fn value(&self) -> u64 {
        self.value
    }

    /// Is this a lobby chain?
    async fn is_lobby(&self) -> bool {
        true
    }

    /// Entry fee for this lobby
    async fn entry_fee(&self) -> Amount {
        self.entry_fee
    }

    /// Total games created
    async fn game_count(&self) -> u64 {
        self.game_count
    }

    /// Lobby owner
    async fn lobby_owner(&self) -> Option<AccountOwner> {
        self.lobby_owner
    }
}

// ============================================================================
// GAME QUERY ROOT
// ============================================================================

struct GameQueryRoot {
    value: u64,
    players: Vec<AccountOwner>,
    eliminated: Vec<AccountOwner>,
    current_round: u64,
    current_questioner: Option<AccountOwner>,
    question_asked: bool,
    question: String,
    option_a: String,
    option_b: String,
    option_c: String,
    status: u8,
    entry_fee: Amount,
}

#[Object]
impl GameQueryRoot {
    /// Legacy counter for testing
    async fn value(&self) -> u64 {
        self.value
    }

    /// Is this a lobby chain?
    async fn is_lobby(&self) -> bool {
        false
    }

    /// All players in the game
    async fn players(&self) -> &Vec<AccountOwner> {
        &self.players
    }

    /// Eliminated players
    async fn eliminated(&self) -> &Vec<AccountOwner> {
        &self.eliminated
    }

    /// Survivors (players - eliminated)
    async fn survivors(&self) -> Vec<AccountOwner> {
        self.players
            .iter()
            .filter(|p| !self.eliminated.contains(p))
            .copied()
            .collect()
    }

    /// Number of survivors
    async fn survivor_count(&self) -> usize {
        self.players.len() - self.eliminated.len()
    }

    /// Current round (1-3)
    async fn current_round(&self) -> u64 {
        self.current_round
    }

    /// Current questioner
    async fn current_questioner(&self) -> &Option<AccountOwner> {
        &self.current_questioner
    }

    /// Has question been asked this round?
    async fn question_asked(&self) -> bool {
        self.question_asked
    }

    /// Current question
    async fn question(&self) -> &str {
        &self.question
    }

    /// Option A
    async fn option_a(&self) -> &str {
        &self.option_a
    }

    /// Option B
    async fn option_b(&self) -> &str {
        &self.option_b
    }

    /// Option C
    async fn option_c(&self) -> &str {
        &self.option_c
    }

    /// Game status: 0=Active, 1=Finished
    async fn status(&self) -> u8 {
        self.status
    }

    /// Entry fee
    async fn entry_fee(&self) -> Amount {
        self.entry_fee
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_graphql::{Request, Response, Value};
    use futures::FutureExt as _;
    use linera_sdk::{util::BlockingWait, views::View, Service, ServiceRuntime};
    use serde_json::json;

    use super::{MajorulesService, LobbyState};

    #[test]
    fn query() {
        let value = 60u64;
        let runtime = Arc::new(ServiceRuntime::<MajorulesService>::new());
        let mut state = LobbyState::load(runtime.root_view_storage_context())
            .blocking_wait()
            .expect("Failed to load state");
        state.value.set(value);
        state.lobby_owner.set(Some(linera_sdk::linera_base_types::AccountOwner::from([1u8; 32])));

        let service = MajorulesService::Lobby { state, runtime };
        let request = Request::new("{ value }");

        let response = service
            .handle_query(request)
            .now_or_never()
            .expect("Query should not await");

        let expected = Response::new(Value::from_json(json!({"value": 60})).unwrap());

        assert_eq!(response, expected)
    }
}


[package]
name = "majorules"
version = "0.1.0"
edition = "2021"

[dependencies]
async-graphql = { version = "=7.0.17", default-features = false }
linera-sdk = "0.15.6"
futures = { version = "0.3 "}
serde = { version = "1.0", features = ["derive"] }
serde_json = { version = "1.0" }
getrandom = { version = "0.2.12", default-features = false, features = ["custom"] }
rand = "0.8.5"

[dev-dependencies]
linera-sdk = { version = "0.15.6", features = ["test", "wasmer"] }
tokio = { version = "1.40", features = ["rt", "sync"] }

[[bin]]
name = "majorules_contract"
path = "src/contract.rs"

[[bin]]
name = "majorules_service"
path = "src/service.rs"

[profile.release]
debug = true
lto = true
opt-level = 'z'
strip = 'debuginfo'
