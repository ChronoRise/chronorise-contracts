# ChronoRise Contracts

> **Privacy-first gaming rewards infrastructure — Soroban smart contracts on Stellar.**

[![Rust](https://img.shields.io/badge/rust-1.96-orange?logo=rust)](https://www.rust-lang.org/)
[![Soroban SDK](https://img.shields.io/badge/soroban--sdk-26-blueviolet)](https://docs.rs/soroban-sdk)
[![Stellar CLI](https://img.shields.io/badge/stellar--cli-27-blue?logo=stellar)](https://github.com/stellar/stellar-cli)
[![Tests](https://img.shields.io/badge/tests-154%20passing-brightgreen)](#testing)
[![Network](https://img.shields.io/badge/network-testnet-yellow)](#deployed-contracts)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

ChronoRise lets players prove they completed game achievements using **Zero-Knowledge Proofs** — without revealing gameplay data, scores, or match history. Verified proofs trigger on-chain reward distribution, soulbound badge minting, and reputation updates, all in a single atomic Soroban transaction.

This repository contains every smart contract deployed as part of the ChronoRise protocol.

---

## Table of Contents

- [Overview](#overview)
- [Contracts](#contracts)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Testing](#testing)
- [Building & Deploying](#building--deploying)
- [Deployed Contracts](#deployed-contracts)
- [Security](#security)
- [Contributing](#contributing)

---

## Overview

```
ChronoRise/
├── chronorise-contracts/   ← you are here
├── chronorise-backend/     ← Rust/Axum API + ZK services
└── chronorise-web/         ← Next.js dashboard + developer portal
```

The contracts layer is entirely self-contained. All state-changing operations enforce on-chain authorization independent of the backend — the backend is a convenience layer, not a trust boundary.

---

## Contracts

```
contracts/
├── claim_orchestrator/   ← atomic end-to-end claim wiring
├── zk_verifier/          ← ZK proof verification + nullifier replay protection
├── reward_pool/          ← token deposits, per-achievement claim payouts
├── achievement_registry/ ← achievement definitions and on-chain awards
├── badge_nft/            ← soulbound (non-transferable) achievement badges
├── player_registry/      ← player profiles, reputation, games, badges
├── treasury/             ← multi-token treasury with spender roles
├── tournament_rewards/   ← full tournament lifecycle and prize distribution
├── governance/           ← DAO proposals, on-chain token-weighted voting
└── shared/               ← error codes, BPS math, validation helpers
```

| Contract | Tests | Description |
|---|:---:|---|
| `claim_orchestrator` | — | Wires all contracts into one atomic claim transaction |
| `zk_verifier` | 10 | Groth16 proof verification, SHA-256 nullifier replay protection |
| `reward_pool` | 16 | Deposit/withdraw, per-achievement claim tracking |
| `achievement_registry` | 11 | Achievement definitions, awarder roles, deduplication |
| `badge_nft` | 18 | Soulbound NFT badges — mint and burn only, no transfer |
| `player_registry` | 22 | Profiles, username index, reputation, games, claims, badges |
| `treasury` | 16 | Multi-token hold, deposit/disburse, spender roles |
| `tournament_rewards` | 23 | Create → enter → start → finalise → claim / refund |
| `governance` | 19 | Propose → vote (on-chain balance) → quorum check → execute |
| `shared` | 19 | `bps_of`, `validate_payout_bps`, error codes, event helpers |

**154 tests total — all passing.**

---

## Architecture

### Claim Flow

A single `claim_orchestrator.claim()` call executes four contract invocations atomically. If any step fails, the entire transaction reverts — no partial state is possible.

```
Player
  │
  ├─ Game client computes witness locally (nothing leaves the device)
  ├─ ChronoRise SDK generates ZK proof { a, b, c }
  ├─ Backend pre-validates and builds the Soroban transaction
  │
  └─ claim_orchestrator.claim(
         player, circuit_id,
         proof { a, b, c }, public_inputs,
         achievement_id, reward_amount, badge_type_id
     )
         │
         ├─ [1] zk_verifier.verify()
         │       Validates proof structure
         │       Nullifier = SHA-256(a ++ b ++ c) — checked and stored
         │
         ├─ [2] reward_pool.claim_achievement_reward()
         │       Releases XLM / USDC / custom token to player
         │       Sets AchievementClaim(player, achievement_id) flag
         │
         ├─ [3] badge_nft.mint()
         │       Mints soulbound badge → token_id
         │       Permanently bound to player address
         │
         └─ [4] player_registry updates
                 add_claimed_reward(player, achievement_id)
                 add_badge(player, token_id)
                 add_reputation(player, delta)
```

Emits: `claim_ok(player, achievement_id, token_id, reward_amount)`

### Dual Replay Protection

Two independent guards — both must pass for a claim to succeed:

| Layer | Contract | Mechanism |
|---|---|---|
| ZK nullifier | `zk_verifier` | `SHA-256(a ++ b ++ c)` stored permanently under `DataKey::Nullifier` |
| Achievement flag | `reward_pool` | `DataKey::AchievementClaim(player, achievement_id)` set on first claim |

Bypassing one does not bypass the other.

### Storage Tiers

| Data | Tier | Rationale |
|---|---|---|
| Admin, counters, config | `instance` | Cheap, always loaded with contract |
| Player profiles, awards, records | `persistent` | Long-lived, user-specific |
| Nullifiers, claim flags | `persistent` | Must survive ledger archival |

### Deployment Order

Contracts must be deployed in dependency order:

```
1. shared               (library — no deployment)
2. zk_verifier
3. reward_pool
4. achievement_registry
5. badge_nft
6. player_registry
7. treasury
8. tournament_rewards
9. governance
10. claim_orchestrator  ← deployed last, receives all contract addresses
```

After deploying `claim_orchestrator`, grant it minter rights in `badge_nft`:

```sh
stellar contract invoke \
  --id <BADGE_NFT_ID> \
  --source deployer \
  --network testnet \
  -- add_minter \
  --address <CLAIM_ORCHESTRATOR_ID>
```

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | 27.x | `cargo install --locked stellar-cli --features opt` |

Verify:

```sh
rustc --version          # rustc 1.96.0 or newer
stellar --version        # stellar 27.x
rustup target list --installed | grep wasm32
```

---

## Getting Started

```sh
git clone https://github.com/ChronoRise/chronorise-contracts.git
cd chronorise-contracts
```

Each contract is an independent Cargo workspace under `contracts/<name>/`. Navigate into any one:

```sh
cd contracts/reward_pool
cargo test
```

---

## Testing

Every contract has a `test.rs` alongside its `lib.rs`. Tests run in the Soroban sandbox — no network required.

### Run a single contract

```sh
cd contracts/<contract_name>
cargo test
```

### Run all contracts

```sh
for contract in shared reward_pool achievement_registry zk_verifier \
                player_registry badge_nft treasury tournament_rewards \
                governance claim_orchestrator; do
  echo "── $contract ──"
  (cd contracts/$contract && cargo test)
done
```

### Test snapshots

Soroban generates ledger state snapshots in `test_snapshots/` during test runs. These are committed to track contract behaviour over time — a change that alters observable state will show up as a snapshot diff.

---

## Building & Deploying

### Build a contract

```sh
cd contracts/<contract_name>
cargo build \
  --target wasm32-unknown-unknown \
  --release
```

Optimise with the Stellar CLI:

```sh
stellar contract optimize \
  --wasm target/wasm32-unknown-unknown/release/<contract_name>.wasm
```

### Deploy to testnet

```sh
# Generate and fund a deployer account
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

# Deploy
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/<contract_name>.wasm \
  --source deployer \
  --network testnet

# Initialise (example: reward_pool)
stellar contract invoke \
  --id <CONTRACT_ID> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --reward_token <TOKEN_ADDRESS>
```

---

## Deployed Contracts

**Network:** Stellar Testnet · **Deployed:** 2026-07-08

| Contract | Contract ID |
|---|---|
| `claim_orchestrator` | [`CBZMSLUGBS5DSHI2VQ6HICVXGCBLAZQXWN6AZHJCWGW3ZA6R23BGCGHL`](https://stellar.expert/explorer/testnet/contract/CBZMSLUGBS5DSHI2VQ6HICVXGCBLAZQXWN6AZHJCWGW3ZA6R23BGCGHL) |
| `zk_verifier` | [`CBSEWCQCELKFUTD4DZATPHDGZR7YHZDMCKFILHY7HEWUOVRQ5XZ4WOHA`](https://stellar.expert/explorer/testnet/contract/CBSEWCQCELKFUTD4DZATPHDGZR7YHZDMCKFILHY7HEWUOVRQ5XZ4WOHA) |
| `reward_pool` | [`CA7FC3G2USG2PCWJYRCL4355ZKGQYJG5F7SX6TFZBQHEZJ3EZ4J7CGCR`](https://stellar.expert/explorer/testnet/contract/CA7FC3G2USG2PCWJYRCL4355ZKGQYJG5F7SX6TFZBQHEZJ3EZ4J7CGCR) |
| `badge_nft` | [`CCTYQLOKWXRQLNRYW75RJ3RRVUB7I2HXV7UMKFYZYGBPOEPUOLOHIAVU`](https://stellar.expert/explorer/testnet/contract/CCTYQLOKWXRQLNRYW75RJ3RRVUB7I2HXV7UMKFYZYGBPOEPUOLOHIAVU) |
| `player_registry` | [`CBL2HWTX3KOE3ZH5QEZV63XZJRJ6U34Y5NXV5M4WFD4LHWL2FN77YVR7`](https://stellar.expert/explorer/testnet/contract/CBL2HWTX3KOE3ZH5QEZV63XZJRJ6U34Y5NXV5M4WFD4LHWL2FN77YVR7) |
| `achievement_registry` | [`CAU2ZVPXM2EBXEQ4X7ADTVGN6QN2ZHHUKUG6SB22V3QBXKRAKEDFGOWH`](https://stellar.expert/explorer/testnet/contract/CAU2ZVPXM2EBXEQ4X7ADTVGN6QN2ZHHUKUG6SB22V3QBXKRAKEDFGOWH) |
| `treasury` | [`CC56QR5ZDSIZKP6FRBHFLVO54TXYZ4XJXS7VSKMCPWU4DONYCDSAGP67`](https://stellar.expert/explorer/testnet/contract/CC56QR5ZDSIZKP6FRBHFLVO54TXYZ4XJXS7VSKMCPWU4DONYCDSAGP67) |
| `tournament_rewards` | [`CBMHNUPZSPBFLZHAMXCBYZVOBNCRPP5NNTJZ3LMZA5JNODOUSJJZSOBO`](https://stellar.expert/explorer/testnet/contract/CBMHNUPZSPBFLZHAMXCBYZVOBNCRPP5NNTJZ3LMZA5JNODOUSJJZSOBO) |
| `governance` | [`CAG67XXNCR7QWZPOS6N3I77WBZOWTFAPBZRF2Z4OYW7KONECBPQ5OK3M`](https://stellar.expert/explorer/testnet/contract/CAG67XXNCR7QWZPOS6N3I77WBZOWTFAPBZRF2Z4OYW7KONECBPQ5OK3M) |

---

## Security

**Design principles:**

- **No gameplay data on-chain** — `zk_verifier` stores only proof hashes and nullifiers, never game state.
- **Soulbound badges** — `badge_nft` has no `transfer` function; badges are permanently bound to the minting address.
- **On-chain voting weights** — `governance.vote()` reads the voter's token balance directly from the chain; weight cannot be self-reported.
- **Dual replay protection** — ZK nullifiers and per-achievement claim flags are independent guards.
- **Admin-only writes** — all state-mutating operations outside player-initiated actions (claim, vote, enter tournament) require admin or role-based authorization.
- **Atomic orchestration** — `claim_orchestrator` ensures all four steps succeed together or none are applied.

**Known limitations:**

- `verify_proof_internal()` is currently a **structural stub** — it accepts any non-empty proof. A real Groth16 pairing check requires BLS12-381 host functions pending in the Soroban host. The backend performs pre-verification using a native ZK library before submitting.
- `governance` does not yet lock tokens during the voting period. Token locking will be added in a future release.

To report a vulnerability, open a private security advisory on GitHub rather than a public issue.

---

## Contributing

1. Fork the repository and create a feature branch.
2. Write or update tests — all 154 must continue to pass.
3. Follow the commit convention: `type(scope): description`
   Types: `feat`, `fix`, `test`, `docs`, `chore`, `refactor`
4. Open a pull request against `main` with a clear description of the change.

---

## Licence

MIT — see [LICENSE](LICENSE).

---

> Part of the [ChronoRise](https://github.com/ChronoRise) organisation.
> Backend: [`chronorise-backend`](https://github.com/ChronoRise/chronorise-backend) · Web: [`chronorise-web`](https://github.com/ChronoRise/chronorise-web)
