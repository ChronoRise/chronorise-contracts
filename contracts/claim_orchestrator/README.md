# Claim Orchestrator

## Purpose

The **Claim Orchestrator** is the central wiring contract in the ChronoRise protocol. It coordinates the full end-to-end achievement claim flow by invoking four downstream contracts in a single atomic transaction:

1. **zk_verifier** — validates the zero-knowledge proof and enforces nullifier replay protection.
2. **reward_pool** — releases the token reward and enforces per-achievement claim deduplication.
3. **badge_nft** — mints a soulbound NFT badge to the player.
4. **player_registry** — updates the player's profile with the claimed achievement, badge ID, and reputation points.

## Key Features

- **Atomic execution**: all four steps succeed or the entire transaction reverts.
- **Dual replay protection**: both ZK nullifiers (zk_verifier) and per-achievement claim tracking (reward_pool) prevent duplicate claims.
- **Zero on-chain gameplay data**: the proof carries no sensitive information; only the achievement ID and public inputs are recorded.
- **Soulbound badge**: the minted badge is permanently bound to the player and cannot be transferred.

## Flow Diagram

```
Player submits proof
   ↓
[1] zk_verifier.verify()
   ↓ (proof valid, nullifier unused)
[2] reward_pool.claim_achievement_reward()
   ↓ (tokens released, claim flag set)
[3] badge_nft.mint()
   ↓ (soulbound badge created)
[4] player_registry.add_claimed_reward(), add_badge(), add_reputation()
   ↓ (player state updated)
Emit claim_ok event
```

## Deployment

The orchestrator must be initialized with the addresses of the four downstream contracts and a reputation delta per claim:

```rust
orchestrator.initialize(
    admin,
    zk_verifier_address,
    reward_pool_address,
    badge_nft_address,
    player_registry_address,
    50, // reputation per claim
)
```

The orchestrator must also be granted minter rights in `badge_nft` so it can mint badges on behalf of players.

## Testing

Integration tests are in `test.rs` and use `contractimport!` to load the compiled WASMs of all four contracts. These tests verify the full end-to-end flow including:

- Token payout
- Badge minting
- Registry updates
- Replay protection (both nullifier and per-achievement)

Run tests:
```sh
make test
```
