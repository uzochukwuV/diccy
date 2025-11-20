#![cfg_attr(target_arch = "wasm32", no_main)]

mod state;

use self::state::BattleTokenState;
use battle_token::{BattleTokenAbi, Message, Operation, TokenResponse};
use linera_sdk::{
    abi::WithContractAbi,
    linera_base_types::{AccountOwner, Amount},
    views::{RootView, View},
    Contract, ContractRuntime,
};

// Type alias for consistency
type Owner = AccountOwner;

/// Token Contract
pub struct BattleTokenContract {
    pub state: BattleTokenState,
    pub runtime: ContractRuntime<Self>,
}

linera_sdk::contract!(BattleTokenContract);

impl WithContractAbi for BattleTokenContract {
    type Abi = BattleTokenAbi;
}

impl Contract for BattleTokenContract {
    type Message = Message;
    type Parameters = Amount; // Initial supply
    type InstantiationArgument = ();
    type EventValue = ();

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = BattleTokenState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");

        Self { state, runtime }
    }

    async fn instantiate(&mut self, _argument: ()) {
        let initial_supply = self.runtime.application_parameters();
        let chain_ownership = self.runtime.chain_ownership();
        let creator = chain_ownership
            .super_owners
            .iter()
            .next()
            .expect("No super owners found")
            .clone();
        let now = self.runtime.system_time();

        // Initialize token metadata
        self.state.name.set("BattleChain Token".to_string());
        self.state.symbol.set("BATTLE".to_string());
        self.state.decimals.set(6);
        self.state.total_supply.set(initial_supply);
        self.state.total_transfers.set(0);
        self.state.total_holders.set(1);
        self.state.total_burned.set(Amount::ZERO);
        self.state.created_at.set(now);
        self.state.last_activity.set(now);

        // Set creator as admin
        self.state.admin.set(Some(creator.clone()));
        log::info!("BattleChain Token initialized with admin: {:?}", creator);

        // Mint initial supply to creator
        self.state.balances.insert(&creator, initial_supply).expect("Failed to set initial balance");
        let mut accounts = self.state.accounts.get().clone();
        accounts.push(creator);
        self.state.accounts.set(accounts);
    }

    async fn execute_operation(&mut self, operation: Operation) -> Self::Response {
        let caller = self
            .runtime
            .authenticated_signer()
            .expect("Must be authenticated");
        let now = self.runtime.system_time();

        match operation {
            // Query operations (no authentication needed for reads)
            Operation::Balance { owner } => {
                let balance = self.state.balance_of(&owner).await;
                log::info!("Balance query for {:?}: {}", owner, balance);
                TokenResponse::Balance(balance)
            }

            Operation::GetAllowance { owner, spender } => {
                let allowance = self.state.allowance(&owner, &spender).await;
                log::info!("Allowance query - owner: {:?}, spender: {:?}, allowance: {}", owner, spender, allowance);
                TokenResponse::Allowance(allowance)
            }

            // Write operations (require authentication)
            Operation::Transfer { to, amount } => {
                match self.state.transfer(caller, to, amount, now).await {
                    Ok(_) => {
                        log::info!("Transfer successful: {:?} -> {:?}, amount: {}", caller, to, amount);
                        TokenResponse::TransferSuccess
                    }
                    Err(e) => {
                        log::error!("Transfer failed: {:?} -> {:?}, amount: {}, error: {:?}", caller, to, amount, e);
                        panic!("Transfer failed: {:?}", e);
                    }
                }
            }

            Operation::Approve { spender, amount } => {
                match self.state.approve(caller, spender, amount).await {
                    Ok(_) => {
                        log::info!("Approval successful: owner {:?} approved {:?} to spend {}", caller, spender, amount);
                        TokenResponse::Ok
                    }
                    Err(e) => {
                        log::error!("Approval failed: owner {:?}, spender {:?}, amount: {}, error: {:?}", caller, spender, amount, e);
                        panic!("Approval failed: {:?}", e);
                    }
                }
            }

            Operation::TransferFrom { from, to, amount } => {
                match self.state.transfer_from(caller, from, to, amount, now).await {
                    Ok(_) => {
                        log::info!("TransferFrom successful: spender {:?} transferred {} from {:?} to {:?}", caller, amount, from, to);
                        TokenResponse::TransferSuccess
                    }
                    Err(e) => {
                        log::error!("TransferFrom failed: spender {:?}, from {:?}, to {:?}, amount: {}, error: {:?}", caller, from, to, amount, e);
                        panic!("TransferFrom failed: {:?}", e);
                    }
                }
            }

            Operation::Burn { amount } => {
                match self.state.burn(caller, amount, now).await {
                    Ok(_) => {
                        log::info!("Burn successful: {:?} burned {}", caller, amount);
                        TokenResponse::Ok
                    }
                    Err(e) => {
                        log::error!("Burn failed: {:?}, amount: {}, error: {:?}", caller, amount, e);
                        panic!("Burn failed: {:?}", e);
                    }
                }
            }

            Operation::Mint { to, amount } => {
                // SECURITY: Only admin can mint tokens
                let admin = self.state.admin.get().as_ref();
                if admin != Some(&caller) {
                    log::error!("Unauthorized mint attempt: {:?} tried to mint {} to {:?}. Only admin {:?} can mint.", caller, amount, to, admin);
                    panic!("Unauthorized: Only admin can mint tokens");
                }

                match self.state.mint(to, amount, now).await {
                    Ok(_) => {
                        log::info!("Mint successful: admin {:?} minted {} to {:?}", caller, amount, to);
                        TokenResponse::Ok
                    }
                    Err(e) => {
                        log::error!("Mint failed: admin {:?}, to {:?}, amount: {}, error: {:?}", caller, to, amount, e);
                        panic!("Mint failed: {:?}", e);
                    }
                }
            }

            Operation::Claim { amount } => {
                // For reward claims or initial distribution
                // SECURITY: Only admin can approve claims
                let admin = self.state.admin.get().as_ref();
                if admin != Some(&caller) {
                    log::error!("Unauthorized claim attempt: {:?} tried to claim {}. Only admin {:?} can process claims.", caller, amount, admin);
                    panic!("Unauthorized: Only admin can process claims");
                }

                match self.state.mint(caller, amount, now).await {
                    Ok(_) => {
                        log::info!("Claim successful: admin {:?} claimed {}", caller, amount);
                        TokenResponse::Ok
                    }
                    Err(e) => {
                        log::error!("Claim failed: {:?}, amount: {}, error: {:?}", caller, amount, e);
                        panic!("Claim failed: {:?}", e);
                    }
                }
            }
        }
    }

    async fn execute_message(&mut self, message: Message) {
        let now = self.runtime.system_time();

        match message {
            Message::Transfer {
                from,
                to,
                amount,
                target_chain: _,
            } => {
                // Deduct from sender on this chain
                match self.state.balance_of(&from).await {
                    balance if balance >= amount => {
                        match self.state.transfer(from, to, amount, now).await {
                            Ok(_) => {
                                log::info!("Cross-chain transfer debit successful: {:?} -> {:?}, amount: {}", from, to, amount);
                                // TODO: Send credit message to target chain
                                // self.runtime.send_message(target_chain, Message::Credit { recipient: to, amount });
                            }
                            Err(e) => {
                                log::error!("Cross-chain transfer debit failed: {:?} -> {:?}, amount: {}, error: {:?}", from, to, amount, e);
                                panic!("Cross-chain transfer debit failed: {:?}", e);
                            }
                        }
                    }
                    balance => {
                        log::error!("Insufficient balance for cross-chain transfer: {:?} has {}, needs {}", from, balance, amount);
                        panic!("Insufficient balance for cross-chain transfer");
                    }
                }
            }

            Message::Credit { recipient, amount } => {
                // Credit tokens received from another chain
                match self.state.mint(recipient, amount, now).await {
                    Ok(_) => {
                        log::info!("Cross-chain credit successful: minted {} to {:?}", amount, recipient);
                    }
                    Err(e) => {
                        log::error!("Cross-chain credit failed: recipient {:?}, amount: {}, error: {:?}", recipient, amount, e);
                        panic!("Cross-chain credit failed: {:?}", e);
                    }
                }
            }

            Message::Debit { sender, amount } => {
                // Confirmation of tokens sent to another chain
                match self.state.burn(sender, amount, now).await {
                    Ok(_) => {
                        log::info!("Cross-chain debit confirmation successful: burned {} from {:?}", amount, sender);
                    }
                    Err(e) => {
                        log::error!("Cross-chain debit confirmation failed: sender {:?}, amount: {}, error: {:?}", sender, amount, e);
                        panic!("Cross-chain debit confirmation failed: {:?}", e);
                    }
                }
            }
        }
    }

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
}
