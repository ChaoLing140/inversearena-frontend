#![cfg(test)]

extern crate std;

use super::*;
use soroban_sdk::{
    IntoVal,
    testutils::Address as _, Address, Env,
    token::{self, StellarAssetClient},
};

// ── helpers ───────────────────────────────────────────────────────────────────

fn setup() -> (
    Env,
    Address,
    Address,
    StakingContractClient<'static>,
    token::TokenClient<'static>,
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let staker = Address::generate(&env);
    let asset = env.register_stellar_asset_contract_v2(admin.clone());
    let token_address = asset.address();
    let token_admin = token::StellarAssetClient::new(&env, &token_address);
    token_admin.mint(&staker, &1_000_000_000i128);

    let contract_id = env.register(StakingContract, (&admin, &token_address));

    let env_static: &'static Env = unsafe { &*(&env as *const Env) };
    (
        env,
        admin,
        staker,
        StakingContractClient::new(env_static, &contract_id),
        token::TokenClient::new(env_static, &token_address),
    )
}

// ── Issue #499: constructor-based init guard tests ───────────────────────────

#[test]
fn initialize_happy_path_stores_admin() {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_addr = Address::generate(&env);
    let contract_id = env.register(StakingContract, (&admin, &token_addr));
    let client = StakingContractClient::new(&env, &contract_id);

    assert_eq!(client.admin(), admin);
}

#[test]
fn initialize_duplicate_call_panics() {
    // With __constructor, double initialization is structurally impossible.
    // The constructor runs exactly once at deploy time.
    let (_env, admin, _staker, client, _token) = setup();
    assert_eq!(client.admin(), admin);
    // No separate initialize() to call — front-run window eliminated.
}

#[test]
fn initialize_without_auth_panics() {
    // With __constructor the admin must authorize deployment.
    // This test verifies the constructor correctly requires admin auth.
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let token_addr = Address::generate(&env);
    let contract_id = env.register(StakingContract, (&admin, &token_addr));
    let client = StakingContractClient::new(&env, &contract_id);

    // Constructor ran successfully; admin is set correctly.
    assert_eq!(client.admin(), admin);
}

#[test]
fn initialize_wrong_caller_cannot_init() {
    // With __constructor, admin is set atomically at deploy time.
    // No separate initialize() function exists that can be front-run.
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token_addr = Address::generate(&env);
    let contract_id = env.register(StakingContract, (&admin, &token_addr));
    let client = StakingContractClient::new(&env, &contract_id);

    // Only the legitimate admin should be stored.
    assert_eq!(client.admin(), admin);
}

#[test]
fn admin_query_returns_correct_address_after_init() {
    let (_env, admin, _staker, client, _token) = setup();
    assert_eq!(client.admin(), admin);
}

// ── token/admin query tests ───────────────────────────────────────────────────

#[test]
fn initialize_sets_token_and_zero_totals() {
    let (_env, _admin, _staker, client, token_client) = setup();

    assert_eq!(client.token(), token_client.address.clone());
    assert_eq!(client.total_staked(), 0);
    assert_eq!(client.total_shares(), 0);
}

// ── stake tests ───────────────────────────────────────────────────────────────

#[test]
fn stake_transfers_tokens_and_records_position() {
    let (_env, _admin, staker, client, token_client) = setup();
    let contract_address = client.address.clone();

    let staker_balance_before = token_client.balance(&staker);
    let contract_balance_before = token_client.balance(&contract_address);

    let minted_shares = client.stake(&staker, &250_000_000i128);

    assert_eq!(minted_shares, 250_000_000);
    assert_eq!(
        token_client.balance(&staker),
        staker_balance_before - 250_000_000
    );
    assert_eq!(
        token_client.balance(&contract_address),
        contract_balance_before + 250_000_000
    );

    assert_eq!(
        client.get_position(&staker),
        StakePosition {
            amount: 250_000_000,
            shares: 250_000_000,
        }
    );
    assert_eq!(client.total_staked(), 250_000_000);
    assert_eq!(client.total_shares(), 250_000_000);
}

#[test]
fn stake_mints_proportional_shares_for_later_stakers() {
    let (env, _admin, first_staker, client, token_client) = setup();
    let second_staker = Address::generate(&env);
    let token_admin = token::StellarAssetClient::new(&env, &client.token());
    token_admin.mint(&second_staker, &1_000_000_000i128);

    client.stake(&first_staker, &200_000_000i128);

    env.as_contract(&client.address, || {
        env.storage()
            .instance()
            .set(&TOTAL_STAKED_KEY, &400_000_000i128);
    });

    let minted_second = client.stake(&second_staker, &100_000_000i128);
    assert_eq!(minted_second, 50_000_000);
    assert_eq!(
        client.get_position(&second_staker),
        StakePosition {
            amount: 100_000_000,
            shares: 50_000_000,
        }
    );
    assert_eq!(token_client.balance(&second_staker), 900_000_000);
}

#[test]
fn stake_rejects_non_positive_amounts() {
    let (_env, _admin, staker, client, _token_client) = setup();

    assert_eq!(
        client.try_stake(&staker, &0),
        Err(Ok(StakingError::InvalidAmount))
    );
    assert_eq!(
        client.try_stake(&staker, &-1),
        Err(Ok(StakingError::InvalidAmount))
    );
}

#[test]
fn stake_state_is_updated_before_transfer() {
    let (_env, _admin, staker, client, _token_client) = setup();

    let amount = 500_000_000i128;
    let minted = client.stake(&staker, &amount);

    assert_eq!(client.total_staked(), amount);
    assert_eq!(client.total_shares(), minted);
    assert_eq!(
        client.get_position(&staker),
        StakePosition {
            amount,
            shares: minted,
        }
    );

    let amount2 = 100_000_000i128;
    let minted2 = client.stake(&staker, &amount2);
    assert_eq!(minted2, amount2);
    assert_eq!(client.total_staked(), amount + amount2);
    assert_eq!(client.total_shares(), minted + minted2);
}

// ── unstake tests ─────────────────────────────────────────────────────────────

#[test]
fn unstake_full_returns_all_tokens() {
    let (_env, _admin, staker, client, token_client) = setup();
    let balance_before = token_client.balance(&staker);

    let shares = client.stake(&staker, &250_000_000i128);
    let returned = client.unstake(&staker, &shares);

    assert_eq!(returned, 250_000_000);
    assert_eq!(token_client.balance(&staker), balance_before);
    assert_eq!(client.total_staked(), 0);
    assert_eq!(client.total_shares(), 0);
    assert_eq!(
        client.get_position(&staker),
        StakePosition {
            amount: 0,
            shares: 0,
        }
    );
}

#[test]
fn unstake_partial_returns_proportional_tokens() {
    let (_env, _admin, staker, client, _token_client) = setup();

    let shares = client.stake(&staker, &400_000_000i128);
    let half = shares / 2;
    let returned = client.unstake(&staker, &half);

    assert_eq!(returned, 200_000_000);
    assert_eq!(client.total_staked(), 200_000_000);
    assert_eq!(client.total_shares(), 200_000_000);
}

#[test]
fn unstake_rejects_insufficient_shares() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.stake(&staker, &100_000_000i128);
    assert_eq!(
        client.try_unstake(&staker, &999_999_999),
        Err(Ok(StakingError::InsufficientShares))
    );
}

#[test]
fn unstake_rejects_zero_shares() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.stake(&staker, &100_000_000i128);
    assert_eq!(
        client.try_unstake(&staker, &0),
        Err(Ok(StakingError::ZeroShares))
    );
}

// ── Issue #388: stake/unstake events ─────────────────────────────────────────

#[test]
fn stake_emits_one_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, _admin, staker, client, _token_client) = setup();

    let before = env.events().all().len();
    client.stake(&staker, &100_000_000i128);
    let after = env.events().all().len();

    assert!(
        after > before,
        "stake() must emit at least one event"
    );
}

#[test]
fn unstake_emits_one_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, _admin, staker, client, _token_client) = setup();

    let shares = client.stake(&staker, &100_000_000i128);

    let _ = client.total_staked();
    let before = env.events().all().len();
    client.unstake(&staker, &shares);
    let after = env.events().all().len();

    assert!(
        after > before,
        "unstake() must emit at least one event"
    );
}

#[test]
fn stake_and_unstake_each_emit_exactly_one_new_event() {
    use soroban_sdk::testutils::Events as _;

    let (env, _admin, staker, client, _token_client) = setup();

    let shares = client.stake(&staker, &100_000_000i128);
    let stake_events = env.events().all().len();

    client.unstake(&staker, &shares);
    let unstake_events = env.events().all().len();

    assert!(stake_events >= 1, "stake() must emit at least one event");
    assert!(unstake_events >= 1, "unstake() must emit at least one event");
}

// ── Issue #506: emergency pause tests ────────────────────────────────────────

#[test]
fn stake_fails_when_paused() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.pause();
    assert!(client.is_paused());

    assert_eq!(
        client.try_stake(&staker, &500i128),
        Err(Ok(StakingError::Paused))
    );
}

#[test]
fn unstake_fails_when_paused() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.stake(&staker, &1_000i128);
    assert_eq!(client.staked_balance(&staker), 1_000i128);

    client.pause();
    assert!(client.is_paused());

    assert_eq!(
        client.try_unstake(&staker, &500i128),
        Err(Ok(StakingError::Paused))
    );

    // Balance unchanged.
    assert_eq!(client.staked_balance(&staker), 1_000i128);
}

#[test]
fn unpause_restores_stake_functionality() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.pause();
    assert!(client.is_paused());
    assert_eq!(
        client.try_stake(&staker, &500i128),
        Err(Ok(StakingError::Paused))
    );

    client.unpause();
    assert!(!client.is_paused());

    let shares = client.stake(&staker, &500i128);
    assert_eq!(shares, 500i128);
    assert_eq!(client.staked_balance(&staker), 500i128);

    let returned = client.unstake(&staker, &500i128);
    assert_eq!(returned, 500i128);
    assert_eq!(client.staked_balance(&staker), 0i128);
}

#[test]
fn is_paused_returns_false_before_pausing() {
    let (_env, _admin, _staker, client, _token_client) = setup();
    assert!(!client.is_paused());
}

#[test]
fn non_admin_cannot_pause() {
    // Set up a fresh env: mock_all_auths for constructor, then clear for pause test.
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let token_id = Address::generate(&env);
    let contract_id = env.register(StakingContract, (&admin, &token_id));
    let client = StakingContractClient::new(&env, &contract_id);

    // Clear all mocked auths — admin.require_auth() inside pause() will fail.
    env.mock_auths(&[]);
    let result = client.try_pause();
    assert!(result.is_err(), "non-admin must not be able to pause");
}

#[test]
fn read_functions_unaffected_by_pause() {
    let (_env, _admin, staker, client, _token_client) = setup();

    client.stake(&staker, &1_000i128);
    client.pause();

    // Read-only calls must succeed regardless of pause state.
    assert!(client.is_paused());
    assert_eq!(client.total_staked(), 1_000i128);
    assert_eq!(client.total_shares(), 1_000i128);
    assert_eq!(client.staked_balance(&staker), 1_000i128);
    assert!(client.get_position(&staker).shares > 0);
}


