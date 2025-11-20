use linera_sdk::{
    linera_base_types::{AccountOwner, Amount, Timestamp},
    views::{MapView, RegisterView, RootView, ViewStorageContext},
};

use crate::TokenError;

// Type alias for consistency
type Owner = AccountOwner;

/// Token State - manages all BATTLE token balances and operations
#[derive(RootView)]
#[view(context = ViewStorageContext)]
pub struct BattleTokenState {
    /// Token metadata
    pub name: RegisterView<String>,
    pub symbol: RegisterView<String>,
    pub decimals: RegisterView<u8>,
    pub total_supply: RegisterView<Amount>,

    /// Admin account (can mint tokens)
    pub admin: RegisterView<Option<Owner>>,

    /// Account balances (Owner -> Amount)
    pub balances: MapView<Owner, Amount>,

    /// Allowances for spending (owner, spender) -> amount
    pub allowances: MapView<(Owner, Owner), Amount>,

    /// Account registry for iteration
    pub accounts: RegisterView<Vec<Owner>>,

    /// Statistics
    pub total_transfers: RegisterView<u64>,
    pub total_holders: RegisterView<u64>,
    pub total_burned: RegisterView<Amount>,

    /// Timestamps
    pub created_at: RegisterView<Timestamp>,
    pub last_activity: RegisterView<Timestamp>,
}

impl BattleTokenState {
    /// Get balance of account
    pub async fn balance_of(&self, account: &Owner) -> Amount {
        self.balances
            .get(account)
            .await
            .unwrap_or(None)
            .unwrap_or(Amount::ZERO)
    }

    /// Transfer tokens between accounts
    pub async fn transfer(
        &mut self,
        from: Owner,
        to: Owner,
        amount: Amount,
        now: Timestamp,
    ) -> Result<(), TokenError> {
        // Validation
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        if from == to {
            return Err(TokenError::SelfTransfer);
        }

        // Check balance
        let from_balance = self.balance_of(&from).await;
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: from_balance,
                required: amount,
            });
        }

        // Deduct from sender
        let new_from_balance = from_balance
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&from, new_from_balance)?;

        // Add to recipient
        let to_balance = self.balance_of(&to).await;
        let new_to_balance = to_balance
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&to, new_to_balance)?;

        // Track new holder
        if to_balance == Amount::ZERO && amount > Amount::ZERO {
            let mut accounts = self.accounts.get().clone();
            if !accounts.contains(&to) {
                accounts.push(to);
                self.accounts.set(accounts);
                self.total_holders.set(*self.total_holders.get() + 1);
            }
        }

        // Update stats
        self.total_transfers.set(*self.total_transfers.get() + 1);
        self.last_activity.set(now);

        Ok(())
    }

    /// Approve spending allowance
    pub async fn approve(
        &mut self,
        owner: Owner,
        spender: Owner,
        amount: Amount,
    ) -> Result<(), TokenError> {
        if owner == spender {
            return Err(TokenError::SelfApproval);
        }

        self.allowances.insert(&(owner, spender), amount)?;
        Ok(())
    }

    /// Get allowance
    pub async fn allowance(&self, owner: &Owner, spender: &Owner) -> Amount {
        self.allowances
            .get(&(*owner, *spender))
            .await
            .unwrap_or(None)
            .unwrap_or(Amount::ZERO)
    }

    /// Transfer from allowance
    pub async fn transfer_from(
        &mut self,
        spender: Owner,
        from: Owner,
        to: Owner,
        amount: Amount,
        now: Timestamp,
    ) -> Result<(), TokenError> {
        // Check allowance
        let allowed = self.allowance(&from, &spender).await;
        if allowed < amount {
            return Err(TokenError::InsufficientAllowance {
                allowed,
                required: amount,
            });
        }

        // Reduce allowance
        let new_allowance = allowed
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.allowances
            .insert(&(from, spender), new_allowance)?;

        // Transfer tokens
        self.transfer(from, to, amount, now).await
    }

    /// Burn tokens (permanently remove from circulation)
    pub async fn burn(&mut self, from: Owner, amount: Amount, now: Timestamp) -> Result<(), TokenError> {
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        let balance = self.balance_of(&from).await;
        if balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: balance,
                required: amount,
            });
        }

        // Remove from account
        let new_balance = balance.try_sub(amount).map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&from, new_balance)?;

        // Reduce total supply
        let new_total_supply = self.total_supply.get()
            .try_sub(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_supply.set(new_total_supply);

        let new_total_burned = self.total_burned.get()
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_burned.set(new_total_burned);

        self.last_activity.set(now);

        Ok(())
    }

    /// Mint new tokens (admin only - for future use)
    pub async fn mint(&mut self, to: Owner, amount: Amount, now: Timestamp) -> Result<(), TokenError> {
        if amount == Amount::ZERO {
            return Err(TokenError::ZeroAmount);
        }

        // Add to recipient
        let balance = self.balance_of(&to).await;
        let new_balance = balance.try_add(amount).map_err(|_| TokenError::MathOverflow)?;
        self.balances.insert(&to, new_balance)?;

        // Increase total supply
        let new_total_supply = self.total_supply.get()
            .try_add(amount)
            .map_err(|_| TokenError::MathOverflow)?;
        self.total_supply.set(new_total_supply);

        // Track new holder
        if balance == Amount::ZERO {
            let mut accounts = self.accounts.get().clone();
            if !accounts.contains(&to) {
                accounts.push(to);
                self.accounts.set(accounts);
                self.total_holders.set(*self.total_holders.get() + 1);
            }
        }

        self.last_activity.set(now);

        Ok(())
    }
}
