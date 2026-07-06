#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env, String};

fn setup() -> (Env, PlayerRegistryContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(PlayerRegistryContract, ());
    let client = PlayerRegistryContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, admin)
}

fn s(env: &Env, text: &str) -> String {
    String::from_str(env, text)
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.player_count(), 0);
    assert_eq!(client.player_list().len(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    client.initialize(&other);
}

// ── Registration ──────────────────────────────────────────────────────────────

#[test]
fn test_register_and_get_player() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Alice"));

    let profile = client.get_player(&player);
    assert_eq!(profile.username, s(&env, "Alice"));
    assert_eq!(profile.wins, 0);
    assert_eq!(profile.tournaments_played, 0);
    assert_eq!(profile.reputation, 0);
    assert!(profile.active);
}

#[test]
fn test_player_count_increments() {
    let (env, client, _admin) = setup();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    client.register_player(&p1, &s(&env, "P1"));
    client.register_player(&p2, &s(&env, "P2"));
    assert_eq!(client.player_count(), 2);
}

#[test]
fn test_player_list_grows() {
    let (env, client, _admin) = setup();
    assert_eq!(client.player_list().len(), 0);
    let p = Address::generate(&env);
    client.register_player(&p, &s(&env, "Solo"));
    assert_eq!(client.player_list().len(), 1);
}

#[test]
fn test_is_registered_true_and_false() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    assert!(!client.is_registered(&player));
    client.register_player(&player, &s(&env, "Bob"));
    assert!(client.is_registered(&player));
}

#[test]
#[should_panic(expected = "player already registered")]
fn test_register_duplicate_wallet_panics() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Alice"));
    client.register_player(&player, &s(&env, "Alice2"));
}

#[test]
#[should_panic(expected = "username taken")]
fn test_register_duplicate_username_panics() {
    let (env, client, _admin) = setup();
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    client.register_player(&p1, &s(&env, "Same"));
    client.register_player(&p2, &s(&env, "Same")); // same username — should panic
}

#[test]
fn test_lookup_username() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Carol"));
    let found = client.lookup_username(&s(&env, "Carol"));
    assert_eq!(found, player);
}

#[test]
#[should_panic(expected = "username not found")]
fn test_lookup_nonexistent_username_panics() {
    let (env, client, _admin) = setup();
    client.lookup_username(&s(&env, "ghost"));
}

// ── Stats updates ─────────────────────────────────────────────────────────────

#[test]
fn test_record_win() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Carol"));
    client.record_win(&player);
    let p = client.get_player(&player);
    assert_eq!(p.wins, 1);
    assert_eq!(p.tournaments_played, 1);
}

#[test]
fn test_record_multiple_wins() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Champ"));
    client.record_win(&player);
    client.record_win(&player);
    client.record_win(&player);
    let p = client.get_player(&player);
    assert_eq!(p.wins, 3);
    assert_eq!(p.tournaments_played, 3);
}

#[test]
fn test_record_participation() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Dave"));
    client.record_participation(&player);
    let p = client.get_player(&player);
    assert_eq!(p.wins, 0);
    assert_eq!(p.tournaments_played, 1);
}

#[test]
fn test_add_reputation() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Eve"));
    client.add_reputation(&player, &100_i128);
    client.add_reputation(&player, &50_i128);
    assert_eq!(client.get_player(&player).reputation, 150);
}

#[test]
fn test_add_game_no_duplicates() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Gamer"));
    client.add_game(&player, &1_u32);
    client.add_game(&player, &2_u32);
    client.add_game(&player, &1_u32); // duplicate — should be ignored
    let p = client.get_player(&player);
    assert_eq!(p.games.len(), 2);
}

// ── Claimed rewards ───────────────────────────────────────────────────────────

#[test]
fn test_add_claimed_reward_no_duplicates() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Reward"));
    client.add_claimed_reward(&player, &10_u32);
    client.add_claimed_reward(&player, &20_u32);
    client.add_claimed_reward(&player, &10_u32); // dup — ignored
    let claimed = client.claimed_rewards(&player);
    assert_eq!(claimed.len(), 2);
}

#[test]
fn test_claimed_rewards_empty_for_new_player() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Fresh"));
    assert_eq!(client.claimed_rewards(&player).len(), 0);
}

// ── Badges ────────────────────────────────────────────────────────────────────

#[test]
fn test_add_badge_no_duplicates() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "BadgeHunter"));
    client.add_badge(&player, &0_u32);
    client.add_badge(&player, &1_u32);
    client.add_badge(&player, &0_u32); // dup — ignored
    assert_eq!(client.badge_list(&player).len(), 2);
}

#[test]
fn test_remove_badge() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Collector"));
    client.add_badge(&player, &5_u32);
    client.add_badge(&player, &6_u32);
    client.remove_badge(&player, &5_u32);
    let badges = client.badge_list(&player);
    assert_eq!(badges.len(), 1);
    assert_eq!(badges.get(0).unwrap(), 6_u32);
}

// ── Ban / Unban ───────────────────────────────────────────────────────────────

#[test]
fn test_set_active_ban_and_unban() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Eve"));
    client.set_active(&player, &false);
    assert!(!client.get_player(&player).active);

    client.set_active(&player, &true);
    assert!(client.get_player(&player).active);
}

#[test]
#[should_panic(expected = "player is inactive")]
fn test_banned_player_cannot_record_win() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    client.register_player(&player, &s(&env, "Banned"));
    client.set_active(&player, &false);
    client.record_win(&player); // should panic
}

// ── Non-existent player queries ───────────────────────────────────────────────

#[test]
#[should_panic(expected = "player not found")]
fn test_get_unregistered_player_panics() {
    let (env, client, _admin) = setup();
    let unknown = Address::generate(&env);
    client.get_player(&unknown);
}
