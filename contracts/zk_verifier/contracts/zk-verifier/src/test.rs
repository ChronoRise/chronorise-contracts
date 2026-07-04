#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Bytes, Env, String};

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn setup() -> (Env, ZkVerifierContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let contract_id = env.register(ZkVerifierContract, ());
    let client = ZkVerifierContractClient::new(&env, &contract_id);

    client.initialize(&admin);
    (env, client, admin)
}

/// Build a minimal valid proof for testing.
fn dummy_proof(env: &Env) -> Proof {
    Proof {
        a: Bytes::from_slice(env, &[1u8; 64]),
        b: Bytes::from_slice(env, &[2u8; 128]),
        c: Bytes::from_slice(env, &[3u8; 64]),
    }
}

/// Build a different proof (different bytes → different nullifier).
fn dummy_proof_2(env: &Env) -> Proof {
    Proof {
        a: Bytes::from_slice(env, &[4u8; 64]),
        b: Bytes::from_slice(env, &[5u8; 128]),
        c: Bytes::from_slice(env, &[6u8; 64]),
    }
}

fn dummy_inputs(env: &Env) -> soroban_sdk::Vec<Bytes> {
    vec![env, Bytes::from_slice(env, &[0xdeu8, 0xad, 0xbe, 0xef])]
}

fn register_circuit(
    env: &Env,
    client: &ZkVerifierContractClient,
    label: &str,
) -> u32 {
    client.register_circuit(
        &String::from_str(env, label),
        &Bytes::from_slice(env, &[0xabu8; 32]),
    )
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn test_initialize() {
    let (_env, client, admin) = setup();
    assert_eq!(client.admin(), admin);
    assert_eq!(client.circuit_count(), 0);
}

#[test]
#[should_panic(expected = "already initialized")]
fn test_double_initialize_panics() {
    let (_env, client, admin) = setup();
    client.initialize(&admin);
}

#[test]
fn test_register_circuit() {
    let (env, client, _admin) = setup();

    let id = register_circuit(&env, &client, "age_proof_v1");
    assert_eq!(id, 0);
    assert_eq!(client.circuit_count(), 1);

    let vk = client.get_circuit(&0);
    assert_eq!(vk.label, String::from_str(&env, "age_proof_v1"));
    assert!(vk.active);
}

#[test]
fn test_register_multiple_circuits_increments_id() {
    let (env, client, _admin) = setup();

    let id0 = register_circuit(&env, &client, "circuit_0");
    let id1 = register_circuit(&env, &client, "circuit_1");
    assert_eq!(id0, 0);
    assert_eq!(id1, 1);
    assert_eq!(client.circuit_count(), 2);
}

#[test]
fn test_deactivate_circuit() {
    let (env, client, _admin) = setup();

    let id = register_circuit(&env, &client, "old_circuit");
    client.deactivate_circuit(&id);

    let vk = client.get_circuit(&id);
    assert!(!vk.active);
}

#[test]
fn test_verify_proof() {
    let (env, client, _admin) = setup();

    let circuit_id = register_circuit(&env, &client, "membership_v1");
    let user = Address::generate(&env);
    let proof = dummy_proof(&env);
    let inputs = dummy_inputs(&env);

    let result = client.verify(&user, &circuit_id, &proof, &inputs);
    assert!(result);

    // Record should be stored.
    let records = client.get_user_records(&user);
    assert_eq!(records.len(), 1);
    assert_eq!(records.get(0).unwrap().circuit_id, circuit_id);
}

#[test]
#[should_panic(expected = "proof already used")]
fn test_replay_attack_rejected() {
    let (env, client, _admin) = setup();

    let circuit_id = register_circuit(&env, &client, "replay_test");
    let user = Address::generate(&env);
    let proof = dummy_proof(&env);
    let inputs = dummy_inputs(&env);

    client.verify(&user, &circuit_id, &proof, &inputs);
    // Second use of the same proof → replay protection should panic.
    client.verify(&user, &circuit_id, &proof, &inputs);
}

#[test]
fn test_different_proofs_both_accepted() {
    let (env, client, _admin) = setup();

    let circuit_id = register_circuit(&env, &client, "multi_proof");
    let user = Address::generate(&env);
    let inputs = dummy_inputs(&env);

    client.verify(&user, &circuit_id, &dummy_proof(&env), &inputs);
    client.verify(&user, &circuit_id, &dummy_proof_2(&env), &inputs);

    assert_eq!(client.get_user_records(&user).len(), 2);
}

#[test]
#[should_panic(expected = "circuit is inactive")]
fn test_verify_inactive_circuit_panics() {
    let (env, client, _admin) = setup();

    let circuit_id = register_circuit(&env, &client, "deactivated");
    client.deactivate_circuit(&circuit_id);

    let user = Address::generate(&env);
    client.verify(&user, &circuit_id, &dummy_proof(&env), &dummy_inputs(&env));
}

#[test]
fn test_nullifier_is_marked_used() {
    let (env, client, _admin) = setup();

    let circuit_id = register_circuit(&env, &client, "nullifier_test");
    let user = Address::generate(&env);
    let proof = dummy_proof(&env);

    // Build the same nullifier the contract would compute.
    let mut proof_bytes = Bytes::new(&env);
    proof_bytes.append(&proof.a);
    proof_bytes.append(&proof.b);
    proof_bytes.append(&proof.c);
    let hash = env.crypto().sha256(&proof_bytes);
    let nullifier = Bytes::from_slice(&env, hash.to_array().as_ref());

    assert!(!client.is_nullifier_used(&nullifier));
    client.verify(&user, &circuit_id, &proof, &dummy_inputs(&env));
    assert!(client.is_nullifier_used(&nullifier));
}
