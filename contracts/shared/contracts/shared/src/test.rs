#![cfg(test)]

use super::*;
use soroban_sdk::{testutils::Address as _, Address, Env};

// ── bps_of ────────────────────────────────────────────────────────────────────

#[test]
fn test_bps_of_full() {
    assert_eq!(bps_of(1_000_000, 10_000), 1_000_000);
}

#[test]
fn test_bps_of_half() {
    assert_eq!(bps_of(1_000_000, 5_000), 500_000);
}

#[test]
fn test_bps_of_zero_bps() {
    assert_eq!(bps_of(1_000_000, 0), 0);
}

#[test]
fn test_bps_of_zero_amount() {
    assert_eq!(bps_of(0, 10_000), 0);
}

#[test]
fn test_bps_of_one_percent() {
    assert_eq!(bps_of(10_000, 100), 100);
}

#[test]
#[should_panic(expected = "bps exceeds 10000")]
fn test_bps_of_overflow_panics() {
    bps_of(100, 10_001);
}

// ── protocol_version ──────────────────────────────────────────────────────────

#[test]
fn test_protocol_version() {
    let env = Env::default();
    let v = protocol_version(&env);
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

// ── Constants ─────────────────────────────────────────────────────────────────

#[test]
fn test_max_constants() {
    assert_eq!(MAX_BPS, 10_000);
    assert_eq!(MAX_PAYOUT_SLOTS, 20);
}

#[test]
fn test_error_codes_are_unique() {
    let codes = [
        ERR_NOT_INITIALIZED,
        ERR_ALREADY_INITIALIZED,
        ERR_UNAUTHORIZED,
        ERR_NOT_FOUND,
        ERR_DUPLICATE,
        ERR_INVALID_AMOUNT,
        ERR_INVALID_STATE,
        ERR_INACTIVE,
        ERR_VERIFICATION_FAILED,
        ERR_UNSUPPORTED_TOKEN,
        ERR_BPS_OVERFLOW,
        ERR_INSUFFICIENT_BALANCE,
        ERR_ALREADY_CLAIMED,
        ERR_SOULBOUND,
        ERR_ZERO_WEIGHT,
        ERR_VOTING_ENDED,
        ERR_QUORUM_NOT_MET,
    ];
    // Check no duplicates: every pair must differ.
    for i in 0..codes.len() {
        for j in (i + 1)..codes.len() {
            assert_ne!(
                codes[i], codes[j],
                "duplicate error code at positions {} and {}",
                i, j
            );
        }
    }
}

// ── require_positive ──────────────────────────────────────────────────────────

#[test]
fn test_require_positive_passes_for_positive() {
    require_positive(1);
    require_positive(i128::MAX);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_require_positive_zero_panics() {
    require_positive(0);
}

#[test]
#[should_panic(expected = "amount must be positive")]
fn test_require_positive_negative_panics() {
    require_positive(-1);
}

// ── require_is_admin ─────────────────────────────────────────────────────────

#[test]
fn test_require_is_admin_passes_when_equal() {
    let env = Env::default();
    let addr = Address::generate(&env);
    require_is_admin(&addr, &addr);
}

#[test]
#[should_panic(expected = "unauthorized")]
fn test_require_is_admin_panics_when_different() {
    let env = Env::default();
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    require_is_admin(&a, &b);
}

// ── validate_payout_bps ───────────────────────────────────────────────────────

#[test]
fn test_validate_payout_bps_valid() {
    let env = Env::default();
    let mut bps = soroban_sdk::Vec::new(&env);
    bps.push_back(6_000_u32);
    bps.push_back(4_000_u32);
    validate_payout_bps(&bps); // should not panic
}

#[test]
fn test_validate_payout_bps_single_winner_takes_all() {
    let env = Env::default();
    let mut bps = soroban_sdk::Vec::new(&env);
    bps.push_back(10_000_u32);
    validate_payout_bps(&bps);
}

#[test]
#[should_panic(expected = "payout_bps must not be empty")]
fn test_validate_payout_bps_empty_panics() {
    let env = Env::default();
    validate_payout_bps(&soroban_sdk::Vec::new(&env));
}

#[test]
#[should_panic(expected = "too many payout slots")]
fn test_validate_payout_bps_too_many_slots_panics() {
    let env = Env::default();
    let mut bps = soroban_sdk::Vec::new(&env);
    for _ in 0..21 {
        bps.push_back(0_u32);
    }
    validate_payout_bps(&bps);
}

#[test]
#[should_panic(expected = "payout bps exceeds 10000")]
fn test_validate_payout_bps_sum_overflow_panics() {
    let env = Env::default();
    let mut bps = soroban_sdk::Vec::new(&env);
    bps.push_back(6_000_u32);
    bps.push_back(5_000_u32); // sum = 11_000 > 10_000
    validate_payout_bps(&bps);
}
