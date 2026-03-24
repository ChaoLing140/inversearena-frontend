#[cfg(test)]
use super::*;
use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env};

fn setup() -> (Env, Address, PayoutContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PayoutContract, ());
    let client = PayoutContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let env_static: &'static Env = unsafe { &*(&env as *const Env) };
    let client = PayoutContractClient::new(env_static, &contract_id);
    (env, admin, client)
}

#[test]
fn test_initialize_sets_admin() {
    let (_env, admin, client) = setup();
    assert_eq!(client.admin(), admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_env, admin, client) = setup();
    client.initialize(&admin);
}

#[test]
fn test_admin_can_distribute_winnings() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = 1000i128;
    let currency = symbol_short!("XLM");

    assert!(!client.is_payout_processed(&idempotency_key, &winner));
    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);
    assert!(client.is_payout_processed(&idempotency_key, &winner));

    let payout = client.get_payout(&idempotency_key, &winner).unwrap();
    assert_eq!(payout.winner, winner);
    assert_eq!(payout.amount, amount);
    assert!(payout.paid);
}

#[test]
#[should_panic(expected = "caller is not authorized to distribute winnings")]
fn test_unauthorized_caller_cannot_distribute() {
    let (env, _admin, client) = setup();
    let unauthorized = Address::generate(&env);
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = 1000i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&unauthorized, &idempotency_key, &winner, &amount, &currency);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_zero_amount_panics() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = 0i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_negative_amount_panics() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = -100i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);
}

#[test]
#[should_panic(expected = "payout already processed for this idempotency key")]
fn test_idempotency_prevents_double_pay() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = 1000i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);
    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);
}

#[test]
fn test_different_idempotency_keys_allow_multiple_payouts() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let amount = 1000i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &1u32, &winner, &amount, &currency);
    client.distribute_winnings(&admin, &2u32, &winner, &amount, &currency);

    assert!(client.is_payout_processed(&1u32, &winner));
    assert!(client.is_payout_processed(&2u32, &winner));
}

#[test]
fn test_get_payout_returns_none_for_unprocessed() {
    let env = Env::default();
    let contract_id = env.register(PayoutContract, ());
    let client = PayoutContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    let winner = Address::generate(&env);
    assert!(client.get_payout(&1u32, &winner).is_none());
}

#[test]
fn test_get_payout_returns_data_for_processed() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let idempotency_key = 1u32;
    let amount = 5000i128;
    let currency = symbol_short!("USDC");

    client.distribute_winnings(&admin, &idempotency_key, &winner, &amount, &currency);

    let payout = client.get_payout(&idempotency_key, &winner).unwrap();
    assert_eq!(payout.winner, winner);
    assert_eq!(payout.amount, amount);
    assert_eq!(payout.currency, currency);
    assert!(payout.paid);
}
