#![cfg(test)]

use super::*;
use soroban_sdk::{
    testutils::Address as _,
    token::{Client as TokenClient, StellarAssetClient},
    vec, Address, Bytes, Env, String,
};

// ─── Import downstream contract types/clients ────────────────────────────────

// zk_verifier
mod zk_verifier_contract {
    soroban_sdk::contractimport!(
        file = "../../../../zk_verifier/target/wasm32-unknown-unknown/release/zk_verifier.wasm"
    );
}

// reward_pool
mod reward_pool_contract {
    soroban_sdk::contractimport!(
        file = "../../../../reward_pool/target/wasm32-unknown-unknown/release/reward_pool.wasm"
    );
}

// badge_nft
mod badge_nft_contract {
    soroban_sdk::contractimport!(
        file = "../../../../badge_nft/target/wasm32-unknown-unknown/release/badge_nft.wasm"
    );
}

// player_registry
mod player_registry_contract {
    soroban_sdk::contractimport!(
        file = "../../../../player_registry/target/wasm32-unknown-unknown/release/player_registry.wasm"
    );
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Build a minimal valid proof for testing (mirrors zk_verifier test helpers).
fn dummy_proof(env: &Env) -> ZkProof {
    ZkProof {
        a: Bytes::from_slice(env, &[1u8; 64]),
        b: Bytes::from_slice(env, &[2u8; 128]),
        c: Bytes::from_slice(env, &[3u8; 64]),
    }
}

fn dummy_proof_2(env: &Env) -> ZkProof {
    ZkProof {
        a: Bytes::from_slice(env, &[4u8; 64]),
        b: Bytes::from_slice(env, &[5u8; 128]),
        c: Bytes::from_slice(env, &[6u8; 64]),
    }
}

fn dummy_inputs(env: &Env) -> soroban_sdk::Vec<Bytes> {
    vec![env, Bytes::from_slice(env, &[0xdeu8, 0xad, 0xbe, 0xef])]
}

/// Full setup: deploys all contracts and wires them together.
struct Env2 {
    env: Env,
    admin: Address,
    token_addr: Address,
    sac: StellarAssetClient<'static>,
    zk_id: Address,
    pool_id: Address,
    badge_id: Address,
    registry_id: Address,
    orch_id: Address,
    orch: ClaimOrchestratorContractClient<'static>,
    circuit_id: u32,
}

impl Env2 {
    fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        // ── Token ──────────────────────────────────────────────────────────
        let sac_contract = env.register_stellar_asset_contract_v2(admin.clone());
        let token_addr = sac_contract.address();
        let sac = StellarAssetClient::new(&env, &token_addr);

        // ── zk_verifier ────────────────────────────────────────────────────
        let zk_id = env.register(zk_verifier_contract::WASM, ());
        let zk_client = zk_verifier_contract::Client::new(&env, &zk_id);
        zk_client.initialize(&admin);
        let circuit_id = zk_client.register_circuit(
            &String::from_str(&env, "achievement_v1"),
            &Bytes::from_slice(&env, &[0xabu8; 32]),
        );

        // ── reward_pool ────────────────────────────────────────────────────
        let pool_id = env.register(reward_pool_contract::WASM, ());
        let pool_client = reward_pool_contract::Client::new(&env, &pool_id);
        pool_client.initialize(&admin, &token_addr);

        // Fund the pool with tokens so claims can be paid out.
        sac.mint(&admin, &100_000_i128);
        let token_client_for_transfer = TokenClient::new(&env, &token_addr);
        // Deposit via the pool contract (admin deposits on behalf of themselves).
        pool_client.deposit(&admin, &100_000_i128);

        // Set reward for achievement 0.
        pool_client.set_achievement_reward(&0_u32, &1_000_i128);

        // ── badge_nft ──────────────────────────────────────────────────────
        let badge_id = env.register(badge_nft_contract::WASM, ());
        let badge_client = badge_nft_contract::Client::new(&env, &badge_id);
        badge_client.initialize(&admin);

        // Register badge type 0.
        badge_client.create_badge_type(
            &String::from_str(&env, "Achievement Hunter"),
            &String::from_str(&env, "ipfs://achievement-hunter"),
        );

        // ── player_registry ────────────────────────────────────────────────
        let registry_id = env.register(player_registry_contract::WASM, ());
        let registry_client = player_registry_contract::Client::new(&env, &registry_id);
        registry_client.initialize(&admin);

        // ── orchestrator ───────────────────────────────────────────────────
        let orch_id = env.register(ClaimOrchestratorContract, ());
        let orch = ClaimOrchestratorContractClient::new(&env, &orch_id);
        orch.initialize(
            &admin,
            &zk_id,
            &pool_id,
            &badge_id,
            &registry_id,
            &50_i128, // 50 reputation per claim
        );

        // Grant the orchestrator minting rights in badge_nft.
        badge_client.add_minter(&orch_id);

        // The pool's admin must authorise the orchestrator to call
        // claim_achievement_reward. Since we use mock_all_auths this is
        // handled automatically.

        // Register one player.
        let player = Address::generate(&env);
        registry_client.register_player(&player, &String::from_str(&env, "TestPlayer"));

        Self {
            env,
            admin,
            token_addr,
            sac: unsafe { core::mem::transmute(sac) },
            zk_id,
            pool_id,
            badge_id,
            registry_id,
            orch_id,
            orch: unsafe { core::mem::transmute(orch) },
            circuit_id,
        }
    }

    fn new_player(&self, name: &str) -> Address {
        let player = Address::generate(&self.env);
        let registry_client =
            player_registry_contract::Client::new(&self.env, &self.registry_id);
        registry_client.register_player(&player, &String::from_str(&self.env, name));
        player
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let e = Env2::new();
    assert_eq!(e.orch.admin(), e.admin);
    assert_eq!(e.orch.zk_verifier(), e.zk_id);
    assert_eq!(e.orch.reward_pool(), e.pool_id);
    assert_eq!(e.orch.badge_nft(), e.badge_id);
    assert_eq!(e.orch.player_registry(), e.registry_id);
    assert_eq!(e.orch.reputation_per_claim(), 50);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize() {
    let e = Env2::new();
    e.orch.initialize(
        &e.admin,
        &e.zk_id,
        &e.pool_id,
        &e.badge_id,
        &e.registry_id,
        &10_i128,
    );
}

#[test]
fn test_end_to_end_claim() {
    let e = Env2::new();
    let player = e.new_player("Alice");

    let proof = dummy_proof(&e.env);
    let inputs = dummy_inputs(&e.env);

    e.orch.claim(
        &player,
        &e.circuit_id,
        &proof,
        &inputs,
        &0_u32,  // achievement_id
        &1_000_i128,
        &0_u32,  // badge_type_id
    );

    // Verify token was received.
    let token_client = TokenClient::new(&e.env, &e.token_addr);
    assert_eq!(token_client.balance(&player), 1_000);

    // Verify badge was minted.
    let badge_client = badge_nft_contract::Client::new(&e.env, &e.badge_id);
    assert_eq!(badge_client.tokens_of(&player).len(), 1);

    // Verify registry was updated.
    let registry_client =
        player_registry_contract::Client::new(&e.env, &e.registry_id);
    let claimed = registry_client.claimed_rewards(&player);
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed.get(0).unwrap(), 0_u32); // achievement_id = 0

    let badges = registry_client.badge_list(&player);
    assert_eq!(badges.len(), 1);

    let profile = registry_client.get_player(&player);
    assert_eq!(profile.reputation, 50); // reputation_per_claim = 50
}

#[test]
#[should_panic(expected = "achievement reward already claimed")]
fn test_double_claim_same_achievement_panics() {
    let e = Env2::new();
    let player = e.new_player("Bob");

    // First claim succeeds.
    e.orch.claim(
        &player,
        &e.circuit_id,
        &dummy_proof(&e.env),
        &dummy_inputs(&e.env),
        &0_u32,
        &1_000_i128,
        &0_u32,
    );

    // Second claim with a different proof but the same achievement
    // must be blocked by reward_pool's per-achievement tracking.
    e.orch.claim(
        &player,
        &e.circuit_id,
        &dummy_proof_2(&e.env),
        &dummy_inputs(&e.env),
        &0_u32, // same achievement_id
        &1_000_i128,
        &0_u32,
    );
}

#[test]
#[should_panic(expected = "proof already used")]
fn test_replay_same_proof_panics() {
    let e = Env2::new();
    let player_a = e.new_player("PlayerA");
    let player_b = e.new_player("PlayerB");

    // Set a reward for achievement 1 as well for player_b.
    let pool_client = reward_pool_contract::Client::new(&e.env, &e.pool_id);
    pool_client.set_achievement_reward(&1_u32, &500_i128);

    // Same proof bytes submitted twice → nullifier reuse should panic.
    let proof = dummy_proof(&e.env);

    e.orch.claim(
        &player_a,
        &e.circuit_id,
        &proof,
        &dummy_inputs(&e.env),
        &0_u32,
        &1_000_i128,
        &0_u32,
    );

    // Same proof from a different player still carries the same nullifier.
    e.orch.claim(
        &player_b,
        &e.circuit_id,
        &proof, // same proof bytes — same nullifier
        &dummy_inputs(&e.env),
        &1_u32,
        &500_i128,
        &0_u32,
    );
}

#[test]
fn test_set_reputation_per_claim() {
    let e = Env2::new();
    e.orch.set_reputation_per_claim(&100_i128);
    assert_eq!(e.orch.reputation_per_claim(), 100);
}

#[test]
fn test_zero_reputation_per_claim_skips_rep_update() {
    let e = Env2::new();
    e.orch.set_reputation_per_claim(&0_i128);

    let player = e.new_player("ZeroRep");
    e.orch.claim(
        &player,
        &e.circuit_id,
        &dummy_proof(&e.env),
        &dummy_inputs(&e.env),
        &0_u32,
        &1_000_i128,
        &0_u32,
    );

    let registry_client =
        player_registry_contract::Client::new(&e.env, &e.registry_id);
    assert_eq!(registry_client.get_player(&player).reputation, 0);
}
