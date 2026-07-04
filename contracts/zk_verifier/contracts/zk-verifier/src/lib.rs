#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Bytes, Env, String, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// A Groth16-style proof submitted on-chain.
/// All field elements are big-endian encoded byte strings.
#[contracttype]
#[derive(Clone)]
pub struct Proof {
    /// G1 point A  (uncompressed, 64 bytes)
    pub a: Bytes,
    /// G2 point B  (uncompressed, 128 bytes)
    pub b: Bytes,
    /// G1 point C  (uncompressed, 64 bytes)
    pub c: Bytes,
}

/// Registered verification key for a circuit.
#[contracttype]
#[derive(Clone)]
pub struct VerifyingKey {
    /// Serialised verifying-key bytes (circuit-specific, opaque to the contract)
    pub vk_bytes: Bytes,
    /// Human-readable label, e.g. "age_proof_v1"
    pub label: String,
    /// Whether new proofs against this key are still accepted
    pub active: bool,
}

/// Outcome stored after a successful verification.
/// Hashes are stored as raw 32-byte `Bytes` values (SHA-256 output).
#[contracttype]
#[derive(Clone)]
pub struct VerificationRecord {
    pub circuit_id: u32,
    /// SHA-256 hash of the concatenated public input bytes
    pub inputs_hash: Bytes,
    /// SHA-256 hash of the concatenated proof bytes (a ++ b ++ c)
    pub proof_hash: Bytes,
    pub verified_at_ledger: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Address of the contract admin
    Admin,
    /// Next circuit ID counter
    NextCircuitId,
    /// VerifyingKey keyed by u32 circuit ID
    VerifyingKey(u32),
    /// Replay-protection flag keyed by proof nullifier (Bytes)
    Nullifier(Bytes),
    /// Vec<VerificationRecord> keyed by user Address
    UserRecords(Address),
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ZkVerifierContract;

#[contractimpl]
impl ZkVerifierContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    /// Initialise the verifier registry. Can only be called once.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextCircuitId, &0_u32);
    }

    // ── Admin helpers ─────────────────────────────────────────────────────────

    fn require_admin(env: &Env) -> Address {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        admin
    }

    // ── Circuit / VK management ───────────────────────────────────────────────

    /// Admin: register a new verifying key and return its circuit ID.
    pub fn register_circuit(env: Env, label: String, vk_bytes: Bytes) -> u32 {
        Self::require_admin(&env);
        assert!(vk_bytes.len() > 0, "vk_bytes must not be empty");

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextCircuitId)
            .unwrap_or(0);

        let vk = VerifyingKey {
            vk_bytes,
            label,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::VerifyingKey(id), &vk);
        env.storage()
            .instance()
            .set(&DataKey::NextCircuitId, &(id + 1));
        id
    }

    /// Admin: deactivate a circuit so no new proofs are accepted for it.
    pub fn deactivate_circuit(env: Env, circuit_id: u32) {
        Self::require_admin(&env);
        let mut vk: VerifyingKey = env
            .storage()
            .persistent()
            .get(&DataKey::VerifyingKey(circuit_id))
            .expect("circuit not found");
        vk.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::VerifyingKey(circuit_id), &vk);
    }

    // ── Proof verification ────────────────────────────────────────────────────

    /// Verify a ZK proof on behalf of `user`.
    ///
    /// The contract performs the following checks:
    /// 1. Circuit exists and is active.
    /// 2. Proof bytes are non-empty and public inputs are provided.
    /// 3. The nullifier (SHA-256 of proof bytes) has not been seen before
    ///    (replay protection).
    /// 4. `verify_proof_internal` stub — replace with a real pairing check
    ///    once Soroban exposes BLS12-381 host functions.
    ///
    /// On success the record is stored under the user's address and the
    /// nullifier is marked as used.
    pub fn verify(
        env: Env,
        user: Address,
        circuit_id: u32,
        proof: Proof,
        public_inputs: Vec<Bytes>,
    ) -> bool {
        user.require_auth();

        // 1. Circuit must exist and be active.
        let vk: VerifyingKey = env
            .storage()
            .persistent()
            .get(&DataKey::VerifyingKey(circuit_id))
            .expect("circuit not found");
        assert!(vk.active, "circuit is inactive");

        // 2. Basic structural validation.
        assert!(proof.a.len() > 0, "proof.a is empty");
        assert!(proof.b.len() > 0, "proof.b is empty");
        assert!(proof.c.len() > 0, "proof.c is empty");
        assert!(public_inputs.len() > 0, "public_inputs is empty");

        // 3. Build nullifier = SHA-256(a ++ b ++ c).
        let mut proof_bytes = Bytes::new(&env);
        proof_bytes.append(&proof.a);
        proof_bytes.append(&proof.b);
        proof_bytes.append(&proof.c);
        let proof_hash_arr = env.crypto().sha256(&proof_bytes);
        let nullifier = Bytes::from_slice(&env, proof_hash_arr.to_array().as_ref());

        // Replay protection.
        assert!(
            !env.storage()
                .persistent()
                .has(&DataKey::Nullifier(nullifier.clone())),
            "proof already used"
        );

        // 4. Hash public inputs for the record.
        let mut inputs_bytes = Bytes::new(&env);
        for input in public_inputs.iter() {
            inputs_bytes.append(&input);
        }
        let inputs_hash_arr = env.crypto().sha256(&inputs_bytes);
        let inputs_hash = Bytes::from_slice(&env, inputs_hash_arr.to_array().as_ref());
        let proof_hash = Bytes::from_slice(&env, proof_hash_arr.to_array().as_ref());

        // 5. Cryptographic verification stub.
        let valid = Self::verify_proof_internal(&proof);
        assert!(valid, "invalid proof");

        // 6. Persist record and mark nullifier as used.
        let record = VerificationRecord {
            circuit_id,
            inputs_hash,
            proof_hash,
            verified_at_ledger: env.ledger().sequence(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Nullifier(nullifier), &true);

        let mut records: Vec<VerificationRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::UserRecords(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        records.push_back(record);
        env.storage()
            .persistent()
            .set(&DataKey::UserRecords(user), &records);

        true
    }

    /// Internal proof verification stub.
    ///
    /// Currently accepts any structurally non-trivial proof.
    /// Replace with a real Groth16 / PLONK pairing check once
    /// BLS12-381 precompiles are available in the Soroban host.
    fn verify_proof_internal(proof: &Proof) -> bool {
        proof.a.len() > 0 && proof.b.len() > 0 && proof.c.len() > 0
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return the verifying key for a circuit.
    pub fn get_circuit(env: Env, circuit_id: u32) -> VerifyingKey {
        env.storage()
            .persistent()
            .get(&DataKey::VerifyingKey(circuit_id))
            .expect("circuit not found")
    }

    /// Return all verification records for a user.
    pub fn get_user_records(env: Env, user: Address) -> Vec<VerificationRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::UserRecords(user))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return true if the nullifier (SHA-256 of proof bytes) has already been used.
    pub fn is_nullifier_used(env: Env, nullifier: Bytes) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Nullifier(nullifier))
    }

    /// Return the total number of registered circuits.
    pub fn circuit_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextCircuitId)
            .unwrap_or(0)
    }

    /// Return the admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

mod test;
