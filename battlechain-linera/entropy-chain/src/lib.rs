use async_graphql::{EmptyMutation, EmptySubscription, Request, Response, Schema, SimpleObject};
use battlechain_shared_types::{EntropySeed, Owner};
use linera_sdk::{
    abi::{ContractAbi, ServiceAbi, WithContractAbi, WithServiceAbi},
    linera_base_types::{ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, View, ViewStorageContext},
    Contract, ContractRuntime, Service, ServiceRuntime,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Entropy Chain Application ABI
pub struct EntropyChainAbi;

impl ContractAbi for EntropyChainAbi {
    type Operation = Operation;
    type Response = ();
}

impl ServiceAbi for EntropyChainAbi {
    type Query = Request;
    type QueryResponse = Response;
}

/// Seed batch for entropy distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedBatch {
    pub batch_id: u64,
    pub seed: [u8; 32],
    pub start_index: u64,
    pub count: u32,
    pub consumed: u32,
    pub vrf_proof: Vec<u8>,
    pub created_at: Timestamp,
}

impl SeedBatch {
    /// Check if batch is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.consumed >= self.count
    }

    /// Get current global index
    pub fn current_global_index(&self) -> u64 {
        self.start_index + (self.consumed as u64)
    }

    /// Consume one seed from batch
    pub fn consume(&mut self) -> Option<u64> {
        if self.is_exhausted() {
            None
        } else {
            let index = self.current_global_index();
            self.consumed += 1;
            Some(index)
        }
    }

    /// Get remaining seeds in batch
    pub fn remaining(&self) -> u32 {
        self.count.saturating_sub(self.consumed)
    }
}

/// Entropy Chain State
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct EntropyState {
    /// Oracle authority (VRF provider) - wrapped in Option because Owner doesn't implement Default
    pub oracle: RegisterView<Option<Owner>>,

    /// VRF public key for verification
    pub vrf_public_key: RegisterView<[u8; 32]>,

    /// Seed batches (batch_id -> batch)
    pub seed_batches: MapView<u64, SeedBatch>,

    /// Current batch ID being consumed
    pub current_batch_id: RegisterView<u64>,

    /// Next batch ID to assign
    pub next_batch_id: RegisterView<u64>,

    /// Next global index (monotonic, prevents replay)
    pub global_next_index: RegisterView<u64>,

    /// Total seeds available
    pub total_available: RegisterView<u64>,

    /// Total seeds consumed
    pub total_consumed: RegisterView<u64>,

    /// Last refill timestamp
    pub last_refill: RegisterView<Timestamp>,

    /// Refill threshold (alert when < threshold)
    pub refill_threshold: RegisterView<u64>,
}

/// Entropy Chain Operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Refill seed batch (oracle only)
    RefillBatch {
        seed: [u8; 32],
        start_index: u64,
        count: u32,
        vrf_proof: Vec<u8>,
    },

    /// Request entropy (cross-chain call)
    RequestEntropy {
        requester_chain: ChainId,
        request_id: String,
    },

    /// Update refill threshold (oracle only)
    UpdateRefillThreshold { threshold: u64 },
}

/// Entropy Chain Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    /// Response with entropy seed
    EntropyResponse {
        request_id: String,
        seed: EntropySeed,
    },

    /// Refill alert
    RefillNeeded { remaining: u64, threshold: u64 },
}

#[derive(Debug, Error)]
pub enum EntropyError {
    #[error("Unauthorized: only oracle can perform this operation")]
    Unauthorized,

    #[error("Seed replay attempt: start_index {0} < global_next_index {1}")]
    SeedReplay(u64, u64),

    #[error("No entropy available")]
    NoEntropyAvailable,

    #[error("View error: {0}")]
    ViewError(String),
}

impl From<linera_sdk::views::ViewError> for EntropyError {
    fn from(error: linera_sdk::views::ViewError) -> Self {
        EntropyError::ViewError(error.to_string())
    }
}

/// Entropy Chain Contract
pub struct EntropyContract {
    state: EntropyState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(EntropyContract);

impl WithContractAbi for EntropyContract {
    type Abi = EntropyChainAbi;
}

impl Contract for EntropyContract {
    type Message = Message;
    type Parameters = (Owner, [u8; 32]); // (oracle, vrf_public_key)
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = EntropyState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let (oracle, vrf_public_key) = self.runtime.application_parameters();
        let now = self.runtime.system_time();

        self.state.oracle.set(Some(oracle));
        self.state.vrf_public_key.set(vrf_public_key);
        self.state.current_batch_id.set(0);
        self.state.next_batch_id.set(0);
        self.state.global_next_index.set(0);
        self.state.total_available.set(0);
        self.state.total_consumed.set(0);
        self.state.last_refill.set(now);
        self.state.refill_threshold.set(100); // Alert when < 100 seeds

        // Note: emit() is not used in v0.15.5 - use logging or events if needed
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        match operation {
            Operation::RefillBatch {
                seed,
                start_index,
                count,
                vrf_proof,
            } => {
                let _ = self.refill_batch(seed, start_index, count, vrf_proof).await;
            }
            Operation::RequestEntropy {
                requester_chain,
                request_id,
            } => {
                let _ = self.request_entropy(requester_chain, request_id).await;
            }
            Operation::UpdateRefillThreshold { threshold } => {
                let _ = self.update_refill_threshold(threshold).await;
            }
        }
    }

    async fn execute_message(&mut self, _message: Message) {
        // Entropy chain doesn't handle incoming messages (only sends)
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl EntropyContract {
    /// Verify caller is oracle
    async fn verify_oracle(&mut self) -> Result<(), EntropyError> {
        let caller = self
            .runtime
            .authenticated_signer()
            .ok_or(EntropyError::Unauthorized)?;

        let oracle = self
            .state
            .oracle
            .get()
            .as_ref()
            .ok_or(EntropyError::Unauthorized)?;

        if &caller != oracle {
            return Err(EntropyError::Unauthorized);
        }

        Ok(())
    }

    /// Refill with new seed batch
    async fn refill_batch(
        &mut self,
        seed: [u8; 32],
        start_index: u64,
        count: u32,
        vrf_proof: Vec<u8>,
    ) -> Result<(), EntropyError> {
        // Verify caller is oracle
        self.verify_oracle().await?;

        let now = self.runtime.system_time();
        let global_next = *self.state.global_next_index.get();

        // Monotonic check: start_index must be >= global_next_index
        if start_index < global_next {
            return Err(EntropyError::SeedReplay(start_index, global_next));
        }

        // TODO: Verify VRF proof against vrf_public_key
        // For now, trust the oracle

        let batch_id = *self.state.next_batch_id.get();
        let batch = SeedBatch {
            batch_id,
            seed,
            start_index,
            count,
            consumed: 0,
            vrf_proof,
            created_at: now,
        };

        // Store batch
        self.state.seed_batches.insert(&batch_id, batch)?;

        // Update counters
        self.state.next_batch_id.set(batch_id + 1);
        self.state
            .total_available
            .set(self.state.total_available.get() + count as u64);
        self.state
            .global_next_index
            .set(start_index + (count as u64));
        self.state.last_refill.set(now);

        Ok(())
    }

    /// Consume one entropy seed
    async fn consume_entropy(&mut self) -> Result<EntropySeed, EntropyError> {
        let mut current_batch_id = *self.state.current_batch_id.get();
        let next_batch_id = *self.state.next_batch_id.get();

        // Find first non-exhausted batch
        while current_batch_id < next_batch_id {
            if let Some(mut batch) = self
                .state
                .seed_batches
                .get(&current_batch_id)
                .await
                .map_err(|e| EntropyError::ViewError(e.to_string()))?
            {
                if batch.is_exhausted() {
                    // Move to next batch
                    current_batch_id += 1;
                    self.state.current_batch_id.set(current_batch_id);
                    continue;
                }

                // Consume from current batch
                if let Some(index) = batch.consume() {
                    // Update batch in storage
                    self.state
                        .seed_batches
                        .insert(&current_batch_id, batch.clone())
                        .map_err(|e| EntropyError::ViewError(e.to_string()))?;

                    // Update counters
                    self.state
                        .total_available
                        .set(self.state.total_available.get().saturating_sub(1));
                    self.state
                        .total_consumed
                        .set(self.state.total_consumed.get() + 1);

                    return Ok(EntropySeed {
                        seed: batch.seed,
                        index,
                        timestamp: batch.created_at,
                    });
                }
            }

            current_batch_id += 1;
            self.state.current_batch_id.set(current_batch_id);
        }

        Err(EntropyError::NoEntropyAvailable)
    }

    /// Check if refill is needed
    fn needs_refill(&self) -> bool {
        self.state.total_available.get() < self.state.refill_threshold.get()
    }

    /// Request entropy (called by other chains)
    async fn request_entropy(
        &mut self,
        requester_chain: ChainId,
        request_id: String,
    ) -> Result<(), EntropyError> {
        let entropy_seed = self.consume_entropy().await?;

        // Send entropy response back to requester
        let message = Message::EntropyResponse {
            request_id,
            seed: entropy_seed,
        };

        self.runtime
            .prepare_message(message)
            .with_authentication()
            .send_to(requester_chain);

        // Check if refill needed and send alert to oracle chain if low
        if self.needs_refill() {
            let alert_message = Message::RefillNeeded {
                remaining: *self.state.total_available.get(),
                threshold: *self.state.refill_threshold.get(),
            };

            // Send alert to application creator chain (oracle's chain)
            let oracle_chain = self.runtime.application_creator_chain_id();
            self.runtime
                .prepare_message(alert_message)
                .send_to(oracle_chain);
        }

        Ok(())
    }

    /// Update refill threshold
    async fn update_refill_threshold(&mut self, threshold: u64) -> Result<(), EntropyError> {
        // Verify caller is oracle
        self.verify_oracle().await?;

        self.state.refill_threshold.set(threshold);

        Ok(())
    }
}

/// Entropy Chain Service (GraphQL)
pub struct EntropyService {
    state: EntropyState,
}

impl WithServiceAbi for EntropyService {
    type Abi = EntropyChainAbi;
}

impl Service for EntropyService {
    type Parameters = ();

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = EntropyState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state }
    }

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot::new(&self.state).await,
            EmptyMutation,
            EmptySubscription,
        )
        .finish();

        schema.execute(request).await
    }
}

/// GraphQL Query Root (cloned data to avoid lifetime issues)
#[derive(Clone)]
struct QueryRoot {
    oracle: Option<String>,
    total_available: u64,
    total_consumed: u64,
    global_next_index: u64,
    current_batch_id: u64,
    next_batch_id: u64,
    refill_threshold: u64,
    seed_batches: Vec<SeedBatchInfo>,
}

impl QueryRoot {
    async fn new(state: &EntropyState) -> Self {
        let current_id = *state.current_batch_id.get();
        let next_id = *state.next_batch_id.get();

        let mut batches = Vec::new();
        for batch_id in current_id..next_id {
            if let Ok(Some(batch)) = state.seed_batches.get(&batch_id).await {
                batches.push(SeedBatchInfo {
                    batch_id: batch.batch_id,
                    start_index: batch.start_index,
                    count: batch.count,
                    consumed: batch.consumed,
                    remaining: batch.remaining(),
                });
            }
        }

        Self {
            oracle: state.oracle.get().map(|o| format!("{}", o)),
            total_available: *state.total_available.get(),
            total_consumed: *state.total_consumed.get(),
            global_next_index: *state.global_next_index.get(),
            current_batch_id: current_id,
            next_batch_id: next_id,
            refill_threshold: *state.refill_threshold.get(),
            seed_batches: batches,
        }
    }
}

#[async_graphql::Object]
impl QueryRoot {
    async fn oracle(&self) -> Option<String> {
        self.oracle.clone()
    }

    async fn total_available(&self) -> u64 {
        self.total_available
    }

    async fn total_consumed(&self) -> u64 {
        self.total_consumed
    }

    async fn global_next_index(&self) -> u64 {
        self.global_next_index
    }

    async fn current_batch_id(&self) -> u64 {
        self.current_batch_id
    }

    async fn next_batch_id(&self) -> u64 {
        self.next_batch_id
    }

    async fn needs_refill(&self) -> bool {
        self.total_available < self.refill_threshold
    }

    async fn refill_threshold(&self) -> u64 {
        self.refill_threshold
    }

    async fn seed_batches(&self) -> Vec<SeedBatchInfo> {
        self.seed_batches.clone()
    }
}

#[derive(Clone, SimpleObject)]
struct SeedBatchInfo {
    batch_id: u64,
    start_index: u64,
    count: u32,
    consumed: u32,
    remaining: u32,
}

linera_sdk::service!(EntropyService);
