#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, token, Address, Env, Map, Vec,
};

// ─── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    Admin,
    RewardToken,
    TotalDeposited,
    Balances,
}

// ─── Contract ────────────────────────────────────────────────────────────────

#[contract]
pub struct RewardPoolContract;

#[contractimpl]
impl RewardPoolContract {
    // ── Initialisation ───────────────────────────────────────────────────────

    /// Initialise the pool with an admin address and the reward token contract.
    /// Can only be called once.
    pub fn initialize(env: Env, admin: Address, reward_token: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::RewardToken, &reward_token);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposited, &0_i128);
        env.storage()
            .instance()
            .set(&DataKey::Balances, &Map::<Address, i128>::new(&env));
    }

    // ── Deposit ──────────────────────────────────────────────────────────────

    /// Deposit `amount` reward tokens into the pool on behalf of `from`.
    /// The caller must have approved this contract to transfer the tokens.
    pub fn deposit(env: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardToken)
            .expect("not initialized");

        // Transfer tokens from sender into this contract.
        let token = token::Client::new(&env, &token_id);
        token.transfer(&from, &env.current_contract_address(), &amount);

        // Update balances.
        let mut balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DataKey::Balances)
            .unwrap_or_else(|| Map::new(&env));
        let prev = balances.get(from.clone()).unwrap_or(0);
        balances.set(from, prev + amount);
        env.storage()
            .instance()
            .set(&DataKey::Balances, &balances);

        // Update total.
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposited)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposited, &(total + amount));
    }

    // ── Distribute ───────────────────────────────────────────────────────────

    /// Admin-only: distribute `amount` tokens to `recipient`.
    pub fn distribute(env: Env, recipient: Address, amount: i128) {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized");
        admin.require_auth();
        assert!(amount > 0, "amount must be positive");

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardToken)
            .expect("not initialized");

        let token = token::Client::new(&env, &token_id);
        token.transfer(&env.current_contract_address(), &recipient, &amount);

        // Reduce total deposited.
        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposited)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposited, &(total - amount));
    }

    // ── Withdraw ─────────────────────────────────────────────────────────────

    /// Withdraw up to `amount` of a depositor's own balance back to themselves.
    pub fn withdraw(env: Env, from: Address, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DataKey::Balances)
            .expect("not initialized");

        let balance = balances.get(from.clone()).unwrap_or(0);
        assert!(balance >= amount, "insufficient balance");

        balances.set(from.clone(), balance - amount);
        env.storage()
            .instance()
            .set(&DataKey::Balances, &balances);

        let total: i128 = env
            .storage()
            .instance()
            .get(&DataKey::TotalDeposited)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::TotalDeposited, &(total - amount));

        let token_id: Address = env
            .storage()
            .instance()
            .get(&DataKey::RewardToken)
            .expect("not initialized");

        let token = token::Client::new(&env, &token_id);
        token.transfer(&env.current_contract_address(), &from, &amount);
    }

    // ── Queries ──────────────────────────────────────────────────────────────

    /// Return the reward-token balance of `address` inside the pool.
    pub fn balance_of(env: Env, address: Address) -> i128 {
        let balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DataKey::Balances)
            .unwrap_or_else(|| Map::new(&env));
        balances.get(address).unwrap_or(0)
    }

    /// Return the total tokens currently held in the pool.
    pub fn total_deposited(env: Env) -> i128 {
        env.storage()
            .instance()
            .get(&DataKey::TotalDeposited)
            .unwrap_or(0)
    }

    /// Return the admin address.
    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    /// Return the reward token contract address.
    pub fn reward_token(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::RewardToken)
            .expect("not initialized")
    }

    /// Return a list of all depositor addresses.
    pub fn depositors(env: Env) -> Vec<Address> {
        let balances: Map<Address, i128> = env
            .storage()
            .instance()
            .get(&DataKey::Balances)
            .unwrap_or_else(|| Map::new(&env));
        balances.keys()
    }
}

mod test;
