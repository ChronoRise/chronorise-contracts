# ChronoRise

> **Privacy-first gaming rewards infrastructure powered by Stellar and Zero-Knowledge Proofs.**

---

## Vision

ChronoRise is an open-source gaming reward protocol built on Stellar. Players prove they completed achievements, reached milestones, or earned rewards — without ever revealing gameplay history, scores, match logs, or personal game data.

Instead of uploading gameplay or trusting centralised servers, games generate cryptographic proofs that verify achievements. Players submit only the proof to claim their reward.

**This enables:**

- 🔒 Private achievements
- 🛡️ Anti-cheat reward verification
- 🌐 Cross-game reputation
- 🔗 Cross-chain compatible identities
- 💸 On-chain reward distribution through Stellar
- 🏆 Verifiable tournaments

---

## Organisation Structure

```
ChronoRise/
├── chronorise-contracts/   ← Smart contracts (this repo)
├── chronorise-backend/     ← Rust/Axum API + ZK services
└── chronorise-web/         ← Next.js dashboard + developer portal
```

---

## Repository 1 — `chronorise-contracts`

**Language:** Rust · Soroban  
**Purpose:** Every smart contract deployed onto Stellar.

### Directory Layout

```
chronorise-contracts/
├── contracts/
│   ├── reward_pool/
│   ├── achievement_registry/
│   ├── zk_verifier/
│   ├── player_registry/
│   ├── badge_nft/
│   ├── treasury/
│   ├── tournament_rewards/
│   ├── governance/
│   ├── claim_orchestrator/   ← end-to-end claim wiring
│   └── shared/               ← common types, error codes, helpers
├── tests/
├── scripts/
├── deploy/
└── docs/
```

### Contracts

| Contract | Responsibility |
|---|---|
| `reward_pool` | Deposit, lock, and release rewards. Per-achievement claim tracking prevents double-claiming. |
| `achievement_registry` | Registers achievement definitions and awards them to players. Deduplication enforced. |
| `zk_verifier` | Core verification. Receives a Groth16 proof + public inputs, checks the nullifier for replay protection, and records the result. Never stores gameplay data. |
| `player_registry` | Player profiles — wallet, games, reputation, claimed rewards, badges. |
| `badge_nft` | Soulbound (non-transferable) achievement NFT badges. Mint and burn only. |
| `treasury` | Holds XLM, USDC, and custom tokens. Admin and spender role management. |
| `tournament_rewards` | Full tournament lifecycle — create → enter → start → finalise → claim / refund. BPS-based prize splits. |
| `governance` | DAO — proposal, on-chain voting weighted by real token balance, quorum + approval threshold. |
| `claim_orchestrator` | Wires all contracts into a single atomic claim transaction (see flow below). |
| `shared` | Error codes, `bps_of` math, `validate_payout_bps`, event helpers, validation utilities. |

---

## Repository 2 — `chronorise-backend`

**Language:** Rust · **Framework:** Axum  
**Database:** Postgres · **Cache:** Redis · **Storage:** S3/R2 · **Queue:** NATS

### Directory Layout

```
chronorise-backend/
├── src/
│   ├── api/            ← REST, GraphQL, WebSocket
│   ├── auth/           ← Wallet login, Passkeys, OAuth, JWT
│   ├── zk/             ← Proof generation, witness generation, circuit execution
│   ├── stellar/        ← SDK, transactions, event subscriptions, contract indexing
│   ├── rewards/        ← Eligibility, cooldowns, anti-spam, fraud detection
│   ├── achievements/   ← Achievement engine (event → witness → proof → claim)
│   ├── tournaments/    ← Leaderboards, prize pools, rank verification
│   ├── games/
│   ├── players/
│   ├── analytics/
│   ├── notifications/  ← Email, Discord, Telegram, Push, Wallet
│   ├── workers/        ← Background jobs: distribution, proof gen, leaderboards
│   ├── config/
│   ├── db/
│   └── utils/
├── migrations/
├── tests/
├── Docker/
└── docs/
```

### Key Modules

**ZK Module** — generates and verifies proofs using RISC Zero, SP1, Halo2, or Noir.

**Achievement Engine**
```
Game Event → Generate Witness → Generate Proof → Store Proof → Player Claims
```

**Stellar Module** — creates, signs, and submits transactions; reads contracts; indexes events via Stellar RPC.

**Rewards Engine** — handles eligibility checks, cooldown windows, anti-spam, duplicate prevention, and fraud detection.

---

## Repository 3 — `chronorise-web`

**Framework:** Next.js · **Language:** TypeScript  
**UI:** Tailwind + Shadcn UI + Wallet Kit

### Directory Layout

```
chronorise-web/
├── app/
├── components/
├── hooks/
├── lib/
├── providers/
├── styles/
├── public/
├── sdk/
└── types/
```

### Pages

| Page | What it shows |
|---|---|
| Dashboard | Total rewards, claimable rewards, badges, games played, privacy score |
| Rewards | Pending / claimed rewards, history, reward pools, proof status |
| Achievements | Hidden & unlocked achievements, NFT badges, progress, private stats |
| Tournaments | Active tournaments, prize pools, prize structure, registration, claims |
| DAO | Proposals, voting, treasury overview |
| Developer Portal | Game registration, achievement creation, circuit upload, API keys, webhooks, SDK downloads |

---

## Architecture

### Full Claim Flow

```
Player
  │
  ▼
Game Client  (computes witness locally)
  │
  ▼
ChronoRise SDK  (generates ZK proof)
  │
  ▼
Rust Backend  (validates, forwards)
  │
  ▼
claim_orchestrator  ──────────────────────────────────────┐
  │                                                        │
  ├─ [1] zk_verifier.verify()        ← proof + nullifier  │
  ├─ [2] reward_pool.claim()         ← token payout        │
  ├─ [3] badge_nft.mint()            ← soulbound badge     │
  └─ [4] player_registry.update()   ← rep + badge + claim ┘
  │
  ▼
Player Wallet  (XLM / USDC / token + NFT badge)
```

### Stellar Components

| Stellar Feature | Usage |
|---|---|
| Soroban Smart Contracts | Achievement verification, reward claims, treasury, governance |
| Stellar Wallets | Player identity and authentication |
| XLM | Native reward payouts and fees |
| USDC on Stellar | Stablecoin rewards for tournaments and campaigns |
| Stellar Assets | In-game tokens, seasonal currencies, sponsor assets |
| Stellar Events | Contract event indexing for achievements and rewards |
| Stellar RPC | Contract reads, transaction submission, indexing |
| Stellar SEP Standards | Wallet interoperability and future ecosystem integrations |
| Soroban Authorization | Secure auth for reward claims and admin actions |

---

## Zero-Knowledge Layer

**Supported frameworks:** RISC Zero · SP1 · Noir · Halo2

A proof can assert:

> *"I completed level 20"*

without revealing: level map, enemy positions, gameplay video, player stats, or cheat-detection logic.

### Replay Protection

Every proof produces a **nullifier** (SHA-256 of proof bytes). The `zk_verifier` contract stores used nullifiers permanently — the same proof can never be submitted twice. The `reward_pool` adds a second layer with per-player per-achievement claim flags.

---

## Privacy Features

- Hidden gameplay data, scores, inventory, and player statistics
- Anonymous reward verification
- Replay-resistant proofs via one-time nullifiers
- Optional selective disclosure for tournaments and audits

---

## SDKs

| SDK | Target |
|---|---|
| ChronoRise SDK | Core integration library |
| Unity SDK | Unity game engine |
| Godot SDK | Godot game engine |
| Unreal SDK | Unreal Engine |
| JavaScript SDK | Web games |
| Rust SDK | Native / server-side |
| Mobile SDK | iOS / Android |

Developers integrate a few SDK calls to generate proofs, submit achievements, and receive reward status updates — no cryptography knowledge required.

---

## Example Achievement Flow

1. A player defeats a raid boss in a supported game.
2. The game computes the achievement witness **locally** — nothing leaves the device.
3. The SDK generates a ZK proof that the achievement conditions were met.
4. The proof and public inputs are sent to the ChronoRise backend.
5. The backend validates and forwards the proof to the `claim_orchestrator` contract.
6. The orchestrator calls `zk_verifier` — proof is verified, nullifier is checked.
7. `reward_pool` releases XLM, USDC, or a custom Stellar asset to the player's wallet.
8. `badge_nft` mints a soulbound achievement badge.
9. `player_registry` records the claim and awards reputation points.

---

## Roadmap

### Phase 1 — Core Infrastructure *(current)*
- [x] ChronoRise organisation and repositories
- [x] Soroban smart contracts (8 contracts + orchestrator)
- [x] Rust backend services
- [x] Web dashboard
- [x] Stellar wallet integration
- [x] ZK proof verification with nullifier replay protection
- [x] XLM reward distribution

### Phase 2 — Game Ecosystem
- [ ] Unity, Unreal, Godot, and JavaScript SDKs
- [ ] Tournament engine
- [ ] Private leaderboards
- [ ] USDC reward campaigns
- [ ] Sponsor-funded reward pools

### Phase 3 — Advanced Privacy
- [ ] Cross-game reputation with ZK proofs
- [ ] Selective disclosure for esports events
- [ ] Private seasonal progression
- [ ] Cross-title achievement aggregation
- [ ] Verifiable anti-cheat attestations

### Phase 4 — Decentralised Governance
- [ ] Community DAO (live on-chain)
- [ ] Community-funded reward pools
- [ ] Third-party game onboarding
- [ ] Public marketplace for reusable achievement circuits

---

> ChronoRise combines Soroban smart contracts, Rust backend services, zero-knowledge proofs, and Stellar's fast low-cost payments to deliver **private, verifiable, and scalable** reward distribution for modern games.
