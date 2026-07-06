#![no_std]
use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

// ─── Protocol Version ─────────────────────────────────────────────────────────

/// Semantic version of the ChronoRise protocol contracts.
#[contracttype]
#[derive(Clone)]
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// Return the current protocol version.
pub fn protocol_version(_env: &Env) -> ProtocolVersion {
    ProtocolVersion {
        major: 0,
        minor: 1,
        patch: 0,
    }
}

// ─── Game Types ───────────────────────────────────────────────────────────────

/// Opaque identifier for a game registered in the protocol.
#[contracttype]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct GameId(pub u32);

// ─── Reward Types ─────────────────────────────────────────────────────────────

/// Category of reward, used by tournament_rewards to tag prize pools.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum RewardType {
    Achievement,
    Daily,
    Weekly,
    Season,
    Sponsored,
    Tournament,
}

// ─── Tournament Classification ────────────────────────────────────────────────

/// High-level kind of tournament.
#[contracttype]
#[derive(Clone, PartialEq)]
pub enum TournamentKind {
    Open,
    Invitational,
    SeasonLeague,
    DailyChallenge,
}

// ─── Error Codes ──────────────────────────────────────────────────────────────
//
// Used across all contracts for consistent panic messages and future
// migration to ContractError integers.

pub const ERR_NOT_INITIALIZED: u32 = 1;
pub const ERR_ALREADY_INITIALIZED: u32 = 2;
pub const ERR_UNAUTHORIZED: u32 = 3;
pub const ERR_NOT_FOUND: u32 = 4;
pub const ERR_DUPLICATE: u32 = 5;
pub const ERR_INVALID_AMOUNT: u32 = 6;
pub const ERR_INVALID_STATE: u32 = 7;
pub const ERR_INACTIVE: u32 = 8;
pub const ERR_VERIFICATION_FAILED: u32 = 9;
pub const ERR_UNSUPPORTED_TOKEN: u32 = 10;
pub const ERR_BPS_OVERFLOW: u32 = 11;
pub const ERR_INSUFFICIENT_BALANCE: u32 = 12;
pub const ERR_ALREADY_CLAIMED: u32 = 13;
pub const ERR_SOULBOUND: u32 = 14;
pub const ERR_ZERO_WEIGHT: u32 = 15;
pub const ERR_VOTING_ENDED: u32 = 16;
pub const ERR_QUORUM_NOT_MET: u32 = 17;

// ─── Basis-Point Maths ────────────────────────────────────────────────────────

/// Maximum basis points (100 %).
pub const MAX_BPS: u32 = 10_000;

/// Maximum ranked payout slots per tournament.
pub const MAX_PAYOUT_SLOTS: u32 = 20;

/// Compute `(amount * bps) / 10_000`.
/// Panics with "bps exceeds 10000" if `bps > MAX_BPS`.
pub fn bps_of(amount: i128, bps: u32) -> i128 {
    assert!(bps <= MAX_BPS, "bps exceeds 10000");
    amount * (bps as i128) / (MAX_BPS as i128)
}

// ─── Governance Token Balance Helper ─────────────────────────────────────────
//
// Re-export the standard Soroban token client so the governance contract
// can read voter balances from the governance token without importing a
// separate crate.
pub use soroban_sdk::token::Client as TokenBalanceClient;

// ─── Event Helpers ────────────────────────────────────────────────────────────

/// Emit a player-registered event.
pub fn emit_player_registered(env: &Env, player: &Address) {
    let topic: Symbol = symbol_short!("p_reg");
    env.events().publish((topic,), player.clone());
}

/// Emit a badge-minted event.
pub fn emit_badge_minted(env: &Env, owner: &Address, token_id: u32) {
    let topic: Symbol = symbol_short!("badge_mnt");
    env.events().publish((topic,), (owner.clone(), token_id));
}

/// Emit a badge-burned event.
pub fn emit_badge_burned(env: &Env, owner: &Address, token_id: u32) {
    let topic: Symbol = symbol_short!("badge_brn");
    env.events().publish((topic,), (owner.clone(), token_id));
}

/// Emit a reward-claimed event.
pub fn emit_reward_claimed(env: &Env, tournament_id: u32, winner: &Address, amount: i128) {
    let topic: Symbol = symbol_short!("rwd_claim");
    env.events()
        .publish((topic,), (tournament_id, winner.clone(), amount));
}

/// Emit a governance-vote event.
pub fn emit_vote_cast(
    env: &Env,
    proposal_id: u32,
    voter: &Address,
    weight: i128,
    support: bool,
) {
    let topic: Symbol = symbol_short!("vote");
    env.events()
        .publish((topic,), (proposal_id, voter.clone(), weight, support));
}

/// Emit an achievement-reward-claimed event.
pub fn emit_achievement_claimed(env: &Env, player: &Address, achievement_id: u32, amount: i128) {
    let topic: Symbol = symbol_short!("ach_clm");
    env.events()
        .publish((topic,), (player.clone(), achievement_id, amount));
}

/// Emit an orchestrator end-to-end claim event.
pub fn emit_claim_completed(
    env: &Env,
    player: &Address,
    achievement_id: u32,
    token_id: u32,
    amount: i128,
) {
    let topic: Symbol = symbol_short!("claim_ok");
    env.events()
        .publish((topic,), (player.clone(), achievement_id, token_id, amount));
}

// ─── Validation Helpers ───────────────────────────────────────────────────────

/// Assert that `amount` is strictly positive. Panics with a consistent message.
pub fn require_positive(amount: i128) {
    assert!(amount > 0, "amount must be positive");
}

/// Assert that a caller is the expected admin.
pub fn require_is_admin(caller: &Address, admin: &Address) {
    assert!(caller == admin, "unauthorized");
}

/// Validate that a payout-bps `Vec` is non-empty, within slot limits,
/// and that its sum does not exceed 10 000.
pub fn validate_payout_bps(bps: &Vec<u32>) {
    assert!(bps.len() > 0, "payout_bps must not be empty");
    assert!(bps.len() <= MAX_PAYOUT_SLOTS, "too many payout slots");
    let mut total: u32 = 0;
    for b in bps.iter() {
        total += b;
    }
    assert!(total <= MAX_BPS, "payout bps exceeds 10000");
}

mod test;
