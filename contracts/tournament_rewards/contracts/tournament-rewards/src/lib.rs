#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, token, Address, Env, Map, Vec,
};

// ─── Types ────────────────────────────────────────────────────────────────────

/// Life-cycle state of a tournament.
#[contracttype]
#[derive(Clone, PartialEq, Debug)]
pub enum TournamentStatus {
    Open,
    InProgress,
    Finalised,
    Cancelled,
}

/// Core tournament descriptor.
#[contracttype]
#[derive(Clone)]
pub struct Tournament {
    pub reward_token: Address,
    /// Entry fee per player (0 = free entry)
    pub entry_fee: i128,
    /// Accumulated prize pool (entry fees + sponsor top-ups)
    pub total_pool: i128,
    pub status: TournamentStatus,
    /// Ordered winner addresses (index 0 = 1st). Set on finalise.
    pub ranked_winners: Vec<Address>,
    /// Payout share per rank in basis points (must sum ≤ 10 000)
    pub payout_bps: Vec<u32>,
    /// Ledger at which the tournament was created
    pub created_at_ledger: u32,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Contract admin (instance)
    Admin,
    /// Next tournament ID counter (instance)
    NextTournamentId,
    /// Tournament keyed by u32 ID (persistent)
    Tournament(u32),
    /// Map<Address, i128>: entry fee paid per entrant, keyed per tournament (persistent)
    Entrants(u32),
    /// Entrant count for a tournament (persistent)
    EntrantCount(u32),
    /// Map<Address, bool>: whether winner has claimed, keyed per tournament (persistent)
    Claimed(u32),
}

// ─── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct TournamentRewardsContract;

#[contractimpl]
impl TournamentRewardsContract {
    // ── Init ──────────────────────────────────────────────────────────────────

    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .instance()
            .set(&DataKey::NextTournamentId, &0_u32);
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

    // ── Tournament creation ───────────────────────────────────────────────────

    /// Admin: create a tournament. Returns the new tournament ID.
    pub fn create_tournament(
        env: Env,
        reward_token: Address,
        entry_fee: i128,
        payout_bps: Vec<u32>,
    ) -> u32 {
        Self::require_admin(&env);
        assert!(entry_fee >= 0, "entry fee must be non-negative");
        assert!(payout_bps.len() > 0, "payout_bps must not be empty");
        assert!(payout_bps.len() <= 20, "too many payout slots");

        // Validate payout slice
        let mut total_bps: u32 = 0;
        for bps in payout_bps.iter() {
            total_bps += bps;
        }
        assert!(total_bps <= 10_000, "payout bps exceeds 10000");

        let id: u32 = env
            .storage()
            .instance()
            .get(&DataKey::NextTournamentId)
            .unwrap_or(0);

        let t = Tournament {
            reward_token,
            entry_fee,
            total_pool: 0,
            status: TournamentStatus::Open,
            ranked_winners: Vec::new(&env),
            payout_bps,
            created_at_ledger: env.ledger().sequence(),
        };

        env.storage().persistent().set(&DataKey::Tournament(id), &t);
        env.storage()
            .instance()
            .set(&DataKey::NextTournamentId, &(id + 1));
        env.storage()
            .persistent()
            .set(&DataKey::Entrants(id), &Map::<Address, i128>::new(&env));
        env.storage().persistent().set(&DataKey::EntrantCount(id), &0_u32);
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(id), &Map::<Address, bool>::new(&env));

        env.events()
            .publish((symbol_short!("t_create"),), (id, env.ledger().sequence()));
        id
    }

    // ── Entry ─────────────────────────────────────────────────────────────────

    /// Player: enter a tournament, paying the entry fee.
    pub fn enter(env: Env, player: Address, tournament_id: u32) {
        player.require_auth();

        let mut t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(t.status == TournamentStatus::Open, "tournament not open");

        let mut entrants: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Entrants(tournament_id))
            .unwrap_or_else(|| Map::new(&env));
        assert!(entrants.get(player.clone()).is_none(), "already entered");

        // Collect entry fee
        if t.entry_fee > 0 {
            let token_client = token::Client::new(&env, &t.reward_token);
            token_client.transfer(&player, &env.current_contract_address(), &t.entry_fee);
            t.total_pool += t.entry_fee;
        }

        entrants.set(player.clone(), t.entry_fee);
        env.storage()
            .persistent()
            .set(&DataKey::Entrants(tournament_id), &entrants);

        let count: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::EntrantCount(tournament_id))
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&DataKey::EntrantCount(tournament_id), &(count + 1));

        env.storage()
            .persistent()
            .set(&DataKey::Tournament(tournament_id), &t);

        env.events()
            .publish((symbol_short!("entered"),), (tournament_id, player));
    }

    // ── Sponsor top-up ────────────────────────────────────────────────────────

    /// Anyone may top-up an Open or InProgress pool.
    pub fn top_up(env: Env, from: Address, tournament_id: u32, amount: i128) {
        from.require_auth();
        assert!(amount > 0, "amount must be positive");

        let mut t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(
            t.status == TournamentStatus::Open || t.status == TournamentStatus::InProgress,
            "tournament not active"
        );

        let token_client = token::Client::new(&env, &t.reward_token);
        token_client.transfer(&from, &env.current_contract_address(), &amount);

        t.total_pool += amount;
        env.storage()
            .persistent()
            .set(&DataKey::Tournament(tournament_id), &t);

        env.events()
            .publish((symbol_short!("top_up"),), (tournament_id, from, amount));
    }

    // ── Lifecycle ─────────────────────────────────────────────────────────────

    /// Admin: close entry and start the tournament.
    pub fn start(env: Env, tournament_id: u32) {
        Self::require_admin(&env);
        let mut t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(t.status == TournamentStatus::Open, "tournament not open");
        t.status = TournamentStatus::InProgress;
        env.storage()
            .persistent()
            .set(&DataKey::Tournament(tournament_id), &t);
    }

    /// Admin: submit ranked results and open prize claiming.
    pub fn finalise(env: Env, tournament_id: u32, ranked_winners: Vec<Address>) {
        Self::require_admin(&env);
        let mut t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(t.status == TournamentStatus::InProgress, "tournament not in progress");
        assert!(
            ranked_winners.len() <= t.payout_bps.len(),
            "more winners than payout slots"
        );
        t.ranked_winners = ranked_winners;
        t.status = TournamentStatus::Finalised;
        env.storage()
            .persistent()
            .set(&DataKey::Tournament(tournament_id), &t);

        env.events()
            .publish((symbol_short!("finalise"),), tournament_id);
    }

    /// Admin: cancel — enables full entry-fee refunds.
    pub fn cancel(env: Env, tournament_id: u32) {
        Self::require_admin(&env);
        let mut t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(
            t.status == TournamentStatus::Open || t.status == TournamentStatus::InProgress,
            "cannot cancel in current state"
        );
        t.status = TournamentStatus::Cancelled;
        env.storage()
            .persistent()
            .set(&DataKey::Tournament(tournament_id), &t);
    }

    // ── Claim ─────────────────────────────────────────────────────────────────

    /// Ranked winner claims their share of the prize pool.
    pub fn claim_reward(env: Env, winner: Address, tournament_id: u32) {
        winner.require_auth();

        let t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(t.status == TournamentStatus::Finalised, "tournament not finalised");

        // Find rank (0-indexed)
        let mut rank_opt: Option<u32> = None;
        for (i, addr) in t.ranked_winners.iter().enumerate() {
            if addr == winner {
                rank_opt = Some(i as u32);
                break;
            }
        }
        let rank = rank_opt.expect("not a winner");

        let mut claimed: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&DataKey::Claimed(tournament_id))
            .unwrap_or_else(|| Map::new(&env));
        assert!(!claimed.get(winner.clone()).unwrap_or(false), "already claimed");

        let bps = t.payout_bps.get(rank).expect("payout slot not found") as i128;
        let payout = t.total_pool * bps / 10_000;
        assert!(payout > 0, "payout is zero");

        let token_client = token::Client::new(&env, &t.reward_token);
        token_client.transfer(&env.current_contract_address(), &winner, &payout);

        claimed.set(winner.clone(), true);
        env.storage()
            .persistent()
            .set(&DataKey::Claimed(tournament_id), &claimed);

        env.events()
            .publish((symbol_short!("rwd_claim"),), (tournament_id, winner, payout));
    }

    /// Entrant reclaims entry fee after cancellation.
    pub fn refund(env: Env, player: Address, tournament_id: u32) {
        player.require_auth();

        let t: Tournament = env
            .storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found");
        assert!(t.status == TournamentStatus::Cancelled, "tournament not cancelled");

        let mut entrants: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Entrants(tournament_id))
            .expect("entrant data not found");
        let fee = entrants.get(player.clone()).expect("not an entrant");
        assert!(fee > 0, "nothing to refund");

        let token_client = token::Client::new(&env, &t.reward_token);
        token_client.transfer(&env.current_contract_address(), &player, &fee);

        entrants.set(player.clone(), 0);
        env.storage()
            .persistent()
            .set(&DataKey::Entrants(tournament_id), &entrants);

        env.events()
            .publish((symbol_short!("refund"),), (tournament_id, player, fee));
    }

    // ── Queries ───────────────────────────────────────────────────────────────

    pub fn get_tournament(env: Env, tournament_id: u32) -> Tournament {
        env.storage()
            .persistent()
            .get(&DataKey::Tournament(tournament_id))
            .expect("tournament not found")
    }

    pub fn entry_fee_paid(env: Env, tournament_id: u32, player: Address) -> i128 {
        let entrants: Map<Address, i128> = env
            .storage()
            .persistent()
            .get(&DataKey::Entrants(tournament_id))
            .unwrap_or_else(|| Map::new(&env));
        entrants.get(player).unwrap_or(0)
    }

    pub fn is_claimed(env: Env, tournament_id: u32, winner: Address) -> bool {
        let claimed: Map<Address, bool> = env
            .storage()
            .persistent()
            .get(&DataKey::Claimed(tournament_id))
            .unwrap_or_else(|| Map::new(&env));
        claimed.get(winner).unwrap_or(false)
    }

    pub fn entrant_count(env: Env, tournament_id: u32) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::EntrantCount(tournament_id))
            .unwrap_or(0)
    }

    pub fn tournament_count(env: Env) -> u32 {
        env.storage()
            .instance()
            .get(&DataKey::NextTournamentId)
            .unwrap_or(0)
    }

    pub fn admin(env: Env) -> Address {
        env.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }
}

mod test;
