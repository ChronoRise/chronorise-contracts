#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, Vec,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn create_token<'a>(env: &'a Env, admin: &Address) -> (Address, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    (sac.address(), StellarAssetClient::new(env, &sac.address()))
}

fn setup() -> (Env, TournamentRewardsContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(TournamentRewardsContract, ());
    let client = TournamentRewardsContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);

    let (token_addr, _) = create_token(&env, &admin);

    (env, client, admin, token_addr)
}

fn fund_player(env: &Env, token: &Address, player: &Address, amount: i128) {
    let sac = StellarAssetClient::new(env, token);
    sac.mint(player, &amount);
}

fn payout_winner_takes_all(env: &Env) -> Vec<u32> {
    let mut v = Vec::new(env);
    v.push_back(10_000_u32);
    v
}

fn payout_60_40(env: &Env) -> Vec<u32> {
    let mut v = Vec::new(env);
    v.push_back(6_000_u32);
    v.push_back(4_000_u32);
    v
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin, _token) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.tournament_count(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let (env, client, _admin, _token) = setup();
    client.initialize(&Address::generate(&env));
}

// ── Tournament creation ───────────────────────────────────────────────────────

#[test]
fn test_create_tournament() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    assert_eq!(id, 0);
    assert_eq!(client.tournament_count(), 1);

    let t = client.get_tournament(&id);
    assert_eq!(t.status, TournamentStatus::Open);
    assert_eq!(t.entry_fee, 100);
    assert_eq!(t.total_pool, 0);
}

#[test]
fn test_create_multiple_tournaments() {
    let (env, client, _admin, token) = setup();
    let id0 = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let id1 = client.create_tournament(&token, &50_i128, &payout_60_40(&env));
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(client.tournament_count(), 2);
}

#[test]
#[should_panic(expected = "payout_bps must not be empty")]
fn test_create_empty_payout_panics() {
    let (env, client, _admin, token) = setup();
    client.create_tournament(&token, &0_i128, &Vec::new(&env));
}

#[test]
#[should_panic(expected = "payout bps exceeds 10000")]
fn test_create_payout_overflow_panics() {
    let (env, client, _admin, token) = setup();
    let mut bad = Vec::new(&env);
    bad.push_back(10_001_u32);
    client.create_tournament(&token, &0_i128, &bad);
}

// ── Entry ─────────────────────────────────────────────────────────────────────

#[test]
fn test_enter_pays_fee() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);

    client.enter(&player, &id);
    assert_eq!(client.entry_fee_paid(&id, &player), 100);
    assert_eq!(client.get_tournament(&id).total_pool, 100);
    assert_eq!(client.entrant_count(&id), 1);
}

#[test]
fn test_enter_free_tournament() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    client.enter(&player, &id);
    assert_eq!(client.entry_fee_paid(&id, &player), 0);
}

#[test]
#[should_panic(expected = "already entered")]
fn test_double_enter_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);
    client.enter(&player, &id);
}

#[test]
#[should_panic(expected = "tournament not open")]
fn test_enter_closed_tournament_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    client.start(&id);
    let player = Address::generate(&env);
    client.enter(&player, &id);
}

// ── Top-up ────────────────────────────────────────────────────────────────────

#[test]
fn test_top_up_adds_to_pool() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let sponsor = Address::generate(&env);
    fund_player(&env, &token, &sponsor, 10_000);
    client.top_up(&sponsor, &id, &5_000_i128);
    assert_eq!(client.get_tournament(&id).total_pool, 5_000);
}

#[test]
#[should_panic(expected = "tournament not active")]
fn test_top_up_finalised_tournament_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    client.enter(&player, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(player);
    client.finalise(&id, &winners);

    let sponsor = Address::generate(&env);
    fund_player(&env, &token, &sponsor, 1_000);
    client.top_up(&sponsor, &id, &1_000_i128);
}

// ── Lifecycle ─────────────────────────────────────────────────────────────────

#[test]
fn test_start_tournament() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    client.start(&id);
    assert_eq!(client.get_tournament(&id).status, TournamentStatus::InProgress);
}

#[test]
#[should_panic(expected = "tournament not open")]
fn test_start_already_started_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    client.start(&id);
    client.start(&id);
}

#[test]
fn test_finalise_tournament() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(player.clone());
    client.finalise(&id, &winners);
    let t = client.get_tournament(&id);
    assert_eq!(t.status, TournamentStatus::Finalised);
    assert_eq!(t.ranked_winners.len(), 1);
}

#[test]
#[should_panic(expected = "more winners than payout slots")]
fn test_finalise_more_winners_than_slots_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    client.enter(&p1, &id);
    client.enter(&p2, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(p1);
    winners.push_back(p2);
    client.finalise(&id, &winners);
}

// ── Claim ─────────────────────────────────────────────────────────────────────

#[test]
fn test_full_lifecycle_winner_claims() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);

    client.enter(&player, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(player.clone());
    client.finalise(&id, &winners);
    client.claim_reward(&player, &id);
    assert!(client.is_claimed(&id, &player));

    // paid 100 entry fee, won 100% of 100 pool back
    let tc = TokenClient::new(&env, &token);
    assert_eq!(tc.balance(&player), 1_000 - 100 + 100);
}

#[test]
fn test_split_payout_60_40() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_60_40(&env));

    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let sponsor = Address::generate(&env);
    fund_player(&env, &token, &sponsor, 10_000);

    client.enter(&p1, &id);
    client.enter(&p2, &id);
    client.top_up(&sponsor, &id, &10_000_i128);
    client.start(&id);

    let mut winners = Vec::new(&env);
    winners.push_back(p1.clone());
    winners.push_back(p2.clone());
    client.finalise(&id, &winners);

    client.claim_reward(&p1, &id);
    client.claim_reward(&p2, &id);

    let tc = TokenClient::new(&env, &token);
    assert_eq!(tc.balance(&p1), 6_000);
    assert_eq!(tc.balance(&p2), 4_000);
}

#[test]
#[should_panic(expected = "already claimed")]
fn test_double_claim_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(player.clone());
    client.finalise(&id, &winners);
    client.claim_reward(&player, &id);
    client.claim_reward(&player, &id);
}

#[test]
#[should_panic(expected = "not a winner")]
fn test_non_winner_cannot_claim() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &0_i128, &payout_winner_takes_all(&env));
    let winner = Address::generate(&env);
    let loser = Address::generate(&env);
    client.enter(&winner, &id);
    client.start(&id);
    let mut winners = Vec::new(&env);
    winners.push_back(winner);
    client.finalise(&id, &winners);
    client.claim_reward(&loser, &id);
}

// ── Cancel / Refund ───────────────────────────────────────────────────────────

#[test]
fn test_cancel_and_refund() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &200_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);

    let balance_before = TokenClient::new(&env, &token).balance(&player);
    client.cancel(&id);
    client.refund(&player, &id);

    let balance_after = TokenClient::new(&env, &token).balance(&player);
    assert_eq!(balance_after, balance_before + 200);
    assert_eq!(client.entry_fee_paid(&id, &player), 0);
}

#[test]
#[should_panic(expected = "tournament not cancelled")]
fn test_refund_on_open_tournament_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);
    client.refund(&player, &id);
}

#[test]
#[should_panic(expected = "nothing to refund")]
fn test_double_refund_panics() {
    let (env, client, _admin, token) = setup();
    let id = client.create_tournament(&token, &100_i128, &payout_winner_takes_all(&env));
    let player = Address::generate(&env);
    fund_player(&env, &token, &player, 1_000);
    client.enter(&player, &id);
    client.cancel(&id);
    client.refund(&player, &id);
    client.refund(&player, &id);
}
