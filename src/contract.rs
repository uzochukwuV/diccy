#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use async_graphql::ComplexObject;
use esport::{DiceAbi, Operation, SettleOutcome};
use linera_sdk::{
    Contract, ContractRuntime, linera_base_types::{AccountOwner, WithContractAbi}, views::{RootView, View}
};
use serde::{Deserialize, Serialize};


use state::{DiceState, MatchId, MatchRecord, PlayerProfile};

pub struct DiceContract {
    state: DiceState,
    runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(DiceContract);

impl WithContractAbi for DiceContract {
    type Abi = DiceAbi;
}

impl Contract for DiceContract {
    type Message = ();
    type InstantiationArgument = ();
    type Parameters = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = DiceState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        DiceContract { state, runtime }
    }

    async fn instantiate(&mut self, _arg: ()) {
        // nothing to initialize for now
    }

    async fn execute_operation(&mut self, operation: Operation) -> SettleOutcome {
        match operation {
            Operation::RegisterPlayer { owner } => {
                self.register_player(owner).await;
                SettleOutcome {
                    success: true,
                    winner: None,
                    message: "Player registered".to_string(),
                }
            }
            Operation::StartMatch { players, rounds } => {
                let match_id = self.start_match(players, rounds).await;
                SettleOutcome {
                    success: true,
                    winner: None,
                    message: format!("Match started id={}", match_id),
                }
            }
            Operation::SettleMatch {
                match_id,
                seed0,
                seed1,
                hits0,
                hits1,
            } => {
                self.settle_match(match_id, seed0, seed1, hits0, hits1)
                    .await
            }
        }
    }

    async fn execute_message(&mut self, _message: Self::Message) {
        // No async messages for this simple scaffold.
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}

impl DiceContract {
    /// Registers a profile if missing.
    async fn register_player(&mut self, owner: AccountOwner) {
        let  profiles = self
            .state
            .profiles
            .get_mut_or_default(&owner.clone())
            .await
            .unwrap();
        let  profile = profiles;
        profile.owner = owner.clone();
        // keep existing xp/stats if present
        self.state.profiles.insert(&owner, profile.clone()).unwrap();
    }

    /// Start a new match; returns assigned match id.
    async fn start_match(&mut self, players: [AccountOwner; 2], rounds: u8) -> MatchId {
        let mut next = self.state.next_match_id.get();
        let match_id = *next;
        *self.state.next_match_id.get_mut() += 1;

        let mut record = MatchRecord::default();
        record.match_id = match_id;
        record.players = players.clone();
        record.rounds = rounds;
        record.hits_player0 = Vec::new();
        record.hits_player1 = Vec::new();
        record.winner = None;
        record.settled = false;

        self.state.matches.insert(&match_id, record).unwrap();
        match_id
    }

    /// Settle a match: verify seeds/hits match deterministic generation; update player stats.
    ///
    /// This method uses crypto primitives available in WASM at validation time. For the
    /// mock VRF/verification we use a SHA256-based expansion of seeds similar to the client.
    async fn settle_match(
        &mut self,
        match_id: MatchId,
        seed0: Vec<u8>,
        seed1: Vec<u8>,
        hits0: Vec<u32>,
        hits1: Vec<u32>,
    ) -> SettleOutcome {
        // Lookup match:
        let  record = match self.state.matches.get_mut(&match_id).await {
            Ok(Some(r)) => r,
            _ => {
                return SettleOutcome {
                    success: false,
                    winner: None,
                    message: "Match not found".to_string(),
                }
            }
        };

        if record.settled {
            return SettleOutcome {
                success: false,
                winner: None,
                message: "Match already settled".to_string(),
            };
        }

        // Recompute deterministic hits from seeds as the on-chain verification.
        // We use a simple SHA256-CTR expansion function to produce `rounds` values per player
        let recomputed0 = compute_hits_from_seeds(&seed0, &seed1, match_id, record.rounds, 1, 6);
        let recomputed1 = compute_hits_from_seeds(&seed1, &seed0, match_id, record.rounds, 1, 6);

        if recomputed0 != hits0 || recomputed1 != hits1 {
            // If hits don't match recomputed, fall back to mock VRF using current system_time to decide winner.
            // This mock is only for demo purposes.
            let now = self.runtime.system_time().micros() as u128;
            let winner_index = (now % 2) as usize;
            let winner = record.players[winner_index].clone();
            record.winner = Some(winner.clone());
            record.settled = true;
            self.state.matches.insert(&match_id, record.clone()).unwrap();
            // update profiles with a fallback result
            self.apply_settlement_effects(&record.players, winner_index)
                .await;
            return SettleOutcome {
                success: true,
                winner: Some(winner),
                message: "Seeds mismatched: mock VRF fallback used to pick winner".to_string(),
            };
        }

        // If recomputed hits match provided hits, compute total damage and determine winner.
        let total0: u32 = recomputed0.iter().sum();
        let total1: u32 = recomputed1.iter().sum();
        let winner_index = if total0 > total1 {
            0
        } else if total1 > total0 {
            1
        } else {
            2
        };

        let winner_opt = match winner_index {
            0 => Some(record.players[0].clone()),
            1 => Some(record.players[1].clone()),
            _ => None,
        };

        record.hits_player0 = recomputed0.clone();
        record.hits_player1 = recomputed1.clone();
        record.winner = winner_opt.clone();
        record.settled = true;
        self.state.matches.insert(&match_id, record.clone()).unwrap();

        // Apply XP and stats updates for the two players.
        match winner_index {
            0 | 1 => {
                self.apply_settlement_effects(&record.players, winner_index)
                    .await;
                SettleOutcome {
                    success: true,
                    winner: winner_opt,
                    message: "Match settled on-chain".to_string(),
                }
            }
            _ => SettleOutcome {
                success: true,
                winner: None,
                message: "Tie: no winner".to_string(),
            },
        }
    }

    /// Update player xp/wins/losses and apply simple level-up rules.
    async fn apply_settlement_effects(&mut self, players: &[AccountOwner; 2], winner_index: usize) {
        // Winner gets +100 xp, loser +20 xp; level up every 200 xp -> +1 max damage.
        for i in 0..2 {
            let owner = players[i].clone();
            let profile = self
                .state
                .profiles
                .get_mut_or_default(&owner.clone())
                .await
                .unwrap();
            if profile.owner == AccountOwner::Reserved(4) {
                // first time default -> set owner
                profile.owner = owner.clone();
            }
            if i == winner_index {
                profile.xp += 100;
                profile.wins += 1;
            } else {
                profile.xp += 20;
                profile.losses += 1;
            }
            // level up logic
            let new_level = 1 + (profile.xp / 200) as u32;
            if new_level > profile.level {
                let leveled = new_level - profile.level;
                profile.level = new_level;
                // each level increments max_damage modestly
                profile.max_damage = profile.max_damage.saturating_add(leveled);
                profile.hp_max = profile.hp_max.saturating_add((leveled * 5) as u32);
            }
            // store back
            self.state.profiles.insert(&owner.clone(), profile.clone()).unwrap();
        }
    }
}

/// Deterministic expansion of seeds into per-round hits.
/// For simplicity we map each SHA256(counter) digest into an integer within [min_damage, max_damage].
fn compute_hits_from_seeds(
    seed_self: &Vec<u8>,
    seed_other: &Vec<u8>,
    match_id: MatchId,
    rounds: u8,
    min_damage: u32,
    max_damage: u32,
) -> Vec<u32> {
    use sha2::{Digest, Sha256};
    let mut hits: Vec<u32> = Vec::with_capacity(rounds as usize);
    for r in 0..rounds {
        // build input: seed_self || seed_other || match_id || round
        let mut hasher = Sha256::new();
        hasher.update(seed_self);
        hasher.update(seed_other);
        hasher.update(&match_id.to_be_bytes());
        hasher.update(&[r]);
        let digest = hasher.finalize();
        // take first 4 bytes as u32
        let d = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
        let range = max_damage.saturating_sub(min_damage).saturating_add(1);
        let mapped = if range == 0 {
            min_damage
        } else {
            min_damage + (d % range)
        };
        hits.push(mapped);
    }
    hits
}

// #[derive(ComplexObject)]
// impl DiceState {}
