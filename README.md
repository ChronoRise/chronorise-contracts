# ChronoRise Contracts

> **Privacy-first gaming rewards infrastructure — Soroban smart contracts on Stellar.**

[![Rust](https://img.shields.io/badge/rust-1.96-orange?logo=rust)](https://www.rust-lang.org/)
[![Soroban SDK](https://img.shields.io/badge/soroban--sdk-26-blueviolet)](https://docs.rs/soroban-sdk)
[![Stellar CLI](https://img.shields.io/badge/stellar--cli-27-blue?logo=stellar)](https://github.com/stellar/stellar-cli)
[![Tests](https://img.shields.io/badge/tests-154%20passing-brightgreen)](#testing)
[![License: MIT](https://img.shields.io/badge/license-MIT-green)](LICENSE)

ChronoRise lets players prove they completed game achievements using **Zero-Knowledge Proofs** — without revealing gameplay data, scores, or match history. Verified proofs trigger on-chain reward distribution, soulbound badge minting, and reputation updates, all in a single atomic Soroban transaction.

This repository contains every smart contract deployed onto Stellar as part of the ChronoRise protocol.

---

## Table of Contents

- [Contracts](#contracts)
- [Architecture](#architecture)
- [Prerequisites](#prerequisites)
- [Getting Started](#getting-started)
- [Testing](#testing)
- [Building for Deployment](#building-for-deployment)
- [Contract Addresses](#contract-addresses)
- [Security](#security)
- [Documentation](#documentation)
- [Contributing](#contributing)

---

## Contracts

```
contracts/
├── claim_orchestrator/   ← Atomic end-to-end claim wiring
├── zk_verifier/          ← ZK proof verification + nullifier replay protection
├── reward_pool/          ← Token deposits, per-achievement claim payouts
├── achievement_registry/ ← Achievement definitions and on-chain awards
├── badge_nft/            ← Soulbound (non-transferable) achievement badges
├── player_registry/      ← Player profiles, reputation, games, badges
├── treasury/             ← Multi-token treasury with spender roles
├── tournament_rewards/   ← Full tournament lifecycle and prize distribution
├── governance/           ← DAO proposals, on-chain token-weighted voting
└── shared/               ← Error codes, BPS math, validation helpers
```

| Contract | Tests | Description |
|---|---|---|
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

A single `claim_orchestrator.claim()` call executes four contract invocations atomically. If any step fails, the entire transaction reverts.

```
Player
  │
  ├── Game Client computes witness locally (nothing leaves the device)
  │
  ├── ChronoRise SDK generates ZK proof
  │
  ├── Backend validates and submits to Soroban
  │
  └── claim_orchestrator.claim()
        │
        ├─ [1] zk_verifier.verify()
        │       Validates Groth16 proof
        │       Checks SHA-256 nullifier (replay protection)
        │
        ├─ [2] reward_pool.claim_achievement_reward()
        │       Releases XLM / USDC / custom token
        │       Sets per-player per-achievement claim flag
        │
        ├─ [3] badge_nft.mint()
        │       Mints soulbound badge — permanently bound to player
        │
        └─ [4] player_registry.update()
                Records claimed achievement, badge ID, reputation delta
```

### Dual Replay Protection

| Layer | Where | Mechanism |
|---|---|---|
| ZK nullifier | `zk_verifier` | SHA-256 of proof bytes stored permanently |
| Achievement claim flag | `reward_pool` | `AchievementClaim(player, achievement_id)` key |

Both layers are independent. Bypassing one does not bypass the other.

### Storage Tiers

| Data | Storage Tier | Rationale |
|---|---|---|
| Admin, counters, config | `instance` | Cheap, always loaded with contract |
| Player profiles, awards, records | `persistent` | Long-lived, user-specific |
| Nullifiers, claim flags | `persistent` | Must survive ledger archival |

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| wasm32 target | — | `rustup target add wasm32-unknown-unknown` |
| Stellar CLI | 27.x | `cargo install --locked stellar-cli --features opt` |

Verify your setup:

```sh
rustc --version          # rustc 1.96.0 or newer
stellar --version        # stellar 27.x
rustup target list --installed | grep wasm32
```

---

## Getting Started

```sh
# Clone the repo
git clone https://github.com/ChronoRise/chronorise-contracts.git
cd chronorise-contracts
```

Each contract is an independent Cargo workspace under `contracts/<name>/`. Navigate into any one to work with it:

```sh
cd contracts/reward_pool
```

---

## Testing

Every contract has a `test.rs` alongside its `lib.rs`. Tests run entirely in the Soroban sandbox — no network connection required.

### Run all tests for a single contract

```sh
cd contracts/<contract_name>
cargo test
```

### Run all contracts in sequence

```sh
for contract in shared reward_pool achievement_registry zk_verifier \
                player_registry badge_nft treasury tournament_rewards governance; do
  echo "── $contract ──"
  (cd contracts/$contract && cargo test)
done
```

### Test output example

```
running 16 tests
test test::test_initialize ... ok
test test::test_deposit_and_balance ... ok
test test::test_claim_achievement_reward_success ... ok
test test::test_double_claim_achievement_reward_panics - should panic ... ok
...
test result: ok. 16 passed; 0 failed
```

### Test snapshots

Soroban generates ledger state snapshots in `test_snapshots/` during test runs. These are committed to track contract behaviour over time. If a contract change alters observable state, the snapshot will diff — a useful regression signal.

---

## Building for Deployment

Build a release WASM binary for a contract:

```sh
cd contracts/<contract_name>

cargo build \
  --target wasm32-unknown-unknown \
  --release \
  --manifest-path contracts/<contract_inner>/Cargo.toml
```

The optimised WASM will be at:

```
target/wasm32-unknown-unknown/release/<contract_name>.wasm
```

Optimise further with `stellar contract optimize`:

```sh
stellar contract optimize \
  --wasm target/wasm32-unknown-unknown/release/<contract_name>.wasm
```

---

## Deploying to Testnet

```sh
# Fund a deployer account on testnet
stellar keys generate deployer --network testnet
stellar keys fund deployer --network testnet

# Deploy a contract
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/<contract>.wasm \
  --source deployer \
  --network testnet

# Initialise (example: reward_pool)
stellar contract invoke \
  --id <CONTRACT_ADDRESS> \
  --source deployer \
  --network testnet \
  -- initialize \
  --admin <ADMIN_ADDRESS> \
  --reward_token <TOKEN_ADDRESS>
```

### Deployment order

Contracts must be deployed in dependency order — downstream contracts first:

```
1. shared          (library — no deployment needed)
2. zk_verifier
3. reward_pool
4. achievement_registry
5. badge_nft
6. player_registry
7. treasury
8. tournament_rewards
9. governance
10. claim_orchestrator  ← deployed last, takes addresses of all above
```

After deploying `claim_orchestrator`, grant it minter rights in `badge_nft`:

```sh
stellar contract invoke \
  --id <BADGE_NFT_ADDRESS> \
  --source deployer \
  --network testnet \
  -- add_minter \
  --address <CLAIM_ORCHESTRATOR_ADDRESS>
```

---

## Contract Addresses

| Network | Status |
|---|---|
| Mainnet | Not deployed |
| Testnet | Coming soon |
| Futurenet | Coming soon |

---

## Security

### Design principles

- **No gameplay data on-chain** — `zk_verifier` stores only proof hashes and nullifiers, never game state.
- **Soulbound badges** — `badge_nft` has no `transfer` function; badges are permanently bound to the minting address.
- **On-chain voting weights** — `governance.vote()` reads the voter's token balance directly from the chain; weight cannot be self-reported.
- **Dual replay protection** — ZK nullifiers and per-achievement claim flags are independent guards.
- **Admin-only writes** — all state-mutating operations outside of player auth (claim, vote, enter tournament) require admin or role-based authorization.
- **Atomic orchestration** — `claim_orchestrator` ensures all four steps succeed together or none are applied.

### Known limitations

- `zk_verifier.verify_proof_internal()` is currently a **structural stub** — it accepts any non-empty proof. A real Groth16 / PLONK pairing check requires BLS12-381 host functions, which are pending in the Soroban host. The backend performs pre-verification using a native ZK library before submitting.
- `governance` does not yet lock tokens during the voting period. A voter could transfer tokens after voting, inflating effective supply. Token locking will be added in a future release.

### Reporting vulnerabilities

Please open a private security advisory on GitHub rather than a public issue.

---

## Documentation

| Document | Description |
|---|---|
| [`project.md`](./project.md) | Full protocol specification — all three repositories |
| [`Flow.md`](./Flow.md) | Backend and web application flows, API reference, worker schedule |
| [`contracts/claim_orchestrator/README.md`](./contracts/claim_orchestrator/README.md) | Orchestrator deployment and integration guide |

---

## Repository Structure

```
chronorise-contracts/
├── contracts/
│   ├── claim_orchestrator/
│   │   └── contracts/claim-orchestrator/
│   │       ├── src/lib.rs
│   │       └── src/test.rs
│   ├── zk_verifier/
│   ├── reward_pool/
│   ├── achievement_registry/
│   ├── badge_nft/
│   ├── player_registry/
│   ├── treasury/
│   ├── tournament_rewards/
│   ├── governance/
│   └── shared/
├── README.md       ← you are here
├── project.md      ← protocol specification
└── Flow.md         ← backend + web flows
```

Each contract workspace follows the same layout:

```
contracts/<name>/
├── Cargo.toml              ← workspace manifest
├── Cargo.lock
├── .gitignore
├── README.md
└── contracts/<name>/
    ├── Cargo.toml          ← crate manifest
    ├── Makefile
    └── src/
        ├── lib.rs          ← contract implementation
        └── test.rs         ← unit tests
```

---

## Contributing

1. Fork the repository and create a feature branch.
2. Write or update tests — all 154 must continue to pass.
3. Follow the existing commit convention: `type(scope): description`  
   Types: `feat`, `fix`, `test`, `docs`, `chore`, `refactor`
4. Open a pull request against `main` with a clear description of the change.

---

## Licence

MIT — see [LICENSE](LICENSE).

---

> Part of the [ChronoRise](https://github.com/ChronoRise) organisation.  
> Backend: [`chronorise-backend`](https://github.com/ChronoRise/chronorise-backend) · Web: [`chronorise-web`](https://github.com/ChronoRise/chronorise-web)
