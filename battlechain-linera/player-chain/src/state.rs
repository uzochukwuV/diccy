use battle_token::BattleTokenAbi;
use battlechain_shared_types::Owner;
use linera_sdk::{
    linera_base_types::{Amount, ApplicationId, ChainId, Timestamp},
    views::{MapView, RegisterView, RootView, ViewStorageContext},
};
use player_chain::PlayerChainError;

/// Player Chain State - manages player inventory and stats
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct PlayerChainState {
    /// All characters owned by this player
    pub characters: RegisterView<Vec<battlechain_shared_types::CharacterNFT>>,

    /// BATTLE token application reference
    pub battle_token_app: RegisterView<Option<ApplicationId<BattleTokenAbi>>>,

    /// Cached BATTLE balance
    pub battle_balance: RegisterView<Amount>,

    /// Locked BATTLE tokens (in battles)
    pub locked_battle: RegisterView<Amount>,

    /// Player stats
    pub total_battles: RegisterView<u64>,
    pub wins: RegisterView<u64>,
    pub losses: RegisterView<u64>,

    /// Active battles
    pub active_battles: RegisterView<Vec<ChainId>>,

    /// Locked stakes per battle
    pub battle_stakes: MapView<ChainId, Amount>,

    /// Character in each battle (battle_chain -> character_nft_id)
    pub battle_characters: MapView<ChainId, String>,

    /// SECURITY: Track known battle chains (for message authentication)
    pub known_battle_chains: MapView<ChainId, bool>,

    /// SECURITY: Admin owner (for pause functionality)
    pub admin: RegisterView<Option<Owner>>,

    /// SECURITY: Paused state
    pub paused: RegisterView<bool>,

    /// SECURITY: Rate limiting - last operation timestamp per operation type
    pub last_character_creation: RegisterView<Timestamp>,
    pub last_battle_join: RegisterView<Timestamp>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
    pub last_active: RegisterView<Timestamp>,
}

impl PlayerChainState {
    /// Get available BATTLE balance
    pub fn available_balance(&self) -> Amount {
        self.battle_balance.get().saturating_sub(*self.locked_battle.get())
    }

    /// Lock BATTLE for battle stake
    pub fn lock_battle(&mut self, battle_chain: ChainId, amount: Amount) -> Result<(), PlayerChainError> {
        if self.available_balance() < amount {
            return Err(PlayerChainError::InsufficientBalance {
                available: self.available_balance(),
                required: amount,
            });
        }

        let new_locked = self.locked_battle.get()
            .try_add(amount)
            .map_err(|_| PlayerChainError::MathOverflow)?;
        self.locked_battle.set(new_locked);

        self.battle_stakes.insert(&battle_chain, amount)
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?;

        Ok(())
    }

    /// Unlock BATTLE from battle
    pub async fn unlock_battle(&mut self, battle_chain: &ChainId) -> Result<Amount, PlayerChainError> {
        // Get the staked amount first
        let amount = self.battle_stakes
            .get(battle_chain)
            .await
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?
            .ok_or(PlayerChainError::BattleNotFound)?;

        // Remove the stake
        self.battle_stakes
            .remove(battle_chain)
            .map_err(|e| PlayerChainError::ViewError(format!("{:?}", e)))?;

        let new_locked = self.locked_battle.get()
            .try_sub(amount)
            .map_err(|_| PlayerChainError::MathOverflow)?;
        self.locked_battle.set(new_locked);

        Ok(amount)
    }

    /// Record battle result
    pub fn record_battle_result(&mut self, won: bool) {
        self.total_battles.set(*self.total_battles.get() + 1);
        if won {
            self.wins.set(*self.wins.get() + 1);
        } else {
            self.losses.set(*self.losses.get() + 1);
        }
    }
}
