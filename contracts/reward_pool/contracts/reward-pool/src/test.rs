#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, token, Address, Env};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn create_token<'a>(env: &'a Env, admin: &Address) -> (Address, token::StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let client = token::StellarAssetClient::new(env, &sac.address());
    (sac.address(), client)
}

fn setup() -> (Env, RewardPoolContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let (token_id, token_admin) = create_token(&env, &admin);

    token_admin.mint(&user, &10_000);

    let contract_id = env.register(RewardPoolContract, ());
    let client = RewardPoolContractClient::new(&env, &contract_id);
    client.initialize(&admin, &token_id);

    (env, client, admin, user, token_id)
}

// ── Init ──────────────────────────────────────────────────────────────────────

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
    client.initialize(&admin, &token_id);
}

// ── Deposit ───────────────────────────────────────────────────────────────────

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
#[should_panic(expected = "amount must be positive")]
fn test_deposit_zero_panics() {
    let (_env, client, _admin, user, _token_id) = setup();
    client.deposit(&user, &0);
}

// ── Withdraw ──────────────────────────────────────────────────────────────────

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

// ── Distribute ────────────────────────────────────────────────────────────────

#[test]
fn test_distribute() {
    let (env, client, _admin, user, token_id) = setup();
    client.deposit(&user, &1_000);

    let recipient = Address::generate(&env);
    client.distribute(&recipient, &300);

    assert_eq!(client.total_deposited(), 700);

    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&recipient), 300);
}

// ── Depositors list ───────────────────────────────────────────────────────────

#[test]
fn test_depositors_list() {
    let (env, client, _admin, user, token_id) = setup();

    let user2 = Address::generate(&env);
    let sac = token::StellarAssetClient::new(&env, &token_id);
    sac.mint(&user2, &5_000);

    client.deposit(&user, &100);
    client.deposit(&user2, &200);

    let depositors = client.depositors();
    assert_eq!(depositors.len(), 2);
}

// ── Per-achievement reward allocation ────────────────────────────────────────

#[test]
fn test_set_and_read_achievement_reward() {
    let (_env, client, _admin, user, _token) = setup();
    client.deposit(&user, &5_000);
    client.set_achievement_reward(&0_u32, &1_000_i128);
    assert_eq!(client.achievement_reward_amount(&0_u32), 1_000);
}

#[test]
fn test_claim_achievement_reward_success() {
    let (env, client, _admin, user, token_id) = setup();
    client.deposit(&user, &5_000);
    client.set_achievement_reward(&42_u32, &500_i128);

    let player = Address::generate(&env);
    client.claim_achievement_reward(&player, &42_u32, &500_i128);

    // Pool balance decremented.
    assert_eq!(client.total_deposited(), 4_500);

    // Player actually received the tokens.
    let token_client = token::Client::new(&env, &token_id);
    assert_eq!(token_client.balance(&player), 500);

    // Claim flag is set.
    assert!(client.is_achievement_claimed(&player, &42_u32));
}

#[test]
#[should_panic(expected = "achievement reward already claimed")]
fn test_double_claim_achievement_reward_panics() {
    let (env, client, _admin, user, _token) = setup();
    client.deposit(&user, &5_000);
    client.set_achievement_reward(&1_u32, &200_i128);

    let player = Address::generate(&env);
    client.claim_achievement_reward(&player, &1_u32, &200_i128);
    // Second claim for the same player + achievement must panic.
    client.claim_achievement_reward(&player, &1_u32, &200_i128);
}

#[test]
fn test_different_players_can_each_claim_same_achievement() {
    let (env, client, _admin, user, _token) = setup();
    client.deposit(&user, &5_000);
    client.set_achievement_reward(&7_u32, &100_i128);

    let player_a = Address::generate(&env);
    let player_b = Address::generate(&env);

    // Both players independently claim the same achievement.
    client.claim_achievement_reward(&player_a, &7_u32, &100_i128);
    client.claim_achievement_reward(&player_b, &7_u32, &100_i128);

    assert!(client.is_achievement_claimed(&player_a, &7_u32));
    assert!(client.is_achievement_claimed(&player_b, &7_u32));
    assert_eq!(client.total_deposited(), 4_800);
}

#[test]
fn test_same_player_can_claim_different_achievements() {
    let (env, client, _admin, user, _token) = setup();
    client.deposit(&user, &5_000);
    client.set_achievement_reward(&10_u32, &200_i128);
    client.set_achievement_reward(&11_u32, &300_i128);

    let player = Address::generate(&env);
    client.claim_achievement_reward(&player, &10_u32, &200_i128);
    client.claim_achievement_reward(&player, &11_u32, &300_i128);

    assert!(client.is_achievement_claimed(&player, &10_u32));
    assert!(client.is_achievement_claimed(&player, &11_u32));
    assert_eq!(client.total_deposited(), 4_500);
}

#[test]
#[should_panic(expected = "insufficient pool balance")]
fn test_claim_more_than_pool_panics() {
    let (env, client, _admin, user, _token) = setup();
    client.deposit(&user, &100); // Only 100 in the pool.

    let player = Address::generate(&env);
    client.claim_achievement_reward(&player, &99_u32, &200_i128); // wants 200 — should panic
}

#[test]
fn test_is_achievement_claimed_returns_false_before_claim() {
    let (env, client, _admin, _user, _token) = setup();
    let player = Address::generate(&env);
    assert!(!client.is_achievement_claimed(&player, &0_u32));
}
