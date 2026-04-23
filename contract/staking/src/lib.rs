#![no_std]

use soroban_sdk::{contract, contracterror, contractimpl, contracttype, Address, Env};

const MIN_STAKE: i128 = 10_000_000;
const MAX_STAKERS_DEFAULT: u32 = 10_000;
const STORAGE_DEPOSIT: i128 = 10_000_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum StakeError {
    BelowMinStake = 1,
    StakerCapReached = 2,
    NotAuthorized = 3,
    AlreadyStaked = 4,
    NotStaked = 5,
    InsufficientBalance = 6,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StakeInfo {
    pub amount: i128,
    pub staked_at: u64,
}

#[contracttype]
enum DataKey {
    Config,
    StakerCount,
    Staker(Address),
    StakeInfo(Address),
}

#[contract]
pub struct StakingContract;

#[contractimpl]
impl StakingContract {
    pub fn init(env: Env, max_stakers: u32) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("already initialized");
        }

        env.storage().instance().set(&DataKey::Config, &max_stakers);
    }

    pub fn get_config(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .unwrap_or(MAX_STAKERS_DEFAULT)
    }

    pub fn get_staker_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::StakerCount)
            .unwrap_or(0)
    }

    pub fn stake(env: Env, staker: Address, amount: i128) -> Result<(), StakeError> {
        if amount < MIN_STAKE {
            return Err(StakeError::BelowMinStake);
        }

        staker.require_auth();

        let is_new_staker = !env
            .storage()
            .instance()
            .has(&DataKey::Staker(staker.clone()));

        if is_new_staker {
            let current_count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::StakerCount)
                .unwrap_or(0);
            let max_stakers: u32 = env
                .storage()
                .instance()
                .get(&DataKey::Config)
                .unwrap_or(MAX_STAKERS_DEFAULT);

            if current_count >= max_stakers {
                return Err(StakeError::StakerCapReached);
            }

            env.storage()
                .instance()
                .set(&DataKey::StakerCount, &(current_count + 1));
        }

        env.storage()
            .instance()
            .set(&DataKey::Staker(staker.clone()), &true);

        let current_stake: i128 = env
            .storage()
            .instance()
            .get(&DataKey::StakeInfo(staker.clone()))
            .map(|s: StakeInfo| s.amount)
            .unwrap_or(0);

        let new_stake = current_stake + amount;
        env.storage().instance().set(
            &DataKey::StakeInfo(staker),
            &StakeInfo {
                amount: new_stake,
                staked_at: env.ledger().timestamp(),
            },
        );

        Ok(())
    }

    pub fn unstake(env: Env, staker: Address, amount: i128) -> Result<i128, StakeError> {
        staker.require_auth();

        let stake_info: StakeInfo = env
            .storage()
            .instance()
            .get(&DataKey::StakeInfo(staker.clone()))
            .ok_or(StakeError::NotStaked)?;

        if amount > stake_info.amount {
            return Err(StakeError::InsufficientBalance);
        }

        let remaining = stake_info.amount - amount;

        if remaining == 0 {
            env.storage()
                .instance()
                .remove(&DataKey::StakeInfo(staker.clone()));
            env.storage().instance().remove(&DataKey::Staker(staker));

            let current_count: u32 = env
                .storage()
                .instance()
                .get(&DataKey::StakerCount)
                .unwrap_or(0);
            if current_count > 0 {
                env.storage()
                    .instance()
                    .set(&DataKey::StakerCount, &(current_count - 1));
            }
        } else {
            env.storage().instance().set(
                &DataKey::StakeInfo(staker),
                &StakeInfo {
                    amount: remaining,
                    staked_at: stake_info.staked_at,
                },
            );
        }

        Ok(amount)
    }

    pub fn get_stake(env: Env, addr: Address) -> Option<StakeInfo> {
        env.storage().instance().get(&DataKey::StakeInfo(addr))
    }

    pub fn is_staker(env: Env, addr: Address) -> bool {
        env.storage().instance().has(&DataKey::Staker(addr))
    }

    pub fn min_stake() -> i128 {
        MIN_STAKE
    }

    pub fn storage_deposit() -> i128 {
        STORAGE_DEPOSIT
    }
}

#[cfg(test)]
mod test;
