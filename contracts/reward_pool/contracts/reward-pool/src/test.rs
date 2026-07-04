#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env};

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Deploy a minimal SAC (Stellar Asset Contract) token for testing and return
/// its address together with an admin client.
fn create_token<'a>(
    env: &Env,
    admin: &Address,
) -> (Address, token::StellarAssetClient<'a>) {
    let contract_address = env.register_stellar_asset_contract_v2(admin.clone());
    let client = token::StellarAssetClient::new(env, &contract_address.address());
    (contract_address.address(), client)
}

fn setup() -> (Env, RewardPoolContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token_id, token_admin) = create_token(&env, &admin);

    // Mint some tokens to the user so they can deposit.
    token_admin.mint(&user, &10_000);

    let contract_id = env.register(RewardPoolContract, ());
    let client = RewardPoolContractClient::new(&env, &contract_id);

    client.initialize(&admin, &token_id);

    (env, client, admin, user, token_id)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin, _user, token_id) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.reward_token(), token_id);
    assert_eq!(client.total_deposited(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_env, client, admin, _user, token_id) = setup();
    // Second call must panic.
    client.initialize(&admin, &token_id);
}

#[test]
fn test_deposit_and_balance() {
    let (_env, client, _admin, user, _token_id) = setup();

    client.deposit(&user, &1_000);

    assert_eq!(client.balance_of(&user), 1_000);
    assert_eq!(client.total_deposited(), 1_000);
}

#[test]
fn test_multiple_deposits_accumulate() {
    let (_env, client, _admin, user, _token_id) = setup();

    client.deposit(&user, &500);
    client.deposit(&user, &300);

    assert_eq!(client.balance_of(&user), 800);
    assert_eq!(client.total_deposited(), 800);
}

#[test]
fn test_withdraw() {
    let (_env, client, _admin, user, _token_id) = setup();

    client.deposit(&user, &1_000);
    client.withdraw(&user, &400);

    assert_eq!(client.balance_of(&user), 600);
    assert_eq!(client.total_deposited(), 600);
}

#[test]
#[should_panic(expected = "insufficient balance")]
fn test_withdraw_more_than_balance_panics() {
    let (_env, client, _admin, user, _token_id) = setup();

    client.deposit(&user, &100);
    client.withdraw(&user, &200);
}

#[test]
fn test_distribute() {
    let (env, client, _admin, user, token_id) = setup();

    client.deposit(&user, &1_000);

    let recipient = Address::generate(&env);
    client.distribute(&recipient, &300);

    assert_eq!(client.total_deposited(), 700);

    // Verify the recipient actually received the tokens.
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&recipient), 300);
}

#[test]
fn test_depositors_list() {
    let (env, client, _admin, user, _token_id) = setup();

    let user2 = Address::generate(&env);
    // Mint tokens to user2 as well.
    // (mock_all_auths covers the SAC mint auth)
    let token_id = client.reward_token();
    let sac = token::StellarAssetClient::new(&env, &token_id);
    sac.mint(&user2, &5_000);

    client.deposit(&user, &100);
    client.deposit(&user2, &200);

    let depositors = client.depositors();
    assert_eq!(depositors.len(), 2);
}
