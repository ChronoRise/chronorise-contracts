#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Map, String, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Full on-chain profile for a registered player.
#[contracttype]
#[derive(Clone)]
pub struct PlayerProfile {
    /// Unique human-readable username (enforced at registry level)
    pub username: String,
    /// Cumulative reputation points earned from achievements and wins
    pub reputation: i128,
    /// Total tournament wins
    pub wins: u32,
    /// Total tournaments entered
    pub tournaments_played: u32,
    /// IDs of games the player has participated in
    pub games: Vec<u32>,
    /// Whether the account is active (false = banned)
    pub active: bool,
    /// Ledger sequence at registration
    pub registered_at_ledger: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Contract admin address (instance)
    Admin,
    /// Counter: total registered players (instance)
    PlayerCount,
    /// PlayerProfile keyed by player Address (persistent)
    Player(Address),
    /// Reverse index: username → Address — enforces uniqueness (persistent)
    Username(String),
    /// Vec<u32> achievement IDs claimed as rewards by a player (persistent)
    ClaimedRewards(Address),
    /// Vec<u32> badge token IDs held by a player (persistent)
    BadgeList(Address),
    /// Map<Address, bool>: master index of all registered players (instance)
    PlayerIndex,
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct PlayerRegistryContract;

#[contractimpl]
impl PlayerRegistryContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    /// Initialise the registry. Can only be called once.
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::PlayerCount, &0_u32);
        env.storage()
            .instance()
            .set(&DataKey::PlayerIndex, &Vec::<Address>::new(&env));
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

    // ── Registration ──────────────────────────────────────────────────────────

    /// Register a new player with a unique username. Admin-only.
    pub fn register_player(env: Env, player: Address, username: String) {
        Self::require_admin(&env);

        // Guard: no duplicate wallet
        assert!(
            !env.storage()
                .persistent()
                .has(&DataKey::Player(player.clone())),
            "player already registered"
        );

        // Guard: username uniqueness
        assert!(
            !env.storage()
                .persistent()
                .has(&DataKey::Username(username.clone())),
            "username taken"
        );

        let profile = PlayerProfile {
            username: username.clone(),
            reputation: 0,
            wins: 0,
            tournaments_played: 0,
            games: Vec::new(&env),
            active: true,
            registered_at_ledger: env.ledger().sequence(),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Player(player.clone()), &profile);

        // Store reverse username → address index
        env.storage()
            .persistent()
            .set(&DataKey::Username(username), &player);

        // Increment counter
        let count: u32 = env
            .storage()
            .instance()
            .get(&DataKey::PlayerCount)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::PlayerCount, &(count + 1));

        // Append to player index list
        let mut index: Vec<Address> = env
            .storage()
            .instance()
            .get(&DataKey::PlayerIndex)
            .unwrap_or_else(|| Vec::new(&env));
        index.push_back(player.clone());
        env.storage()
            .instance()
            .set(&DataKey::PlayerIndex, &index);

        // Emit event
        env.events()
            .publish((symbol_short!("p_reg"),), player);
    }

    // ── Stats updates ─────────────────────────────────────────────────────────

    /// Admin: record a tournament win. Increments wins and tournaments_played.
    pub fn record_win(env: Env, player: Address) {
        Self::require_admin(&env);
        let mut p = Self::load_active_player(&env, &player);
        p.wins += 1;
        p.tournaments_played += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Player(player), &p);
    }

    /// Admin: record tournament participation (no win). Increments tournaments_played.
    pub fn record_participation(env: Env, player: Address) {
        Self::require_admin(&env);
        let mut p = Self::load_active_player(&env, &player);
        p.tournaments_played += 1;
        env.storage()
            .persistent()
            .set(&DataKey::Player(player), &p);
    }

    /// Admin: add reputation points to a player.
    pub fn add_reputation(env: Env, player: Address, delta: i128) {
        Self::require_admin(&env);
        let mut p = Self::load_active_player(&env, &player);
        p.reputation = p.reputation.saturating_add(delta);
        env.storage()
            .persistent()
            .set(&DataKey::Player(player), &p);
    }

    /// Admin: add a game to the player's games list.
    pub fn add_game(env: Env, player: Address, game_id: u32) {
        Self::require_admin(&env);
        let mut p = Self::load_active_player(&env, &player);
        // Avoid duplicates
        for g in p.games.iter() {
            if g == game_id {
                return;
            }
        }
        p.games.push_back(game_id);
        env.storage()
            .persistent()
            .set(&DataKey::Player(player), &p);
    }

    /// Admin: record that a reward (achievement_id) was claimed by this player.
    pub fn add_claimed_reward(env: Env, player: Address, achievement_id: u32) {
        Self::require_admin(&env);
        let _ = Self::load_active_player(&env, &player);

        let mut claimed: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::ClaimedRewards(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        for id in claimed.iter() {
            if id == achievement_id {
                return;
            }
        }
        claimed.push_back(achievement_id);
        env.storage()
            .persistent()
            .set(&DataKey::ClaimedRewards(player), &claimed);
    }

    /// Admin: record a badge token being added to the player's badge list.
    pub fn add_badge(env: Env, player: Address, token_id: u32) {
        Self::require_admin(&env);
        let _ = Self::load_active_player(&env, &player);

        let mut badges: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::BadgeList(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));

        for id in badges.iter() {
            if id == token_id {
                return;
            }
        }
        badges.push_back(token_id);
        env.storage()
            .persistent()
            .set(&DataKey::BadgeList(player), &badges);
    }

    /// Admin: remove a badge from the player's badge list.
    pub fn remove_badge(env: Env, player: Address, token_id: u32) {
        Self::require_admin(&env);
        let badges: Vec<u32> = env
            .storage()
            .persistent()
            .get(&DataKey::BadgeList(player.clone()))
            .unwrap_or_else(|| Vec::new(&env));
        let mut new_badges = Vec::new(&env);
        for id in badges.iter() {
            if id != token_id {
                new_badges.push_back(id);
            }
        }
        env.storage()
            .persistent()
            .set(&DataKey::BadgeList(player), &new_badges);
    }

    /// Admin: ban or unban a player.
    pub fn set_active(env: Env, player: Address, active: bool) {
        Self::require_admin(&env);
        let mut p: PlayerProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Player(player.clone()))
            .expect("player not found");
        p.active = active;
        env.storage()
            .persistent()
            .set(&DataKey::Player(player), &p);
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    fn load_active_player(env: &Env, player: &Address) -> PlayerProfile {
        let p: PlayerProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Player(player.clone()))
            .expect("player not found");
        assert!(p.active, "player is inactive");
        p
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    /// Return the profile for `player`.
    pub fn get_player(env: Env, player: Address) -> PlayerProfile {
        env.storage()
            .persistent()
            .get(&DataKey::Player(player))
            .expect("player not found")
    }

    /// Return true if `player` is registered.
    pub fn is_registered(env: Env, player: Address) -> bool {
        env.storage()
            .persistent()
            .has(&DataKey::Player(player))
    }

    /// Return the address registered with `username`, or panic if not found.
    pub fn lookup_username(env: Env, username: String) -> Address {
        env.storage()
            .persistent()
            .get(&DataKey::Username(username))
            .expect("username not found")
    }

    /// Return all achievement IDs claimed as rewards by `player`.
    pub fn claimed_rewards(env: Env, player: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::ClaimedRewards(player))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return all badge token IDs held by `player`.
    pub fn badge_list(env: Env, player: Address) -> Vec<u32> {
        env.storage()
            .persistent()
            .get(&DataKey::BadgeList(player))
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return the list of all registered player addresses.
    pub fn player_list(env: Env) -> Vec<Address> {
        env.storage()
            .instance()
            .get(&DataKey::PlayerIndex)
            .unwrap_or_else(|| Vec::new(&env))
    }

    /// Return total number of registered players.
    pub fn player_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::PlayerCount)
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
