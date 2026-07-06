#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Bytes, Env, Vec,
};

// ─── Cross-Contract Client Declarations ──────────────────────────────────────
//
// These lightweight clients allow the orchestrator to invoke the other
// ChronoRise contracts. Soroban generates the call machinery from these
// declarations at compile time.

/// zk_verifier contract interface — used to check replay protection.
mod zk_verifier {
    use soroban_sdk::{contractclient, Bytes, Env};

    #[contractclient(name = "ZkVerifierClient")]
    pub trait ZkVerifierInterface {
        fn is_nullifier_used(env: Env, nullifier: Bytes) -> bool;
        fn verify(
            env: Env,
            user: soroban_sdk::Address,
            circuit_id: u32,
            proof: super::ZkProof,
            public_inputs: soroban_sdk::Vec<Bytes>,
        ) -> bool;
    }
}

/// reward_pool contract interface — used to pay out achievement rewards.
mod reward_pool {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "RewardPoolClient")]
    pub trait RewardPoolInterface {
        fn claim_achievement_reward(
            env: Env,
            recipient: Address,
            achievement_id: u32,
            amount: i128,
        );
        fn is_achievement_claimed(env: Env, player: Address, achievement_id: u32) -> bool;
        fn achievement_reward_amount(env: Env, achievement_id: u32) -> i128;
    }
}

/// badge_nft contract interface — used to mint the soulbound badge.
mod badge_nft {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "BadgeNftClient")]
    pub trait BadgeNftInterface {
        fn mint(env: Env, caller: Address, recipient: Address, badge_type_id: u32) -> u32;
    }
}

/// player_registry contract interface — used to update player state.
mod player_registry {
    use soroban_sdk::{contractclient, Address, Env};

    #[contractclient(name = "PlayerRegistryClient")]
    pub trait PlayerRegistryInterface {
        fn is_registered(env: Env, player: Address) -> bool;
        fn add_claimed_reward(env: Env, player: Address, achievement_id: u32);
        fn add_badge(env: Env, player: Address, token_id: u32);
        fn add_reputation(env: Env, player: Address, delta: i128);
    }
}

// ─── Shared Proof Type ────────────────────────────────────────────────────────
//
// Mirrors the `Proof` struct in zk_verifier so the orchestrator can receive
// and forward proofs without importing that crate directly.

#[contracttype]
#[derive(Clone)]
pub struct ZkProof {
    pub a: Bytes,
    pub b: Bytes,
    pub c: Bytes,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Orchestrator admin
    Admin,
    /// Address of the deployed zk_verifier contract
    ZkVerifier,
    /// Address of the deployed reward_pool contract
    RewardPool,
    /// Address of the deployed badge_nft contract
    BadgeNft,
    /// Address of the deployed player_registry contract
    PlayerRegistry,
    /// Reputation delta awarded per verified claim
    ReputationPerClaim,
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct ClaimOrchestratorContract;

#[contractimpl]
impl ClaimOrchestratorContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    /// Initialise the orchestrator with addresses of all downstream contracts.
    /// Can only be called once.
    pub fn initialize(
        env: Env,
        admin: Address,
        zk_verifier: Address,
        reward_pool: Address,
        badge_nft: Address,
        player_registry: Address,
        reputation_per_claim: i128,
    ) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        assert!(reputation_per_claim >= 0, "reputation_per_claim must be non-negative");

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::ZkVerifier, &zk_verifier);
        env.storage().instance().set(&DataKey::RewardPool, &reward_pool);
        env.storage().instance().set(&DataKey::BadgeNft, &badge_nft);
        env.storage()
            .instance()
            .set(&DataKey::PlayerRegistry, &player_registry);
        env.storage()
            .instance()
            .set(&DataKey::ReputationPerClaim, &reputation_per_claim);
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

    // ── Config updates ────────────────────────────────────────────────────────

    /// Admin: update the reputation awarded per successful claim.
    pub fn set_reputation_per_claim(env: Env, delta: i128) {
        Self::require_admin(&env);
        assert!(delta >= 0, "reputation_per_claim must be non-negative");
        env.storage()
            .instance()
            .set(&DataKey::ReputationPerClaim, &delta);
    }

    // ── Core Orchestration ────────────────────────────────────────────────────

    /// End-to-end achievement claim.
    ///
    /// Flow:
    ///   1. Verify the ZK proof via `zk_verifier.verify()`.
    ///   2. Release the token reward via `reward_pool.claim_achievement_reward()`.
    ///   3. Mint the soulbound badge via `badge_nft.mint()`.
    ///   4. Update player state via `player_registry.add_claimed_reward()`,
    ///      `.add_badge()`, and `.add_reputation()`.
    ///   5. Emit a top-level `claim_ok` event.
    ///
    /// Parameters:
    /// - `player`         — the Stellar address claiming the reward
    /// - `circuit_id`     — ID of the ZK circuit registered in zk_verifier
    /// - `proof`          — the Groth16 proof
    /// - `public_inputs`  — public inputs accompanying the proof
    /// - `achievement_id` — the achievement being claimed
    /// - `reward_amount`  — the token amount to release from reward_pool
    /// - `badge_type_id`  — the badge type to mint in badge_nft
    pub fn claim(
        env: Env,
        player: Address,
        circuit_id: u32,
        proof: ZkProof,
        public_inputs: Vec<Bytes>,
        achievement_id: u32,
        reward_amount: i128,
        badge_type_id: u32,
    ) {
        player.require_auth();
        assert!(reward_amount > 0, "reward_amount must be positive");

        // Load contract addresses.
        let zk_verifier_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::ZkVerifier)
            .expect("not initialized");
        let reward_pool_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("not initialized");
        let badge_nft_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::BadgeNft)
            .expect("not initialized");
        let player_registry_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::PlayerRegistry)
            .expect("not initialized");
        let rep_delta: i128 = env
            .storage()
            .instance()
            .get(&DataKey::ReputationPerClaim)
            .unwrap_or(0);

        // ── Step 1: ZK proof verification ─────────────────────────────────────
        //
        // The zk_verifier contract checks:
        //   - Circuit is active.
        //   - Proof bytes are non-empty.
        //   - Nullifier has not been used (replay protection).
        //   - verify_proof_internal passes.
        // It panics on any failure, aborting the whole transaction.
        let zk_client = zk_verifier::ZkVerifierClient::new(&env, &zk_verifier_id);
        let verified = zk_client.verify(&player, &circuit_id, &proof, &public_inputs);
        assert!(verified, "proof verification failed");

        // ── Step 2: Token reward release ──────────────────────────────────────
        //
        // reward_pool enforces per-player per-achievement replay protection
        // independently of the nullifier, preventing the same achievement
        // from being claimed twice even if two different proofs are submitted.
        let pool_client = reward_pool::RewardPoolClient::new(&env, &reward_pool_id);
        pool_client.claim_achievement_reward(&player, &achievement_id, &reward_amount);

        // ── Step 3: Soulbound badge mint ──────────────────────────────────────
        //
        // The orchestrator is an authorised minter in badge_nft.
        // The contract address is used as the `caller`.
        let badge_client = badge_nft::BadgeNftClient::new(&env, &badge_nft_id);
        let token_id = badge_client.mint(
            &env.current_contract_address(),
            &player,
            &badge_type_id,
        );

        // ── Step 4: Player registry update ───────────────────────────────────
        let registry_client =
            player_registry::PlayerRegistryClient::new(&env, &player_registry_id);

        // Record the claimed achievement ID.
        registry_client.add_claimed_reward(&player, &achievement_id);

        // Record the new badge token.
        registry_client.add_badge(&player, &token_id);

        // Award reputation points.
        if rep_delta > 0 {
            registry_client.add_reputation(&player, &rep_delta);
        }

        // ── Step 5: Top-level event ────────────────────────────────────────────
        env.events().publish(
            (symbol_short!("claim_ok"),),
            (player, achievement_id, token_id, reward_amount),
        );
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn zk_verifier(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::ZkVerifier)
            .expect("not initialized")
    }

    pub fn reward_pool(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::RewardPool)
            .expect("not initialized")
    }

    pub fn badge_nft(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::BadgeNft)
            .expect("not initialized")
    }

    pub fn player_registry(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::PlayerRegistry)
            .expect("not initialized")
    }

    pub fn reputation_per_claim(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::ReputationPerClaim)
            .unwrap_or(0)
    }
}

mod test;
