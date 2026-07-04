#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, AchievementRegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(AchievementRegistryContract, ());
    let client = AchievementRegistryContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    (env, client, admin)
}

fn make_str(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.achievement_count(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_env, client, admin) = setup();
    client.initialize(&admin);
}

#[test]
fn test_register_achievement() {
    let (env, client, _admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "First Deposit"),
        &make_str(&env, "Made your first deposit into the reward pool"),
    );

    assert_eq!(id, 0);
    assert_eq!(client.achievement_count(), 1);

    let def = client.get_achievement(&0);
    assert_eq!(def.name, make_str(&env, "First Deposit"));
    assert!(def.active);
}

#[test]
fn test_register_multiple_achievements_increments_id() {
    let (env, client, _admin) = setup();

    let id0 = client.register_achievement(
        &make_str(&env, "Ach 0"),
        &make_str(&env, "desc 0"),
    );
    let id1 = client.register_achievement(
        &make_str(&env, "Ach 1"),
        &make_str(&env, "desc 1"),
    );

    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(client.achievement_count(), 2);
}

#[test]
fn test_deactivate_achievement() {
    let (env, client, _admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Temporary"),
        &make_str(&env, "Will be deactivated"),
    );

    client.deactivate_achievement(&id);
    let def = client.get_achievement(&id);
    assert!(!def.active);
}

#[test]
fn test_award_and_has_achievement() {
    let (env, client, admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Early Bird"),
        &make_str(&env, "Joined in the first week"),
    );

    let user = Address::generate(&env);
    client.award(&admin, &user, &id);

    assert!(client.has_achievement(&user, &id));
    let awards = client.get_user_awards(&user);
    assert_eq!(awards.len(), 1);
    assert_eq!(awards.get(0).unwrap().achievement_id, id);
}

#[test]
#[should_panic(expected = "already awarded")]
fn test_duplicate_award_panics() {
    let (env, client, admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Unique"),
        &make_str(&env, "Can only be awarded once"),
    );

    let user = Address::generate(&env);
    client.award(&admin, &user, &id);
    client.award(&admin, &user, &id); // duplicate — should panic
}

#[test]
#[should_panic(expected = "achievement is inactive")]
fn test_award_inactive_achievement_panics() {
    let (env, client, admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Retired"),
        &make_str(&env, "No longer active"),
    );
    client.deactivate_achievement(&id);

    let user = Address::generate(&env);
    client.award(&admin, &user, &id);
}

#[test]
fn test_awarder_can_award() {
    let (env, client, _admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Delegated"),
        &make_str(&env, "Awarded by a delegate"),
    );

    let awarder = Address::generate(&env);
    client.add_awarder(&awarder);

    let user = Address::generate(&env);
    client.award(&awarder, &user, &id);

    assert!(client.has_achievement(&user, &id));
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_unauthorized_awarder_panics() {
    let (env, client, _admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Guarded"),
        &make_str(&env, "Restricted"),
    );

    let rando = Address::generate(&env);
    let user = Address::generate(&env);
    client.award(&rando, &user, &id);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_remove_awarder_revokes_access() {
    let (env, client, _admin) = setup();

    let id = client.register_achievement(
        &make_str(&env, "Temp Awarder"),
        &make_str(&env, "desc"),
    );

    let awarder = Address::generate(&env);
    client.add_awarder(&awarder);
    client.remove_awarder(&awarder);

    // after removal the awarder should be rejected
    let user = Address::generate(&env);
    // This should panic with "unauthorized"
    client.award(&awarder, &user, &id);
}
