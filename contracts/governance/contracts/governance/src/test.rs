#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{Client as TokenClient, StellarAssetClient},
    Address, Env, String,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn create_token<'a>(env: &Env, admin: &Address) -> (Address, StellarAssetClient<'a>) {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let client = StellarAssetClient::new(env, &sac.address());
    (sac.address(), client)
}

fn setup() -> (
    Env,
    GovernanceContractClient<'static>,
    Address,   // admin (also acts as proposer)
    Address,   // gov_token
) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let (gov_token, _sac) = create_token(&env, &admin);

    let contract_id = env.register(GovernanceContract, ());
    let client = GovernanceContractClient::new(&env, &contract_id);

    // quorum=100, approval_bps=5000 (50%), voting_period=100 ledgers
    client.initialize(&admin, &gov_token, &100_i128, &5000_u32, &100_u32);

    (env, client, admin, gov_token)
}

fn mint_tokens(env: &Env, token: &Address, admin: &Address, recipient: &Address, amount: i128) {
    let sac = StellarAssetClient::new(env, token);
    let _ = admin; // required by borrow checker but mock_all_auths handles auth
    sac.mint(recipient, &amount);
}

fn make_str(env: &Env, s: &str) -> String {
    String::from_str(env, s)
}

fn advance_past_voting(env: &Env) {
    env.ledger().set_sequence_number(env.ledger().sequence() + 102);
}

// ── Init ──────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin, _token) = setup();
    assert_eq!(client.admin(), admin);
    let (quorum, bps, period) = client.config();
    assert_eq!(quorum, 100);
    assert_eq!(bps, 5000);
    assert_eq!(period, 100);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let (env, client, _admin, gov_token) = setup();
    let other = Address::generate(&env);
    client.initialize(&other, &gov_token, &100_i128, &5000_u32, &100_u32);
}

// ── Proposals ─────────────────────────────────────────────────────────────────

#[test]
fn test_propose() {
    let (env, client, proposer, _token) = setup();
    let id = client.propose(
        &proposer,
        &make_str(&env, "Raise fees"),
        &make_str(&env, "Proposal to increase protocol fees"),
    );
    assert_eq!(id, 0);
    assert_eq!(client.proposal_count(), 1);

    let proposal = client.get_proposal(&id);
    assert_eq!(proposal.status, ProposalStatus::Active);
    assert_eq!(proposal.votes_for, 0);
    assert_eq!(proposal.votes_against, 0);
}

#[test]
fn test_multiple_proposals_increment_id() {
    let (env, client, proposer, _token) = setup();
    let id0 = client.propose(
        &proposer,
        &make_str(&env, "Proposal A"),
        &make_str(&env, "desc"),
    );
    let id1 = client.propose(
        &proposer,
        &make_str(&env, "Proposal B"),
        &make_str(&env, "desc"),
    );
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(client.proposal_count(), 2);
}

// ── Voting with on-chain balance ──────────────────────────────────────────────

#[test]
fn test_vote_uses_real_token_balance() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 200);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Add feature"),
        &make_str(&env, "Details"),
    );

    client.vote(&voter, &id, &true);

    assert!(client.has_voted(&id, &voter));
    assert_eq!(client.vote_weight(&id, &voter), 200);
    assert_eq!(client.get_proposal(&id).votes_for, 200);
}

#[test]
fn test_vote_against_uses_real_token_balance() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 50);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Test Against"),
        &make_str(&env, "desc"),
    );
    client.vote(&voter, &id, &false);

    assert_eq!(client.get_proposal(&id).votes_against, 50);
    assert_eq!(client.get_proposal(&id).votes_for, 0);
}

#[test]
fn test_multiple_voters_accumulate() {
    let (env, client, proposer, gov_token) = setup();

    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter_a, 300);
    mint_tokens(&env, &gov_token, &proposer, &voter_b, 100);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Multi-voter"),
        &make_str(&env, "desc"),
    );

    client.vote(&voter_a, &id, &true);
    client.vote(&voter_b, &id, &false);

    let p = client.get_proposal(&id);
    assert_eq!(p.votes_for, 300);
    assert_eq!(p.votes_against, 100);
}

#[test]
#[should_panic(expected = "already voted")]
fn test_double_vote_panics() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 100);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Test"),
        &make_str(&env, "desc"),
    );
    client.vote(&voter, &id, &true);
    client.vote(&voter, &id, &false); // should panic
}

#[test]
#[should_panic(expected = "zero token balance")]
fn test_voter_with_no_tokens_is_rejected() {
    let (env, client, proposer, _token) = setup();
    let zero_voter = Address::generate(&env);
    // No tokens minted → balance is 0.

    let id = client.propose(
        &proposer,
        &make_str(&env, "Zero balance"),
        &make_str(&env, "desc"),
    );
    client.vote(&zero_voter, &id, &true); // should panic
}

// ── Finalisation ──────────────────────────────────────────────────────────────

#[test]
fn test_finalise_passes_when_quorum_met_and_majority() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 200);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Pass me"),
        &make_str(&env, "desc"),
    );
    client.vote(&voter, &id, &true);

    advance_past_voting(&env);

    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).status, ProposalStatus::Passed);
}

#[test]
fn test_finalise_rejects_when_quorum_not_met() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 50); // 50 < quorum of 100

    let id = client.propose(
        &proposer,
        &make_str(&env, "Reject me"),
        &make_str(&env, "desc"),
    );
    client.vote(&voter, &id, &true);

    advance_past_voting(&env);

    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).status, ProposalStatus::Rejected);
}

#[test]
fn test_finalise_rejects_when_majority_not_reached() {
    let (env, client, proposer, gov_token) = setup();
    let voter_a = Address::generate(&env);
    let voter_b = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter_a, 100);
    mint_tokens(&env, &gov_token, &proposer, &voter_b, 200);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Against majority"),
        &make_str(&env, "desc"),
    );
    // 100 for, 200 against — 33% approval < 50% threshold
    client.vote(&voter_a, &id, &true);
    client.vote(&voter_b, &id, &false);

    advance_past_voting(&env);

    client.finalise(&id);
    assert_eq!(client.get_proposal(&id).status, ProposalStatus::Rejected);
}

#[test]
#[should_panic(expected = "voting still in progress")]
fn test_finalise_early_panics() {
    let (env, client, proposer, _token) = setup();
    let id = client.propose(
        &proposer,
        &make_str(&env, "Too early"),
        &make_str(&env, "desc"),
    );
    // Do NOT advance ledger — voting hasn't ended.
    client.finalise(&id);
}

// ── Execution ─────────────────────────────────────────────────────────────────

#[test]
fn test_mark_executed() {
    let (env, client, proposer, gov_token) = setup();
    let voter = Address::generate(&env);
    mint_tokens(&env, &gov_token, &proposer, &voter, 200);

    let id = client.propose(
        &proposer,
        &make_str(&env, "Execute me"),
        &make_str(&env, "desc"),
    );
    client.vote(&voter, &id, &true);
    advance_past_voting(&env);
    client.finalise(&id);

    client.mark_executed(&id);
    assert_eq!(client.get_proposal(&id).status, ProposalStatus::Executed);
}

#[test]
#[should_panic(expected = "proposal not passed")]
fn test_mark_executed_on_active_panics() {
    let (env, client, proposer, _token) = setup();
    let id = client.propose(
        &proposer,
        &make_str(&env, "Not passed"),
        &make_str(&env, "desc"),
    );
    client.mark_executed(&id);
}

// ── Cancel ────────────────────────────────────────────────────────────────────

#[test]
fn test_cancel() {
    let (env, client, proposer, _token) = setup();
    let id = client.propose(
        &proposer,
        &make_str(&env, "Cancel me"),
        &make_str(&env, "desc"),
    );
    client.cancel(&id);
    assert_eq!(client.get_proposal(&id).status, ProposalStatus::Cancelled);
}

#[test]
#[should_panic(expected = "proposal not active")]
fn test_cancel_already_cancelled_panics() {
    let (env, client, proposer, _token) = setup();
    let id = client.propose(
        &proposer,
        &make_str(&env, "Double cancel"),
        &make_str(&env, "desc"),
    );
    client.cancel(&id);
    client.cancel(&id); // should panic
}

// ── Config ────────────────────────────────────────────────────────────────────

#[test]
fn test_update_config() {
    let (_env, client, _admin, _token) = setup();
    client.update_config(&200_i128, &6000_u32, &50_u32);
    let (quorum, bps, period) = client.config();
    assert_eq!(quorum, 200);
    assert_eq!(bps, 6000);
    assert_eq!(period, 50);
}

// ── Gov token query ───────────────────────────────────────────────────────────

#[test]
fn test_gov_token_query() {
    let (_env, client, _admin, gov_token) = setup();
    assert_eq!(client.gov_token(), gov_token);
}
