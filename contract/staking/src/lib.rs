#![no_std]

use soroban_sdk::{
    Address, Env, Symbol, contract, contracterror, contractimpl, contracttype, symbol_short,
    token,
};

// ── Storage keys ──────────────────────────────────────────────────────────────

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const TOKEN_KEY: Symbol = symbol_short!("TOKEN");
pub const TOTAL_STAKED_KEY: Symbol = symbol_short!("TSTAKE");
const TOTAL_SHARES_KEY: Symbol = symbol_short!("TSHARES");

// ── Event topics ──────────────────────────────────────────────────────────────

const TOPIC_PAUSED: Symbol = symbol_short!("PAUSED");
const TOPIC_UNPAUSED: Symbol = symbol_short!("UNPAUSED");
const TOPIC_STAKE: Symbol = symbol_short!("STAKED");
const TOPIC_UNSTAKE: Symbol = symbol_short!("UNSTAKED");

// ── Error codes ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StakingError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Paused = 3,
    InvalidAmount = 4,
    InsufficientShares = 5,
    ZeroShares = 6,
}

// ── Storage key schema ────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
enum DataKey {
    Position(Address),
}

// ── Types ─────────────────────────────────────────────────────────────────────

/// Per-staker position record.
///
/// * `amount`  — total tokens currently deposited by this staker.
/// * `shares`  — shares currently held by this staker.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakePosition {
    pub amount: i128,
    pub shares: i128,
}

// ── Contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    /// Placeholder function — returns a fixed value for contract liveness checks.
    pub fn hello(_env: Env) -> u32 {
        101112
    }

    // ── Initialisation ───────────────────────────────────────────────────────

    /// Initialise the staking contract. Must be called exactly once after deployment.
    ///
    /// # Authorization
    /// Requires auth from the `admin` address to prevent front-running.
    pub fn __constructor(env: Env, admin: Address, token: Address) {
        admin.require_auth();
        env.storage().instance().set(&ADMIN_KEY, &admin);
        env.storage().instance().set(&TOKEN_KEY, &token);
    }

    /// Return the current admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("not initialized")
    }

    /// Return the staking token address.
    pub fn token(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&TOKEN_KEY)
            .expect("not initialized")
    }

    // ── Pause mechanism ──────────────────────────────────────────────────────

    /// Pause the contract. Prevents `stake` and `unstake` from executing.
    ///
    /// # Authorization
    /// Requires admin signature.
    pub fn pause(env: Env) {
        let admin = Self::admin(env.clone());
        admin.require_auth();
        env.storage().instance().set(&PAUSED_KEY, &true);
        env.events().publish((TOPIC_PAUSED,), ());
    }

    /// Unpause the contract. Restores normal `stake` and `unstake` operation.
    ///
    /// # Authorization
    /// Requires admin signature.
    pub fn unpause(env: Env) {
        let admin = Self::admin(env.clone());
        admin.require_auth();
        env.storage().instance().set(&PAUSED_KEY, &false);
        env.events().publish((TOPIC_UNPAUSED,), ());
    }

    /// Return whether the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        env.storage()
            .instance()
            .get(&PAUSED_KEY)
            .unwrap_or(false)
    }

    // ── Query functions ───────────────────────────────────────────────────────

    /// Total tokens currently held in the staking pool.
    pub fn total_staked(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&TOTAL_STAKED_KEY)
            .unwrap_or(0i128)
    }

    /// Total shares outstanding across all stakers.
    pub fn total_shares(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&TOTAL_SHARES_KEY)
            .unwrap_or(0i128)
    }

    /// Return the `StakePosition` for `staker`.
    pub fn get_position(env: Env, staker: Address) -> StakePosition {
        env.storage()
            .persistent()
            .get(&DataKey::Position(staker))
            .unwrap_or(StakePosition { amount: 0, shares: 0 })
    }

    /// Return the token amount currently staked by `staker`.
    pub fn staked_balance(env: Env, staker: Address) -> i128 {
        Self::get_position(env, staker).amount
    }

    // ── Staking ───────────────────────────────────────────────────────────────

    /// Deposit `amount` tokens and return the number of shares minted.
    ///
    /// Shares are minted proportionally: when the pool is empty, shares = amount;
    /// otherwise, shares = amount × total_shares / total_staked.
    ///
    /// # Errors
    /// * [`StakingError::Paused`] — Contract is paused.
    /// * [`StakingError::NotInitialized`] — Contract has not been initialized.
    /// * [`StakingError::InvalidAmount`] — `amount` is zero or negative.
    ///
    /// # Authorization
    /// Requires `staker.require_auth()`.
    pub fn stake(env: Env, staker: Address, amount: i128) -> Result<i128, StakingError> {
        require_not_paused(&env)?;
        staker.require_auth();

        if amount <= 0 {
            return Err(StakingError::InvalidAmount);
        }

        let token_contract = get_token_contract(&env)?;

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&TOTAL_STAKED_KEY)
            .unwrap_or(0);
        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&TOTAL_SHARES_KEY)
            .unwrap_or(0);

        let shares_minted = if total_staked == 0 || total_shares == 0 {
            amount
        } else {
            amount
                .checked_mul(total_shares)
                .and_then(|v| v.checked_div(total_staked))
                .unwrap_or(amount)
        };

        // CEI: update state before token transfer.
        env.storage()
            .instance()
            .set(&TOTAL_STAKED_KEY, &(total_staked + amount));
        env.storage()
            .instance()
            .set(&TOTAL_SHARES_KEY, &(total_shares + shares_minted));

        let mut position: StakePosition = env
            .storage()
            .persistent()
            .get(&DataKey::Position(staker.clone()))
            .unwrap_or(StakePosition { amount: 0, shares: 0 });
        position.amount += amount;
        position.shares += shares_minted;
        env.storage()
            .persistent()
            .set(&DataKey::Position(staker.clone()), &position);

        // Interaction: transfer tokens into the contract.
        token::Client::new(&env, &token_contract).transfer(
            &staker,
            &env.current_contract_address(),
            &amount,
        );

        env.events()
            .publish((TOPIC_STAKE,), (staker, amount, shares_minted));

        Ok(shares_minted)
    }

    /// Redeem `shares` shares and return the corresponding token amount.
    ///
    /// Tokens returned = shares × total_staked / total_shares.
    ///
    /// # Errors
    /// * [`StakingError::Paused`] — Contract is paused.
    /// * [`StakingError::NotInitialized`] — Contract has not been initialized.
    /// * [`StakingError::ZeroShares`] — `shares` is zero.
    /// * [`StakingError::InvalidAmount`] — `shares` is negative.
    /// * [`StakingError::InsufficientShares`] — `shares` exceeds staker's balance.
    ///
    /// # Authorization
    /// Requires `staker.require_auth()`.
    pub fn unstake(env: Env, staker: Address, shares: i128) -> Result<i128, StakingError> {
        require_not_paused(&env)?;
        staker.require_auth();

        if shares == 0 {
            return Err(StakingError::ZeroShares);
        }
        if shares < 0 {
            return Err(StakingError::InvalidAmount);
        }

        let mut position: StakePosition = env
            .storage()
            .persistent()
            .get(&DataKey::Position(staker.clone()))
            .unwrap_or(StakePosition { amount: 0, shares: 0 });
        if position.shares < shares {
            return Err(StakingError::InsufficientShares);
        }

        let total_staked: i128 = env
            .storage()
            .instance()
            .get(&TOTAL_STAKED_KEY)
            .unwrap_or(0);
        let total_shares: i128 = env
            .storage()
            .instance()
            .get(&TOTAL_SHARES_KEY)
            .unwrap_or(0);

        let tokens_returned = shares
            .checked_mul(total_staked)
            .and_then(|v| v.checked_div(total_shares))
            .unwrap_or(shares);

        let token_contract = get_token_contract(&env)?;

        // CEI: update state before token transfer.
        position.shares -= shares;
        position.amount = position.amount.saturating_sub(tokens_returned);
        env.storage()
            .persistent()
            .set(&DataKey::Position(staker.clone()), &position);
        env.storage()
            .instance()
            .set(&TOTAL_STAKED_KEY, &(total_staked - tokens_returned));
        env.storage()
            .instance()
            .set(&TOTAL_SHARES_KEY, &(total_shares - shares));

        // Interaction: transfer tokens back to staker.
        token::Client::new(&env, &token_contract).transfer(
            &env.current_contract_address(),
            &staker,
            &tokens_returned,
        );

        env.events()
            .publish((TOPIC_UNSTAKE,), (staker, tokens_returned, shares));

        Ok(tokens_returned)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_token_contract(env: &Env) -> Result<Address, StakingError> {
    env.storage()
        .instance()
        .get(&TOKEN_KEY)
        .ok_or(StakingError::NotInitialized)
}

fn require_not_paused(env: &Env) -> Result<(), StakingError> {
    if env
        .storage()
        .instance()
        .get(&PAUSED_KEY)
        .unwrap_or(false)
    {
        return Err(StakingError::Paused);
    }
    Ok(())
}

#[cfg(test)]
mod test;

#[cfg(test)]
mod integration_tests;
