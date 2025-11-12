use async_trait::async_trait;
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{MapView, RootView, ViewStorageContext},
    Contract, ContractRuntime, Service, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use shared_types::{CharacterClass, CharacterSnapshot, Owner};
use std::collections::HashMap;
use thiserror::Error;

/// Matchmaking chain ABI
pub struct MatchmakingAbi;

impl linera_sdk::abi::ContractAbi for MatchmakingAbi {
    type Operation = Operation;
    type Response = ();
}

impl linera_sdk::abi::ServiceAbi for MatchmakingAbi {
    type Query = ();
    type QueryResponse = ();
}

/// Battle offer status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferStatus {
    Open,
    Matched,
    Cancelled,
    Expired,
}

/// Battle offer type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferType {
    DirectChallenge { target_owner: Owner }, // Challenge specific player
    OpenChallenge,                            // Anyone can accept
    QuickMatch,                               // Auto-match with similar level
}

/// Battle offer created by a player
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BattleOffer {
    pub offer_id: u64,
    pub creator: Owner,
    pub creator_chain: ChainId,
    pub character_snapshot: CharacterSnapshot,
    pub stake: Amount,
    pub offer_type: OfferType,
    pub status: OfferStatus,
    pub created_at: Timestamp,
    pub expires_at: Timestamp,
}

/// Quick match queue entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueEntry {
    pub player: Owner,
    pub player_chain: ChainId,
    pub character_snapshot: CharacterSnapshot,
    pub stake: Amount,
    pub joined_at: Timestamp,
}

/// Match confirmation state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchState {
    AwaitingStakes,          // Waiting for both players to confirm stakes
    Player1StakeConfirmed,   // Player 1 locked stake
    Player2StakeConfirmed,   // Player 2 locked stake
    BothStakesConfirmed,     // Both confirmed, ready to start battle
    BattleInitialized,       // Battle chain created
}

/// Pending match awaiting stake confirmations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingMatch {
    pub match_id: u64,
    pub player1: Owner,
    pub player1_chain: ChainId,
    pub player1_character: CharacterSnapshot,
    pub player1_stake: Amount,
    pub player2: Owner,
    pub player2_chain: ChainId,
    pub player2_character: CharacterSnapshot,
    pub player2_stake: Amount,
    pub state: MatchState,
    pub created_at: Timestamp,
    pub battle_chain: Option<ChainId>,
}

/// Matchmaking chain state
#[derive(RootView)]
pub struct MatchmakingState {
    /// Open battle offers
    pub offers: MapView<u64, BattleOffer>,

    /// Quick match queue (owner -> queue entry)
    pub quick_match_queue: MapView<Owner, QueueEntry>,

    /// Pending matches awaiting confirmations
    pub pending_matches: MapView<u64, PendingMatch>,

    /// Active battle chains
    pub active_battles: MapView<ChainId, u64>, // battle_chain -> match_id

    /// Player to pending match mapping
    pub player_pending_match: MapView<Owner, u64>, // player -> match_id

    /// Counter for offer IDs
    pub offer_counter: u64,

    /// Counter for match IDs
    pub match_counter: u64,

    /// Battle token application ID
    pub battle_token_app: Option<ApplicationId>,

    /// Player chain registry (owner -> chain_id)
    pub player_chains: MapView<Owner, ChainId>,
}

/// Operations for the Matchmaking chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Initialize the matchmaking chain
    Initialize {
        battle_token_app: ApplicationId,
    },

    /// Register player's chain
    RegisterPlayerChain {
        player_chain: ChainId,
    },

    /// Create a battle offer
    CreateOffer {
        character_snapshot: CharacterSnapshot,
        stake: Amount,
        offer_type: OfferType,
        duration_minutes: u64, // How long offer stays open
    },

    /// Cancel an open offer
    CancelOffer {
        offer_id: u64,
    },

    /// Accept a challenge offer
    AcceptChallenge {
        offer_id: u64,
        character_snapshot: CharacterSnapshot,
    },

    /// Join quick match queue
    JoinQuickMatch {
        character_snapshot: CharacterSnapshot,
        stake: Amount,
    },

    /// Leave quick match queue
    LeaveQueue,

    /// Confirm stake has been locked (called by player chain)
    ConfirmStake {
        match_id: u64,
        player: Owner,
    },

    /// Clean up expired offers
    CleanExpiredOffers,
}

/// Messages sent to/from Matchmaking chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Request player to lock stake for battle
    LockStakeRequest {
        match_id: u64,
        amount: Amount,
        opponent: Owner,
        battle_chain: ChainId,
    },

    /// Notify players that battle is starting
    BattleReady {
        match_id: u64,
        battle_chain: ChainId,
        opponent: Owner,
    },

    /// Battle has completed (from battle chain)
    BattleCompleted {
        battle_chain: ChainId,
        winner: Owner,
        loser: Owner,
    },
}

/// Matchmaking errors
#[derive(Debug, Error, Serialize, Deserialize)]
pub enum MatchmakingError {
    #[error("Offer not found: {0}")]
    OfferNotFound(u64),

    #[error("Offer is not open")]
    OfferNotOpen,

    #[error("Cannot accept your own offer")]
    CannotAcceptOwnOffer,

    #[error("Not authorized to cancel this offer")]
    NotAuthorized,

    #[error("Player already in queue")]
    AlreadyInQueue,

    #[error("Player not in queue")]
    NotInQueue,

    #[error("Player already in pending match")]
    AlreadyInMatch,

    #[error("Match not found: {0}")]
    MatchNotFound(u64),

    #[error("Invalid match state")]
    InvalidMatchState,

    #[error("Insufficient stake amount")]
    InsufficientStake,

    #[error("Battle token app not initialized")]
    TokenAppNotInitialized,

    #[error("Player chain not registered")]
    PlayerChainNotRegistered,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for MatchmakingError {
    fn from(error: linera_sdk::views::ViewError) -> Self {
        MatchmakingError::ViewError(error.to_string())
    }
}

pub struct MatchmakingContract {
    state: MatchmakingState,
    runtime: ContractRuntime,
}

impl MatchmakingState {
    /// Check if player is already in a pending match
    pub async fn player_has_pending_match(&self, player: &Owner) -> Result<bool, MatchmakingError> {
        Ok(self.player_pending_match.get(player).await?.is_some())
    }

    /// Get player's registered chain
    pub async fn get_player_chain(&self, player: &Owner) -> Result<ChainId, MatchmakingError> {
        self.player_chains
            .get(player)
            .await?
            .ok_or(MatchmakingError::PlayerChainNotRegistered)
    }

    /// Create a new match and return match_id
    pub async fn create_match(
        &mut self,
        player1: Owner,
        player1_chain: ChainId,
        player1_character: CharacterSnapshot,
        player1_stake: Amount,
        player2: Owner,
        player2_chain: ChainId,
        player2_character: CharacterSnapshot,
        player2_stake: Amount,
        timestamp: Timestamp,
    ) -> Result<u64, MatchmakingError> {
        let match_id = self.match_counter;
        self.match_counter += 1;

        let pending_match = PendingMatch {
            match_id,
            player1,
            player1_chain,
            player1_character,
            player1_stake,
            player2,
            player2_chain,
            player2_character,
            player2_stake,
            state: MatchState::AwaitingStakes,
            created_at: timestamp,
            battle_chain: None,
        };

        self.pending_matches.insert(&match_id, pending_match)?;
        self.player_pending_match.insert(&player1, match_id)?;
        self.player_pending_match.insert(&player2, match_id)?;

        Ok(match_id)
    }
}

#[async_trait]
impl Contract for MatchmakingContract {
    type Error = MatchmakingError;
    type Storage = MatchmakingState;
    type State = MatchmakingState;
    type Message = Message;

    async fn new(state: Self::State, runtime: ContractRuntime) -> Result<Self, Self::Error> {
        Ok(MatchmakingContract { state, runtime })
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    async fn initialize(
        &mut self,
        _context: &linera_sdk::OperationContext,
        argument: Self::InitializationArgument,
    ) -> Result<(), Self::Error> {
        if let Operation::Initialize { battle_token_app } = argument {
            self.state.battle_token_app = Some(battle_token_app);
            self.state.offer_counter = 0;
            self.state.match_counter = 0;
            Ok(())
        } else {
            Err(MatchmakingError::ViewError(
                "Invalid initialization argument".to_string(),
            ))
        }
    }

    async fn execute_operation(
        &mut self,
        context: &linera_sdk::OperationContext,
        operation: Self::Operation,
    ) -> Result<(), Self::Error> {
        let caller = context.authenticated_signer.ok_or(MatchmakingError::NotAuthorized)?;
        let timestamp = context.system.timestamp;

        match operation {
            Operation::Initialize { .. } => {
                // Already handled in initialize()
                Ok(())
            }

            Operation::RegisterPlayerChain { player_chain } => {
                self.state.player_chains.insert(&caller, player_chain).await?;
                Ok(())
            }

            Operation::CreateOffer {
                character_snapshot,
                stake,
                offer_type,
                duration_minutes,
            } => {
                // Check if player already has pending match
                if self.state.player_has_pending_match(&caller).await? {
                    return Err(MatchmakingError::AlreadyInMatch);
                }

                let player_chain = self.state.get_player_chain(&caller).await?;
                let offer_id = self.state.offer_counter;
                self.state.offer_counter += 1;

                let expires_at = Timestamp::from(
                    timestamp.micros() + (duration_minutes * 60 * 1_000_000)
                );

                let offer = BattleOffer {
                    offer_id,
                    creator: caller,
                    creator_chain: player_chain,
                    character_snapshot,
                    stake,
                    offer_type,
                    status: OfferStatus::Open,
                    created_at: timestamp,
                    expires_at,
                };

                self.state.offers.insert(&offer_id, offer).await?;
                Ok(())
            }

            Operation::CancelOffer { offer_id } => {
                let mut offer = self.state.offers
                    .get(&offer_id)
                    .await?
                    .ok_or(MatchmakingError::OfferNotFound(offer_id))?;

                if offer.creator != caller {
                    return Err(MatchmakingError::NotAuthorized);
                }

                if offer.status != OfferStatus::Open {
                    return Err(MatchmakingError::OfferNotOpen);
                }

                offer.status = OfferStatus::Cancelled;
                self.state.offers.insert(&offer_id, offer).await?;
                Ok(())
            }

            Operation::AcceptChallenge {
                offer_id,
                character_snapshot,
            } => {
                // Check if player already has pending match
                if self.state.player_has_pending_match(&caller).await? {
                    return Err(MatchmakingError::AlreadyInMatch);
                }

                let mut offer = self.state.offers
                    .get(&offer_id)
                    .await?
                    .ok_or(MatchmakingError::OfferNotFound(offer_id))?;

                if offer.creator == caller {
                    return Err(MatchmakingError::CannotAcceptOwnOffer);
                }

                if offer.status != OfferStatus::Open {
                    return Err(MatchmakingError::OfferNotOpen);
                }

                // Check if offer is expired
                if timestamp.micros() > offer.expires_at.micros() {
                    offer.status = OfferStatus::Expired;
                    self.state.offers.insert(&offer_id, offer).await?;
                    return Err(MatchmakingError::OfferNotOpen);
                }

                // Check for direct challenge target
                if let OfferType::DirectChallenge { target_owner } = offer.offer_type {
                    if target_owner != caller {
                        return Err(MatchmakingError::NotAuthorized);
                    }
                }

                let challenger_chain = self.state.get_player_chain(&caller).await?;

                // Mark offer as matched
                offer.status = OfferStatus::Matched;
                self.state.offers.insert(&offer_id, offer.clone()).await?;

                // Create pending match
                let match_id = self.state.create_match(
                    offer.creator,
                    offer.creator_chain,
                    offer.character_snapshot.clone(),
                    offer.stake,
                    caller,
                    challenger_chain,
                    character_snapshot,
                    offer.stake, // Same stake amount
                    timestamp,
                ).await?;

                // TODO: Send stake lock requests to both player chains
                // self.runtime.send_message(player1_chain, Message::LockStakeRequest { ... });
                // self.runtime.send_message(player2_chain, Message::LockStakeRequest { ... });

                Ok(())
            }

            Operation::JoinQuickMatch {
                character_snapshot,
                stake,
            } => {
                // Check if already in queue
                if self.state.quick_match_queue.get(&caller).await?.is_some() {
                    return Err(MatchmakingError::AlreadyInQueue);
                }

                // Check if player already has pending match
                if self.state.player_has_pending_match(&caller).await? {
                    return Err(MatchmakingError::AlreadyInMatch);
                }

                let player_chain = self.state.get_player_chain(&caller).await?;

                let queue_entry = QueueEntry {
                    player: caller,
                    player_chain,
                    character_snapshot: character_snapshot.clone(),
                    stake,
                    joined_at: timestamp,
                };

                self.state.quick_match_queue.insert(&caller, queue_entry).await?;

                // Try to find a match in the queue
                // TODO: Implement matching logic based on level, stake, etc.
                // For now, we'll need to iterate through the queue to find a match
                // This requires additional helper methods for queue iteration

                Ok(())
            }

            Operation::LeaveQueue => {
                if self.state.quick_match_queue.get(&caller).await?.is_none() {
                    return Err(MatchmakingError::NotInQueue);
                }

                self.state.quick_match_queue.remove(&caller).await?;
                Ok(())
            }

            Operation::ConfirmStake { match_id, player } => {
                let mut pending_match = self.state.pending_matches
                    .get(&match_id)
                    .await?
                    .ok_or(MatchmakingError::MatchNotFound(match_id))?;

                // Update match state based on which player confirmed
                if player == pending_match.player1 {
                    match pending_match.state {
                        MatchState::AwaitingStakes => {
                            pending_match.state = MatchState::Player1StakeConfirmed;
                        }
                        MatchState::Player2StakeConfirmed => {
                            pending_match.state = MatchState::BothStakesConfirmed;
                        }
                        _ => return Err(MatchmakingError::InvalidMatchState),
                    }
                } else if player == pending_match.player2 {
                    match pending_match.state {
                        MatchState::AwaitingStakes => {
                            pending_match.state = MatchState::Player2StakeConfirmed;
                        }
                        MatchState::Player1StakeConfirmed => {
                            pending_match.state = MatchState::BothStakesConfirmed;
                        }
                        _ => return Err(MatchmakingError::InvalidMatchState),
                    }
                } else {
                    return Err(MatchmakingError::NotAuthorized);
                }

                // If both stakes confirmed, initialize battle chain
                if pending_match.state == MatchState::BothStakesConfirmed {
                    // TODO: Create Battle Chain and update pending_match.battle_chain
                    // TODO: Send BattleReady messages to both players
                    pending_match.state = MatchState::BattleInitialized;
                }

                self.state.pending_matches.insert(&match_id, pending_match).await?;
                Ok(())
            }

            Operation::CleanExpiredOffers => {
                // TODO: Iterate through offers and mark expired ones
                // This requires additional helper methods for iteration
                Ok(())
            }
        }
    }

    async fn execute_message(
        &mut self,
        context: &linera_sdk::MessageContext,
        message: Self::Message,
    ) -> Result<(), Self::Error> {
        match message {
            Message::LockStakeRequest { .. } => {
                // This would be sent TO player chains, not received here
                Ok(())
            }

            Message::BattleReady { .. } => {
                // This would be sent TO player chains, not received here
                Ok(())
            }

            Message::BattleCompleted {
                battle_chain,
                winner,
                loser,
            } => {
                // Clean up the pending match and active battle
                if let Some(match_id) = self.state.active_battles.get(&battle_chain).await? {
                    self.state.pending_matches.remove(&match_id).await?;
                    self.state.active_battles.remove(&battle_chain).await?;
                    self.state.player_pending_match.remove(&winner).await?;
                    self.state.player_pending_match.remove(&loser).await?;
                }
                Ok(())
            }
        }
    }

    async fn store(mut self) -> Result<Self::State, Self::Error> {
        self.state.save().await?;
        Ok(self.state)
    }
}

pub struct MatchmakingService {
    state: MatchmakingState,
    runtime: ServiceRuntime,
}

#[async_trait]
impl Service for MatchmakingService {
    type Error = MatchmakingError;
    type Storage = MatchmakingState;
    type State = MatchmakingState;

    async fn new(state: Self::State, runtime: ServiceRuntime) -> Result<Self, Self::Error> {
        Ok(MatchmakingService { state, runtime })
    }

    async fn handle_query(
        &mut self,
        _context: &linera_sdk::QueryContext,
        _query: Self::Query,
    ) -> Result<Self::QueryResponse, Self::Error> {
        // TODO: Implement GraphQL queries for:
        // - List open offers
        // - Get offer by ID
        // - Get quick match queue size
        // - Get player's pending match
        // - Get active battles count
        Ok(())
    }
}

linera_sdk::contract!(MatchmakingContract);
linera_sdk::service!(MatchmakingService);
