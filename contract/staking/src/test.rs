#[cfg(test)]
mod tests {
    use crate::{StakingContract, StakingContractClient};
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup() -> (Env, StakingContractClient<'static>, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(StakingContract, ());
        let client = StakingContractClient::new(&env, &contract_id);
        let user = Address::generate(&env);
        client.init(&10_000);
        (env, client, user)
    }

    #[test]
    fn test_stake_at_min() {
        let (_env, client, user) = setup();
        client.stake(&user, &10_000_000);
        let stake = client.get_stake(&user);
        assert!(stake.is_some());
        assert_eq!(stake.unwrap().amount, 10_000_000);
    }

    #[test]
    fn test_stake_below_min() {
        let (_env, client, user) = setup();
        let result = client.try_stake(&user, &9_999_999);
        assert!(result.is_err());
    }

    #[test]
    fn test_staker_count_increments() {
        let (env, client, user) = setup();
        let user2 = Address::generate(&env);
        client.stake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 1);
        client.stake(&user2, &10_000_000);
        assert_eq!(client.get_staker_count(), 2);
    }

    #[test]
    fn test_staker_cap_reached() {
        let (env, _client, _user) = setup();
        let env2 = Env::default();
        env2.mock_all_auths();
        let contract_id = env2.register(StakingContract, ());
        let client = StakingContractClient::new(&env2, &contract_id);
        client.init(&2);
        let user1 = Address::generate(&env2);
        let user2 = Address::generate(&env2);
        let user3 = Address::generate(&env2);
        client.stake(&user1, &10_000_000);
        client.stake(&user2, &10_000_000);
        let result = client.try_stake(&user3, &10_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_staker_count_decrement_on_full_unstake() {
        let (_env, client, user) = setup();
        client.stake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 1);
        client.unstake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 0);
    }

    #[test]
    fn test_additional_stake_from_existing_staker() {
        let (env, client, user) = setup();
        client.stake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 1);
        client.stake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 1);
        let stake = client.get_stake(&user).unwrap();
        assert_eq!(stake.amount, 20_000_000);
    }

    #[test]
    fn test_partial_unstake_keeps_staker_count() {
        let (_env, client, user) = setup();
        client.stake(&user, &20_000_000);
        assert_eq!(client.get_staker_count(), 1);
        client.unstake(&user, &10_000_000);
        assert_eq!(client.get_staker_count(), 1);
        let stake = client.get_stake(&user).unwrap();
        assert_eq!(stake.amount, 10_000_000);
    }

    #[test]
    fn test_unstake_without_stake_fails() {
        let (_env, client, user) = setup();
        let result = client.try_unstake(&user, &10_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_unstake_more_than_staked_fails() {
        let (_env, client, user) = setup();
        client.stake(&user, &10_000_000);
        let result = client.try_unstake(&user, &20_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_staker() {
        let (env, client, user) = setup();
        let non_staker = Address::generate(&env);
        assert!(!client.is_staker(&user));
        client.stake(&user, &10_000_000);
        assert!(client.is_staker(&user));
        assert!(!client.is_staker(&non_staker));
    }

    #[test]
    fn test_min_stake_constant() {
        assert_eq!(crate::StakingContract::min_stake(), 10_000_000);
    }

    #[test]
    fn test_storage_deposit_constant() {
        assert_eq!(crate::StakingContract::storage_deposit(), 10_000_000);
    }
}
