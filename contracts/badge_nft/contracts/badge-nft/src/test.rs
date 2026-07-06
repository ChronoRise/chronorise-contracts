#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, BadgeNftContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(BadgeNftContract, ());
    let client = BadgeNftContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.badge_type_count(), 0);
    assert_eq!(client.total_supply(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let (env, client, _admin) = setup();
    client.initialize(&Address::generate(&env));
}

// ── Badge types ───────────────────────────────────────────────────────────────

#[test]
fn test_create_badge_type() {
    let (env, client, _admin) = setup();
    let id = client.create_badge_type(
        &String::from_str(&env, "Champion"),
        &String::from_str(&env, "ipfs://champion"),
    );
    assert_eq!(id, 0);
    let bt = client.get_badge_type(&id);
    assert!(bt.active);
    assert_eq!(bt.name, String::from_str(&env, "Champion"));
    assert_eq!(client.badge_type_count(), 1);
}

#[test]
fn test_multiple_badge_types_increment_id() {
    let (env, client, _admin) = setup();
    let id0 = client.create_badge_type(
        &String::from_str(&env, "Rookie"),
        &String::from_str(&env, "ipfs://rookie"),
    );
    let id1 = client.create_badge_type(
        &String::from_str(&env, "Legend"),
        &String::from_str(&env, "ipfs://legend"),
    );
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(client.badge_type_count(), 2);
}

#[test]
fn test_deactivate_badge_type() {
    let (env, client, _admin) = setup();
    let id = client.create_badge_type(
        &String::from_str(&env, "Veteran"),
        &String::from_str(&env, "ipfs://veteran"),
    );
    client.deactivate_badge_type(&id);
    assert!(!client.get_badge_type(&id).active);
}

// ── Minting ───────────────────────────────────────────────────────────────────

#[test]
fn test_mint_and_get_token() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Rookie"),
        &String::from_str(&env, "ipfs://rookie"),
    );
    let recipient = Address::generate(&env);
    let token_id = client.mint(&admin, &recipient, &badge_type_id);
    assert_eq!(token_id, 0);

    let badge = client.get_token(&token_id);
    assert_eq!(badge.owner, recipient);
    assert_eq!(badge.badge_type_id, badge_type_id);
    assert!(badge.alive);
    assert_eq!(client.total_supply(), 1);
}

#[test]
fn test_tokens_of_returns_all_owned() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Pro"),
        &String::from_str(&env, "ipfs://pro"),
    );
    let recipient = Address::generate(&env);
    client.mint(&admin, &recipient, &badge_type_id);
    client.mint(&admin, &recipient, &badge_type_id);
    assert_eq!(client.tokens_of(&recipient).len(), 2);
}

#[test]
fn test_authorised_minter_can_mint() {
    let (env, client, _admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Elite"),
        &String::from_str(&env, "ipfs://elite"),
    );
    let minter = Address::generate(&env);
    client.add_minter(&minter);

    let recipient = Address::generate(&env);
    let token_id = client.mint(&minter, &recipient, &badge_type_id);
    assert_eq!(client.get_token(&token_id).owner, recipient);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_unknown_caller_cannot_mint() {
    let (env, client, _admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Elite"),
        &String::from_str(&env, "ipfs://elite"),
    );
    let rando = Address::generate(&env);
    let recipient = Address::generate(&env);
    client.mint(&rando, &recipient, &badge_type_id);
}

#[test]
#[should_panic(expected = "badge type inactive")]
fn test_mint_inactive_badge_type_panics() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Retired"),
        &String::from_str(&env, "ipfs://retired"),
    );
    client.deactivate_badge_type(&badge_type_id);
    let recipient = Address::generate(&env);
    client.mint(&admin, &recipient, &badge_type_id);
}

// ── Soulbound enforcement ─────────────────────────────────────────────────────

/// Verify there is no `transfer` function on the contract client.
/// The absence of a `transfer` method on `BadgeNftClient` is the compile-time
/// guarantee. This test documents the design intent and verifies that mint
/// does NOT change ownership — only the original recipient owns the badge.
#[test]
fn test_badge_is_soulbound_owner_never_changes() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Soulbound"),
        &String::from_str(&env, "ipfs://soulbound"),
    );
    let owner = Address::generate(&env);
    let token_id = client.mint(&admin, &owner, &badge_type_id);

    // The owner field is set at mint and there is no way to change it.
    let badge = client.get_token(&token_id);
    assert_eq!(badge.owner, owner);

    // tokens_of still returns the token for the original owner.
    assert_eq!(client.tokens_of(&owner).len(), 1);
}

/// Another address should have zero tokens even after the owner mints.
#[test]
fn test_unrelated_address_has_no_tokens() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Unique"),
        &String::from_str(&env, "ipfs://unique"),
    );
    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    client.mint(&admin, &owner, &badge_type_id);

    assert_eq!(client.tokens_of(&other).len(), 0);
}

// ── Burn ──────────────────────────────────────────────────────────────────────

#[test]
fn test_owner_can_burn() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Burnout"),
        &String::from_str(&env, "ipfs://burnout"),
    );
    let owner = Address::generate(&env);
    let token_id = client.mint(&admin, &owner, &badge_type_id);

    client.burn(&owner, &token_id);

    let badge = client.get_token(&token_id);
    assert!(!badge.alive);
    assert_eq!(client.tokens_of(&owner).len(), 0);
}

#[test]
#[should_panic(expected = "already burned")]
fn test_double_burn_panics() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Gone"),
        &String::from_str(&env, "ipfs://gone"),
    );
    let owner = Address::generate(&env);
    let token_id = client.mint(&admin, &owner, &badge_type_id);
    client.burn(&owner, &token_id);
    client.burn(&owner, &token_id); // second burn must panic
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_non_owner_cannot_burn() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Protected"),
        &String::from_str(&env, "ipfs://protected"),
    );
    let owner = Address::generate(&env);
    let thief = Address::generate(&env);
    let token_id = client.mint(&admin, &owner, &badge_type_id);
    client.burn(&thief, &token_id); // should panic
}

// ── Minter management ─────────────────────────────────────────────────────────

#[test]
fn test_add_and_remove_minter() {
    let (env, client, _admin) = setup();
    let minter = Address::generate(&env);
    client.add_minter(&minter);
    client.remove_minter(&minter);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_removed_minter_cannot_mint() {
    let (env, client, _admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Revoked"),
        &String::from_str(&env, "ipfs://revoked"),
    );
    let minter = Address::generate(&env);
    client.add_minter(&minter);
    client.remove_minter(&minter);

    let recipient = Address::generate(&env);
    client.mint(&minter, &recipient, &badge_type_id); // should panic
}

// ── Edge cases ────────────────────────────────────────────────────────────────

#[test]
fn test_total_minted_alias() {
    let (env, client, admin) = setup();
    let badge_type_id = client.create_badge_type(
        &String::from_str(&env, "Alias"),
        &String::from_str(&env, "ipfs://alias"),
    );
    let r = Address::generate(&env);
    client.mint(&admin, &r, &badge_type_id);
    assert_eq!(client.total_minted(), 1);
    assert_eq!(client.total_supply(), 1);
}
