#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, Map, String, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Class-level definition for a badge type.
#[contracttype]
#[derive(Clone)]
pub struct BadgeType {
    /// Display name, e.g. "Champion"
    pub name: String,
    /// IPFS / on-chain URI for artwork / JSON metadata
    pub metadata_uri: String,
    /// Whether new badges of this type can still be minted
    pub active: bool,
}

/// An individual soulbound badge token.
///
/// Soulbound means the token is permanently bound to the address it was
/// minted to. There is no `transfer` function. The only mutation allowed
/// is burning by the owner.
#[contracttype]
#[derive(Clone)]
pub struct Badge {
    pub badge_type_id: u32,
    /// Original and permanent owner — never changes after mint.
    pub owner: Address,
    /// Ledger sequence at mint
    pub minted_at_ledger: u32,
    /// Whether the badge is still alive (false = burned)
    pub alive: bool,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Contract admin (instance)
    Admin,
    /// Next badge-type ID counter (instance)
    NextTypeId,
    /// Next token ID counter (instance)
    NextTokenId,
    /// BadgeType keyed by u32 type ID (persistent)
    BadgeType(u32),
    /// Badge keyed by u32 token ID (persistent)
    Token(u32),
    /// Vec<u32> live token IDs owned by Address (persistent)
    OwnerTokens(Address),
    /// Map<Address, bool> of authorised minters (instance)
    Minters,
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct BadgeNftContract;

#[contractimpl]
impl BadgeNftContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::NextTypeId, &0_u32);
        env.storage().instance().set(&DataKey::NextTokenId, &0_u32);
        env.storage()
            .instance()
            .set(&DataKey::Minters, &Map::<Address, bool>::new(&env));
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

    fn is_minter(env: &Env, caller: &Address) -> bool {
        let minters: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Minters)
            .unwrap_or_else(|| Map::new(env));
        minters.get(caller.clone()).unwrap_or(false)
    }

    // ── Minter management ─────────────────────────────────────────────────────

    pub fn add_minter(env: Env, address: Address) {
        Self::require_admin(&env);
        let mut minters: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Minters)
            .unwrap_or_else(|| Map::new(&env));
        minters.set(address, true);
        env.storage().instance().set(&DataKey::Minters, &minters);
    }

    pub fn remove_minter(env: Env, address: Address) {
        Self::require_admin(&env);
        let mut minters: Map<Address, bool> = env
            .storage()
            .instance()
            .get(&DataKey::Minters)
            .unwrap_or_else(|| Map::new(&env));
        minters.set(address, false);
        env.storage().instance().set(&DataKey::Minters, &minters);
    }

    // ── Badge type management ─────────────────────────────────────────────────

    /// Admin: define a new badge type.
    pub fn create_badge_type(env: Env, name: String, metadata_uri: String) -> u32 {
        Self::require_admin(&env);

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTypeId)
            .unwrap_or(0);

        let badge_type = BadgeType {
            name,
            metadata_uri,
            active: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::BadgeType(id), &badge_type);
        env.storage()
            .instance()
            .set(&DataKey::NextTypeId, &(id + 1));
        id
    }

    /// Admin: deactivate a badge type so no new tokens can be minted.
    pub fn deactivate_badge_type(env: Env, badge_type_id: u32) {
        Self::require_admin(&env);
        let mut bt: BadgeType = env
            .storage()
            .persistent()
            .get(&DataKey::BadgeType(badge_type_id))
            .expect("badge type not found");
        bt.active = false;
        env.storage()
            .persistent()
            .set(&DataKey::BadgeType(badge_type_id), &bt);
    }

    // ── Minting ───────────────────────────────────────────────────────────────

    /// Mint a soulbound badge of `badge_type_id` to `recipient`.
    /// Caller must be admin or an authorised minter.
    /// Badges are permanently bound to the recipient — they cannot be transferred.
    pub fn mint(env: Env, caller: Address, recipient: Address, badge_type_id: u32) -> u32 {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        assert!(
            caller == admin || Self::is_minter(&env, &caller),
            "unauthorized"
        );

        let bt: BadgeType = env
            .storage()
            .persistent()
            .get(&DataKey::BadgeType(badge_type_id))
            .expect("badge type not found");
        assert!(bt.active, "badge type inactive");

        let token_id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0);

        let badge = Badge {
            badge_type_id,
            owner: recipient.clone(),
            minted_at_ledger: env.ledger().sequence(),
            alive: true,
        };
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &badge);
        env.storage()
            .instance()
            .set(&DataKey::NextTokenId, &(token_id + 1));

        // Index: owner → token list
        let mut tokens: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(recipient.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        tokens.push_back(token_id);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(recipient.clone()), &tokens);

        env.events()
            .publish((soroban_sdk::symbol_short!("badge_mnt"),), (recipient, token_id));

        token_id
    }

    // ── Burn ─────────────────────────────────────────────────────────────────

    /// Owner burns their own badge, permanently destroying it.
    /// Burning is the only mutation permitted on a soulbound badge.
    pub fn burn(env: Env, owner: Address, token_id: u32) {
        owner.require_auth();

        let mut badge: Badge = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("token not found");

        assert!(badge.owner == owner, "unauthorized");
        assert!(badge.alive, "already burned");

        badge.alive = false;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id), &badge);

        // Remove from owner token list
        let old_tokens: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        let mut new_tokens = Vec::new(&env);
        for t in old_tokens.iter() {
            if t != token_id {
                new_tokens.push_back(t);
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::OwnerTokens(owner.clone()), &new_tokens);

        env.events()
            .publish((soroban_sdk::symbol_short!("badge_brn"),), (owner, token_id));
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn get_token(env: Env, token_id: u32) -> Badge {
        env.storage()
            .persistent()
            .get(&DataKey::Token(token_id))
            .expect("token not found")
    }

    /// Return all live token IDs owned by `owner`.
    pub fn tokens_of(env: Env, owner: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::OwnerTokens(owner))
            .unwrap_or_else(|| Vec::new(&env))
    }

    pub fn get_badge_type(env: Env, badge_type_id: u32) -> BadgeType {
        env.storage()
            .persistent()
            .get(&DataKey::BadgeType(badge_type_id))
            .expect("badge type not found")
    }

    pub fn badge_type_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextTypeId)
            .unwrap_or(0)
    }

    /// Return the total number of minted tokens (ever).
    pub fn total_supply(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextTokenId)
            .unwrap_or(0)
    }

    /// Alias for total_supply — kept for compatibility.
    pub fn total_minted(env: Env) -> u32 {
        Self::total_supply(env)
    }

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

mod test;
