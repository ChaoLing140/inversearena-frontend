#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const TOPIC_PAYOUT_EXECUTED: Symbol = symbol_short!("PAYOUT");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Payout(u32, Address),
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct PayoutData {
    pub winner: Address,
    pub amount: i128,
    pub currency: Symbol,
    pub paid: bool,
}

#[contract]
pub struct PayoutContract;

#[contractimpl]
impl PayoutContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN_KEY) {
            panic!("already initialized");
        }
        env.storage().instance().set(&ADMIN_KEY, &admin);
    }

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("not initialized")
    }

    pub fn distribute_winnings(
        env: Env,
        caller: Address,
        idempotency_key: u32,
        winner: Address,
        amount: i128,
        currency: Symbol,
    ) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN_KEY)
            .expect("not initialized");

        if caller != admin {
            panic!("caller is not authorized to distribute winnings");
        }

        if amount <= 0 {
            panic!("amount must be positive");
        }

        let payout_key = DataKey::Payout(idempotency_key, winner.clone());
        if env
            .storage()
            .instance()
            .get::<_, PayoutData>(&payout_key)
            .is_some()
        {
            panic!("payout already processed for this idempotency key");
        }

        let payout_data = PayoutData {
            winner: winner.clone(),
            amount,
            currency: currency.clone(),
            paid: true,
        };
        env.storage().instance().set(&payout_key, &payout_data);

        env.events()
            .publish((TOPIC_PAYOUT_EXECUTED,), (winner, amount, currency));
    }

    pub fn is_payout_processed(env: Env, idempotency_key: u32, winner: Address) -> bool {
        let payout_key = DataKey::Payout(idempotency_key, winner);
        env.storage()
            .instance()
            .get::<_, PayoutData>(&payout_key)
            .map(|p| p.paid)
            .unwrap_or(false)
    }

    pub fn get_payout(env: Env, idempotency_key: u32, winner: Address) -> Option<PayoutData> {
        let payout_key = DataKey::Payout(idempotency_key, winner);
        env.storage().instance().get(&payout_key)
    }
}

#[cfg(test)]
mod test;
