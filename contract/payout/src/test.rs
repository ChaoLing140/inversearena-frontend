#[cfg(test)]
use super::*;
use soroban_sdk::{
    symbol_short,
    testutils::Address as _,
    token::{StellarAssetClient, TokenClient},
    Address, Env, Vec,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (Env, Address, PayoutContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PayoutContract, ());
    let admin = Address::generate(&env);

    let env_static: &'static Env = unsafe { &*(&env as *const Env) };
    let client = PayoutContractClient::new(env_static, &contract_id);
    client.initialize(&admin);

    (env, admin, client)
}

fn setup_with_token() -> (
    Env,
    Address,
    PayoutContractClient<'static>,
    Address, // token id
    Address, // treasury
) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(PayoutContract, ());
    let admin = Address::generate(&env);
    let treasury = Address::generate(&env);

    let token_admin = Address::generate(&env);
    let token_id = env
        .register_stellar_asset_contract_v2(token_admin.clone())
        .address();
    StellarAssetClient::new(&env, &token_id).mint(&contract_id, &10_000i128);

    let env_static: &'static Env = unsafe { &*(&env as *const Env) };
    let client = PayoutContractClient::new(env_static, &contract_id);
    client.initialize(&admin);
    client.set_treasury(&treasury);

    (env, admin, client, token_id, treasury)
}

// ── initialize ────────────────────────────────────────────────────────────────

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

// ── distribute_winnings ───────────────────────────────────────────────────────

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
fn test_unauthorized_caller_cannot_distribute() {
    let (env, _admin, client) = setup();
    let unauthorized = Address::generate(&env);
    let winner = Address::generate(&env);
    let result = client.try_distribute_winnings(
        &unauthorized,
        &1u32,
        &winner,
        &1000i128,
        &symbol_short!("XLM"),
    );
    assert_eq!(result, Err(Ok(PayoutError::UnauthorizedCaller)));
}

#[test]
fn test_admin_spoofing_rejected_without_auth() {
    let env = Env::default();
    // No mock_all_auths — require_auth() must reject unsigned call
    let contract_id = env.register(PayoutContract, ());
    let client = PayoutContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let winner = Address::generate(&env);
    let result = client.try_distribute_winnings(
        &admin,
        &1u32,
        &winner,
        &1000i128,
        &symbol_short!("XLM"),
    );
    assert!(result.is_err());
}

#[test]
fn test_zero_amount_returns_error() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let result =
        client.try_distribute_winnings(&admin, &1u32, &winner, &0i128, &symbol_short!("XLM"));
    assert_eq!(result, Err(Ok(PayoutError::InvalidAmount)));
}

#[test]
fn test_negative_amount_returns_error() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let result =
        client.try_distribute_winnings(&admin, &1u32, &winner, &-500i128, &symbol_short!("XLM"));
    assert_eq!(result, Err(Ok(PayoutError::InvalidAmount)));
}

#[test]
fn test_idempotency_prevents_double_pay_same_amount() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let amount = 1000i128;
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &1u32, &winner, &amount, &currency);

    let second = client.try_distribute_winnings(&admin, &1u32, &winner, &amount, &currency);
    assert_eq!(second, Err(Ok(PayoutError::AlreadyPaid)));

    let payout = client.get_payout(&1u32, &winner).unwrap();
    assert_eq!(payout.amount, amount);
}

#[test]
fn test_idempotency_prevents_double_pay_different_amount() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let currency = symbol_short!("USDC");

    client.distribute_winnings(&admin, &99u32, &winner, &1000i128, &currency);

    let second = client.try_distribute_winnings(&admin, &99u32, &winner, &9999i128, &currency);
    assert_eq!(second, Err(Ok(PayoutError::AlreadyPaid)));

    let payout = client.get_payout(&99u32, &winner).unwrap();
    assert_eq!(payout.amount, 1000i128);
}

#[test]
fn test_different_idempotency_keys_allow_multiple_payouts() {
    let (env, admin, client) = setup();
    let winner = Address::generate(&env);
    let currency = symbol_short!("XLM");

    client.distribute_winnings(&admin, &1u32, &winner, &1000i128, &currency);
    client.distribute_winnings(&admin, &2u32, &winner, &1000i128, &currency);

    assert!(client.is_payout_processed(&1u32, &winner));
    assert!(client.is_payout_processed(&2u32, &winner));
}

// ── get_payout ────────────────────────────────────────────────────────────────

#[test]
fn test_get_payout_returns_none_for_unprocessed() {
    let env = Env::default();
    env.mock_all_auths();
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
    let amount = 5000i128;
    let currency = symbol_short!("USDC");

    client.distribute_winnings(&admin, &1u32, &winner, &amount, &currency);

    let payout = client.get_payout(&1u32, &winner).unwrap();
    assert_eq!(payout.winner, winner);
    assert_eq!(payout.amount, amount);
    assert_eq!(payout.currency, currency);
    assert!(payout.paid);
}

// ── distribute_prize ──────────────────────────────────────────────────────────

#[test]
fn test_distribute_prize_transfers_tokens_to_winners() {
    let (env, _admin, client, token_id, _treasury) = setup_with_token();
    let winner1 = Address::generate(&env);
    let winner2 = Address::generate(&env);
    let mut winners = Vec::new(&env);
    winners.push_back(winner1.clone());
    winners.push_back(winner2.clone());

    client.distribute_prize(&1u32, &1000i128, &winners, &token_id);

    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&winner1), 500i128);
    assert_eq!(token.balance(&winner2), 500i128);
}

#[test]
fn test_distribute_prize_sends_dust_to_treasury() {
    let (env, _admin, client, token_id, treasury) = setup_with_token();
    let winner1 = Address::generate(&env);
    let winner2 = Address::generate(&env);
    let winner3 = Address::generate(&env);
    let mut winners = Vec::new(&env);
    winners.push_back(winner1.clone());
    winners.push_back(winner2.clone());
    winners.push_back(winner3.clone());

    // 1000 / 3 = 333 each, dust = 1
    client.distribute_prize(&2u32, &1000i128, &winners, &token_id);

    let token = TokenClient::new(&env, &token_id);
    assert_eq!(token.balance(&winner1), 333i128);
    assert_eq!(token.balance(&winner2), 333i128);
    assert_eq!(token.balance(&winner3), 333i128);
    assert_eq!(token.balance(&treasury), 1i128);
}

#[test]
fn test_distribute_prize_idempotency_prevents_double_payout() {
    let (env, _admin, client, token_id, _treasury) = setup_with_token();
    let winner = Address::generate(&env);
    let mut winners = Vec::new(&env);
    winners.push_back(winner.clone());

    client.distribute_prize(&3u32, &500i128, &winners, &token_id);
    assert!(client.is_prize_distributed(&3u32));

    let second = client.try_distribute_prize(&3u32, &500i128, &winners, &token_id);
    assert_eq!(second, Err(Ok(PayoutError::AlreadyPaid)));
}

#[test]
fn test_distribute_prize_no_winners_returns_error() {
    let (env, _admin, client, token_id, _treasury) = setup_with_token();
    let empty: Vec<Address> = Vec::new(&env);
    let result = client.try_distribute_prize(&4u32, &1000i128, &empty, &token_id);
    assert_eq!(result, Err(Ok(PayoutError::NoWinners)));
}

#[test]
fn test_distribute_prize_invalid_amount_returns_error() {
    let (env, _admin, client, token_id, _treasury) = setup_with_token();
    let winner = Address::generate(&env);
    let mut winners = Vec::new(&env);
    winners.push_back(winner);
    let result = client.try_distribute_prize(&5u32, &0i128, &winners, &token_id);
    assert_eq!(result, Err(Ok(PayoutError::InvalidAmount)));
}
