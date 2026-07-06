#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn create_token<'a>(env: &'a Env, admin: &Address) -> (Address, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    (sac.address(), StellarAssetClient::new(env, &sac.address()))
}

fn setup() -> (Env, TreasuryContractClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TreasuryContract, ());
    let client = TreasuryContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (token_addr, sac) = create_token(&env, &admin);
    let depositor = Address::generate(&env);
    sac.mint(&depositor, &1_000_000);
    client.add_supported_token(&token_addr);

    (env, client, admin, token_addr, depositor)
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin, _token, _depositor) = setup();
    assert_eq!(client.admin(), admin);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let (env, client, _admin, _token, _depositor) = setup();
    client.initialize(&Address::generate(&env));
}

// ── Token management ──────────────────────────────────────────────────────────

#[test]
fn test_add_supported_token() {
    let (_env, client, _admin, token, _depositor) = setup();
    let tokens = client.supported_tokens();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens.get(0).unwrap(), token);
}

#[test]
#[should_panic(expected = "token already supported")]
fn test_add_duplicate_token_panics() {
    let (_env, client, _admin, token, _depositor) = setup();
    client.add_supported_token(&token);
}

#[test]
fn test_multiple_supported_tokens() {
    let (env, client, admin, _token, _depositor) = setup();
    let sac2 = env.register_stellar_asset_contract_v2(admin.clone());
    let token2 = sac2.address();
    client.add_supported_token(&token2);
    assert_eq!(client.supported_tokens().len(), 2);
}

// ── Deposit ───────────────────────────────────────────────────────────────────

#[test]
fn test_deposit_updates_stats() {
    let (_env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &500_000);
    assert_eq!(client.total_deposited(&token), 500_000);
    assert_eq!(client.total_disbursed(&token), 0);
}

#[test]
fn test_multiple_deposits_accumulate() {
    let (_env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &200_000);
    client.deposit(&depositor, &token, &300_000);
    assert_eq!(client.total_deposited(&token), 500_000);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_deposit_zero_panics() {
    let (_env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &0);
}

#[test]
#[should_panic(expected = "unsupported token")]
fn test_deposit_unsupported_token_panics() {
    let (env, client, _admin, _token, depositor) = setup();
    let fake = Address::generate(&env);
    client.deposit(&depositor, &fake, &100);
}

// ── Disburse ──────────────────────────────────────────────────────────────────

#[test]
fn test_disburse_updates_stats_and_transfers() {
    let (env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &500_000);

    let recipient = Address::generate(&env);
    client.disburse(&token, &recipient, &200_000);

    assert_eq!(client.total_disbursed(&token), 200_000);

    let token_client = TokenClient::new(&env, &token);
    assert_eq!(token_client.balance(&recipient), 200_000);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_disburse_zero_panics() {
    let (_env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &100_000);
    let r = Address::generate(&_env);
    client.disburse(&token, &r, &0);
}

#[test]
#[should_panic(expected = "unsupported token")]
fn test_disburse_unsupported_token_panics() {
    let (env, client, _admin, _token, _depositor) = setup();
    let fake = Address::generate(&env);
    let r = Address::generate(&env);
    client.disburse(&fake, &r, &100);
}

// ── Token stats ───────────────────────────────────────────────────────────────

#[test]
fn test_get_token_stats() {
    let (env, client, _admin, token, depositor) = setup();
    client.deposit(&depositor, &token, &600_000);
    let r = Address::generate(&env);
    client.disburse(&token, &r, &100_000);

    let stats = client.get_token_stats(&token);
    assert_eq!(stats.total_deposited, 600_000);
    assert_eq!(stats.total_disbursed, 100_000);
    assert_eq!(stats.fees_collected, 0);
}

#[test]
fn test_stats_zero_for_unknown_token() {
    let (env, client, _admin, _token, _depositor) = setup();
    let unknown = Address::generate(&env);
    let stats = client.get_token_stats(&unknown);
    assert_eq!(stats.total_deposited, 0);
    assert_eq!(stats.total_disbursed, 0);
}

// ── Spender management ────────────────────────────────────────────────────────

#[test]
fn test_add_and_check_spender() {
    let (env, client, _admin, _token, _depositor) = setup();
    let spender = Address::generate(&env);
    assert!(!client.is_spender(&spender));
    client.add_spender(&spender);
    assert!(client.is_spender(&spender));
}

#[test]
fn test_remove_spender() {
    let (env, client, _admin, _token, _depositor) = setup();
    let spender = Address::generate(&env);
    client.add_spender(&spender);
    client.remove_spender(&spender);
    assert!(!client.is_spender(&spender));
}
