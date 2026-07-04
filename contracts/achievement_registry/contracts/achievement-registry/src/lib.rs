#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, String, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// On-chain representation of a single achievement definition.
#[contracttype]
#[derive(Clone)]
pub struct AchievementDef {
    /// Human-readable name, e.g. "First Deposit"
    pub name: String,
    /// Short description of the achievement
    pub description: String,
    /// Whether new awards of this achievement are still accepted
    pub active: bool,
}

/// A record that a specific user earned a specific achievement.
#[contracttype]
#[derive(Clone)]
pub struct AwardRecord {
    pub achievement_id: u32,
    pub awarded_at_ledger: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Address of the contract admin
    Admin,
    /// Next achievement ID counter
    NextId,
    /// AchievementDef keyed by u32 ID
    Achievement(u32),
    /// Vec<AwardRecord> keyed by user Address
    UserAwards(Address),
    /// Map<Address, bool> of addresses authorised to award achievements
    Awarders,
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct AchievementRegistryContract;

#[contractimpl]
impl AchievementRegistryContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    /// Initialise the registry. Can only be called once.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextId, &0_u32);
        env.storage()
            .instance()
            .set(&DataKey::Awarders, &Map::<Address, bool>::new(&env));
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

    fn is_awarder(env: &Env, caller: &Address) -> bool {
        let awarders: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Awarders)
            .unwrap_or_else(|| Map::new(env));
        awarders.get(caller.clone()).unwrap_or(false)
    }

    // ── Awarder management ────────────────────────────────────────────────────

    /// Admin: grant `address` the right to award achievements.
    pub fn add_awarder(env: Env, address: Address) {
        Self::require_admin(&env);
        let mut awarders: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Awarders)
            .unwrap_or_else(|| Map::new(&env));
        awarders.set(address, true);
        env.storage().instance().set(&DataKey::Awarders, &awarders);
    }

    /// Admin: revoke awarding rights from `address`.
    pub fn remove_awarder(env: Env, address: Address) {
        Self::require_admin(&env);
        let mut awarders: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Awarders)
            .unwrap_or_else(|| Map::new(&env));
        awarders.set(address, false);
        env.storage().instance().set(&DataKey::Awarders, &awarders);
    }

    // ── Achievement definitions ───────────────────────────────────────────────

    /// Admin: register a new achievement type and return its assigned ID.
    pub fn register_achievement(
        env: Env,
        name: String,
        description: String,
    ) -> u32 {
        Self::require_admin(&env);
        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(0);
        let def = AchievementDef {
            name,
            description,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Achievement(id), &def);
        env.storage()
            .instance()
            .set(&DataKey::NextId, &(id + 1));
        id
    }

    /// Admin: deactivate an achievement so it can no longer be awarded.
    pub fn deactivate_achievement(env: Env, achievement_id: u32) {
        Self::require_admin(&env);
        let mut def: AchievementDef = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(achievement_id))
            .expect("achievement not found");
        def.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::Achievement(achievement_id), &def);
    }

    // ── Awarding ──────────────────────────────────────────────────────────────

    /// Award an achievement to `user`. Caller must be admin or an awarder.
    /// Duplicate awards (same user + achievement) are rejected.
    pub fn award(env: Env, caller: Address, user: Address, achievement_id: u32) {
        caller.require_auth();

        // Authorisation: admin or registered awarder.
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        let is_admin = caller == admin;
        assert!(
            is_admin || Self::is_awarder(&env, &caller),
            "unauthorized"
        );

        // Achievement must exist and be active.
        let def: AchievementDef = env
            .storage()
            .persistent()
            .get(&DataKey::Achievement(achievement_id))
            .expect("achievement not found");
        assert!(def.active, "achievement is inactive");

        // Prevent duplicate awards.
        let mut awards: Vec<AwardRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::UserAwards(user.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        for record in awards.iter() {
            if record.achievement_id == achievement_id {
                panic!("already awarded");
            }
        }

        awards.push_back(AwardRecord {
            achievement_id,
            awarded_at_ledger: env.ledger().sequence(),
        });
        env.storage()
            .persistent()
            .set(&DataKey::UserAwards(user), &awards);
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return the definition for a given achievement ID.
    pub fn get_achievement(env: Env, achievement_id: u32) -> AchievementDef {
        env.storage()
            .persistent()
            .get(&DataKey::Achievement(achievement_id))
            .expect("achievement not found")
    }

    /// Return all award records for a user.
    pub fn get_user_awards(env: Env, user: Address) -> Vec<AwardRecord> {
        env.storage()
            .persistent()
            .get(&DataKey::UserAwards(user))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return true if `user` has earned `achievement_id`.
    pub fn has_achievement(env: Env, user: Address, achievement_id: u32) -> bool {
        let awards: Vec<AwardRecord> = env
            .storage()
            .persistent()
            .get(&DataKey::UserAwards(user))
            .unwrap_or_else(|| Vec::new(&env));
        for record in awards.iter() {
            if record.achievement_id == achievement_id {
                return true;
            }
        }
        false
    }

    /// Return the total number of registered achievements.
    pub fn achievement_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextId)
            .unwrap_or(0)
    }

    /// Return the contract admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

mod test;
