# ChronoRise — Application Flow

> Detailed flow documentation for `chronorise-backend` and `chronorise-web`.  
> For smart contract internals see [`project.md`](./project.md).

---

## Table of Contents

1. [System Overview](#system-overview)
2. [Authentication Flow](#authentication-flow)
3. [Game Registration Flow](#game-registration-flow)
4. [Achievement Claim Flow](#achievement-claim-flow)
5. [Tournament Flow](#tournament-flow)
6. [Governance Flow](#governance-flow)
7. [Reward Distribution Flow](#reward-distribution-flow)
8. [Notification Flow](#notification-flow)
9. [Backend Module Responsibilities](#backend-module-responsibilities)
10. [Web App Page Flows](#web-app-page-flows)
11. [API Overview](#api-overview)
12. [Background Workers](#background-workers)
13. [Data Storage Strategy](#data-storage-strategy)

---

## System Overview

```
┌──────────────────────────────────────────────────────────┐
│                     chronorise-web                        │
│         Next.js  ·  TypeScript  ·  Wallet Kit             │
└───────────────────────┬──────────────────────────────────┘
                        │  REST / GraphQL / WebSocket
┌───────────────────────▼──────────────────────────────────┐
│                   chronorise-backend                       │
│         Rust  ·  Axum  ·  Postgres  ·  Redis  ·  NATS    │
│                                                           │
│  ┌────────┐  ┌──────┐  ┌─────────┐  ┌─────────────────┐ │
│  │  auth  │  │  zk  │  │ stellar │  │ rewards/achieve │ │
│  └────────┘  └──────┘  └─────────┘  └─────────────────┘ │
└───────────────────────┬──────────────────────────────────┘
                        │  Stellar RPC / Soroban
┌───────────────────────▼──────────────────────────────────┐
│               chronorise-contracts                         │
│   claim_orchestrator · zk_verifier · reward_pool          │
│   badge_nft · player_registry · treasury · governance     │
└──────────────────────────────────────────────────────────┘
```

---

## Authentication Flow

Players authenticate with their Stellar wallet. No username or password required.

```
Player opens chronorise-web
        │
        ▼
Connect Wallet  (Freighter / Albedo / WalletConnect via Wallet Kit)
        │
        ▼
Backend: POST /auth/challenge
  └─ Generate a random nonce tied to the wallet address
  └─ Store nonce in Redis with 5-min TTL
        │
        ▼
Frontend: Sign nonce with wallet private key
        │
        ▼
Backend: POST /auth/verify
  └─ Verify signature against wallet public key
  └─ Confirm nonce exists and is unexpired
  └─ Issue JWT (short-lived) + refresh token (long-lived)
  └─ Create player session in Redis
        │
        ▼
Frontend: Store JWT in memory, refresh token in HttpOnly cookie
        │
        ▼
All subsequent API calls include Authorization: Bearer <jwt>
```

**Supported auth methods:**

| Method | Flow |
|---|---|
| Stellar Wallet | Sign challenge nonce (primary) |
| Passkeys | WebAuthn credential linked to wallet address |
| OAuth | Google / Discord → link to wallet address |

**Session lifecycle:**
- JWT expires after 15 minutes
- Refresh token renews the JWT silently via `/auth/refresh`
- Logout invalidates the refresh token in Redis

---

## Game Registration Flow

Developers register their game through the Developer Portal before any achievements can be created.

```
Developer opens Developer Portal
        │
        ▼
POST /games/register
  └─ Submit: game name, description, website, webhook URL
  └─ Backend validates inputs
  └─ Backend creates game record in Postgres
  └─ Backend generates API key (stored as bcrypt hash)
  └─ Returns: game_id, api_key (shown once)
        │
        ▼
Developer configures webhook endpoint in their game server
        │
        ▼
POST /games/{game_id}/achievements
  └─ Submit: achievement name, description, difficulty, reward amount,
             circuit ID (references a registered ZK circuit in zk_verifier),
             badge type ID
  └─ Backend stores achievement definition
  └─ Backend calls achievement_registry.register_achievement() on-chain
  └─ Returns: achievement_id
        │
        ▼
Developer downloads SDK + embeds game_id and achievement definitions
```

---

## Achievement Claim Flow

The core flow. Entirely private — no gameplay data leaves the player's device.

```
 GAME CLIENT                 BACKEND                    SOROBAN
      │                         │                          │
      │  Player triggers         │                          │
      │  achievement condition   │                          │
      │                         │                          │
      ▼                         │                          │
 [1] Compute witness locally     │                          │
     (inside the game)          │                          │
      │                         │                          │
      ▼                         │                          │
 [2] SDK: generate ZK proof      │                          │
     from witness + circuit      │                          │
      │                         │                          │
      ▼                         │                          │
 [3] POST /achievements/claim ──►│                          │
     { proof, public_inputs,     │                          │
       achievement_id,           │                          │
       player_address }          │                          │
                                 │                          │
                                [4] Validate request        │
                                    Check JWT               │
                                    Check achievement exists │
                                    Check not already claimed│
                                    (Postgres fast-path)    │
                                         │                  │
                                        [5] Pre-verify proof│
                                            (backend ZK lib)│
                                         │                  │
                                        [6] Build Soroban tx│
                                            claim_orchestrator
                                            .claim(...)  ──►│
                                                            │
                                                           [7] zk_verifier.verify()
                                                               nullifier check
                                                            │
                                                           [8] reward_pool.claim()
                                                               token transfer
                                                            │
                                                           [9] badge_nft.mint()
                                                               soulbound badge
                                                            │
                                                          [10] player_registry.update()
                                                               rep + badge + claim
                                                            │
                                         ◄──────────────────│
                                        [11] Index tx events │
                                             Update Postgres  │
                                             Invalidate Redis │
                                         │                  │
              ◄──────────────────────────│                  │
 [12] Return claim result               │                  │
      { reward_amount, badge_id,        │                  │
        tx_hash, reputation_delta }     │                  │
      │                                 │                  │
      ▼                                 │                  │
 Update UI (badges, rewards)            │                  │
 Send notification ◄────────────────────│                  │
```

### Fraud & Anti-Spam Guards (Backend Layer)

Before touching the chain, the backend enforces:

| Guard | Mechanism |
|---|---|
| JWT auth | Request must carry a valid JWT |
| Achievement exists | Postgres lookup |
| Already claimed (fast-path) | Redis / Postgres check before any Soroban call |
| Rate limiting | Per-player per-achievement cooldown window in Redis |
| Proof pre-validation | Backend ZK library validates proof structure before submitting |
| Anti-spam | IP + wallet rate limiting via Redis sliding window |

Even if all backend guards are bypassed, the contracts enforce their own independent checks (nullifier + claim flag).

---

## Tournament Flow

```
ADMIN / GAME DEV              BACKEND                   SOROBAN
        │                        │                         │
        ▼                        │                         │
POST /tournaments/create ───────►│                         │
  { reward_token, entry_fee,     │                         │
    payout_bps, name }           │                         │
                                [1] Validate payout BPS    │
                                [2] tournament_rewards      │
                                    .create_tournament() ──►│
                                                           [3] Store on-chain
                                ◄──────────────────────────│
                                [4] Store metadata         │
                                    in Postgres             │
        │                        │                         │
        ▼                        │                         │
Tournament visible on web        │                         │

─────────────────── PLAYER ENTERS ────────────────────────

Player clicks "Enter"            │                         │
        │                        │                         │
POST /tournaments/{id}/enter ───►│                         │
                                [5] Verify player JWT      │
                                [6] tournament_rewards     │
                                    .enter(player) ────────►│
                                                           [7] Deduct entry fee
                                                               Update pool
                                ◄──────────────────────────│
                                [8] Update Postgres        │
                                [9] Notify player          │

─────────────────── TOURNAMENT RUNS ───────────────────────

Admin finalises results          │                         │
        │                        │                         │
POST /tournaments/{id}/finalise ►│                         │
  { ranked_winners }             │                         │
                                [10] tournament_rewards    │
                                     .finalise(winners) ───►│
                                                           [11] Set status
                                                                Store ranked list
                                ◄──────────────────────────│
                                [12] Update Postgres       │
                                [13] Notify all entrants   │

─────────────────── PLAYER CLAIMS PRIZE ───────────────────

Player clicks "Claim Prize"      │                         │
        │                        │                         │
POST /tournaments/{id}/claim ───►│                         │
                                [14] tournament_rewards    │
                                     .claim_reward() ──────►│
                                                           [15] Calculate BPS payout
                                                                Transfer tokens
                                ◄──────────────────────────│
                                [16] Update Postgres       │
                                [17] Notify player         │
```

---

## Governance Flow

```
TOKEN HOLDER                  BACKEND                    SOROBAN
      │                          │                          │
      ▼                          │                          │
POST /governance/propose ───────►│                          │
  { title, description }         │                          │
                                [1] Verify JWT              │
                                [2] governance.propose() ──►│
                                                           [3] Store proposal
                                                               Set voting window
                                ◄──────────────────────────│
                                [4] Store in Postgres       │
                                [5] Announce in Discord     │
      │                          │                          │
      ▼                          │                          │
POST /governance/{id}/vote ─────►│                          │
  { support: true/false }        │                          │
                                [6] Verify JWT              │
                                [7] governance.vote() ─────►│
                                                           [8] Read token balance
                                                               on-chain (no weight
                                                               parameter — chain
                                                               is the source of truth)
                                                           [9] Record vote + weight
                                ◄──────────────────────────│
                                [10] Update Postgres        │
      │                          │                          │
      ▼  (voting window ends)     │                          │
Anyone calls:                    │                          │
POST /governance/{id}/finalise ─►│                          │
                                [11] governance.finalise() ►│
                                                          [12] Check quorum
                                                               Check approval BPS
                                                               Set Passed/Rejected
                                ◄──────────────────────────│
                                [13] Update Postgres        │
                                [14] Notify community       │
```

---

## Reward Distribution Flow

Covers both direct achievement rewards and treasury-funded campaigns.

```
                    BACKEND WORKERS
                          │
          ┌───────────────┼───────────────┐
          │               │               │
          ▼               ▼               ▼
   RewardWorker    TreasuryMonitor   LeaderboardWorker
          │               │               │
          │         Polls treasury        │
          │         balances via          │
          │         Stellar RPC           │
          │               │               │
          ▼               ▼               ▼
   Process pending   Alert if low    Update Redis
   reward queue      balance         leaderboard cache
   from NATS         threshold hit
          │
          ▼
   For each pending reward:
     1. Check eligibility (Postgres)
     2. Check cooldown (Redis)
     3. Build Soroban transaction
     4. Sign with backend keypair
     5. Submit via Stellar RPC
     6. Wait for confirmation
     7. Update Postgres record
     8. Publish notification event to NATS
          │
          ▼
   NotificationWorker consumes NATS event
   → Email / Discord / Telegram / Push
```

---

## Notification Flow

```
Event source (claim, tournament result, governance vote, etc.)
        │
        ▼
Publish to NATS topic:
  chronorise.notifications.{player_id}
        │
        ▼
NotificationWorker consumes message
        │
        ├─── Email?      → Send via SMTP / Resend
        ├─── Discord?    → Discord webhook
        ├─── Telegram?   → Telegram Bot API
        ├─── Push?       → Web Push / FCM
        └─── Wallet?     → Stellar memo / SEP notification
        │
        ▼
Mark notification as delivered in Postgres
```

Player notification preferences are stored in Postgres and cached in Redis. Workers check preferences before dispatching each channel.

---

## Backend Module Responsibilities

### `src/api/`
Axum router. Exposes:
- **REST** — CRUD for games, achievements, players, tournaments
- **GraphQL** — flexible queries for dashboard data and analytics
- **WebSocket** — real-time updates (live tournament standings, claim status)

### `src/auth/`
- Wallet challenge/verify (nonce-based)
- Passkey (WebAuthn) registration and assertion
- OAuth callback and wallet linking
- JWT issuance, refresh, revocation
- Session management in Redis

### `src/zk/`
- Witness generation from game events
- Proof generation using RISC Zero / SP1 / Halo2 / Noir
- Proof pre-validation before on-chain submission
- Circuit management (upload, version, activate/deactivate)

### `src/stellar/`
- Transaction building and signing
- Transaction submission via Stellar RPC
- Contract read calls (balance queries, state reads)
- Event subscription and indexing
- Fee bump logic for sponsored transactions

### `src/rewards/`
- Reward eligibility rules engine
- Cooldown window enforcement (Redis TTL keys)
- Anti-spam and duplicate prevention
- Fraud detection heuristics

### `src/achievements/`
Achievement engine pipeline:
```
Game Event received
  → Validate achievement definition exists
  → Retrieve circuit for the achievement
  → Generate witness (game-provided data)
  → Generate ZK proof (zk module)
  → Store proof in Postgres (pending)
  → Player calls /claim → trigger on-chain flow
```

### `src/tournaments/`
- Tournament creation, lifecycle management
- Private leaderboard calculation (ZK-based rank proofs)
- Prize pool tracking
- Rank verification

### `src/analytics/`
Tracks (privacy-preserving, aggregated only):
- Total proofs submitted / verified
- Reward volume by token type
- Active games and player counts
- Claim success/failure rates

### `src/workers/`

| Worker | Trigger | Job |
|---|---|---|
| `RewardWorker` | NATS queue | Process pending reward payouts |
| `ProofWorker` | NATS queue | Async proof generation for high-load |
| `LeaderboardWorker` | Cron (30s) | Refresh Redis leaderboard cache |
| `TreasuryMonitor` | Cron (5min) | Poll treasury balances, alert on low funds |
| `EventIndexer` | Stellar RPC stream | Index contract events into Postgres |
| `NotificationWorker` | NATS queue | Fan-out notifications to all channels |

---

## Web App Page Flows

### Dashboard

```
Player lands on /dashboard
  │
  ▼
Fetch via GraphQL:
  - player profile (player_registry)
  - claimable rewards (reward_pool)
  - badge list (badge_nft)
  - recent tournament results
  - reputation score
  │
  ▼
Render:
  ┌─────────────────────────────────────────┐
  │  Total Rewards  │  Badges  │  Rep Score │
  ├─────────────────┴──────────┴────────────┤
  │  Games Played   │  Privacy Score        │
  ├─────────────────────────────────────────┤
  │  Recent Activity Feed                   │
  └─────────────────────────────────────────┘
```

### Rewards Page

```
/rewards
  │
  ├─ Pending Rewards   → GET /rewards/pending
  │                      Shows unclaimed rewards, proof status
  │
  ├─ Claimed History   → GET /rewards/history
  │                      Paginated claim records with tx hashes
  │
  └─ Reward Pools      → GET /rewards/pools
                         Active pools, token types, expiry
```

### Achievements Page

```
/achievements
  │
  ├─ Unlocked         → GET /achievements/mine
  │                     Shows badge + ledger timestamp
  │                     Hidden: which game, score, or method
  │
  ├─ Available        → GET /achievements/available
  │                     What can be claimed (no spoilers on locked ones)
  │
  └─ Claim Flow       → POST /achievements/claim
                         Opens proof submission modal
                         SDK handles proof generation in background
                         Progress indicator while Soroban tx confirms
```

### Tournaments Page

```
/tournaments
  │
  ├─ Browse           → GET /tournaments?status=open
  │
  ├─ Enter            → POST /tournaments/{id}/enter
  │                      Wallet approves entry fee transfer
  │
  ├─ Leaderboard      → GET /tournaments/{id}/leaderboard
  │                      Private rankings (ZK proofs of rank)
  │
  └─ Claim Prize      → POST /tournaments/{id}/claim
                          Wallet signs claim transaction
```

### Developer Portal

```
/developer
  │
  ├─ Register Game    → POST /games/register
  │                      Returns api_key (shown once — save it)
  │
  ├─ Achievements     → POST /games/{id}/achievements
  │                      Name, description, circuit ID, badge type, reward amount
  │
  ├─ Circuits         → POST /zk/circuits/upload
  │                      Upload circuit WASM / proving key
  │                      Backend registers in zk_verifier contract
  │
  ├─ API Keys         → GET/POST/DELETE /developer/keys
  │
  ├─ Webhooks         → POST /developer/webhooks
  │                      Events: achievement_claimed, reward_distributed,
  │                              player_joined, tournament_ended
  │
  └─ SDK Downloads    → Static links to Unity / Godot / JS / Rust SDKs
```

### DAO Page

```
/dao
  │
  ├─ Browse Proposals  → GET /governance/proposals
  │
  ├─ Create Proposal   → POST /governance/propose
  │                       Requires wallet auth + minimum token balance
  │
  ├─ Vote              → POST /governance/{id}/vote
  │                       Wallet signs vote tx
  │                       Weight = on-chain token balance (automatic)
  │
  └─ Treasury View     → GET /treasury/stats
                          Per-token balances, deposit/disburse history
```

---

## API Overview

### Auth
| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/auth/challenge` | Request nonce for wallet signing |
| `POST` | `/auth/verify` | Verify signed nonce, issue JWT |
| `POST` | `/auth/refresh` | Refresh JWT using refresh token |
| `POST` | `/auth/logout` | Revoke refresh token |

### Players
| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/players/me` | Current player profile |
| `PATCH` | `/players/me` | Update display preferences |
| `GET` | `/players/{address}` | Public profile by wallet address |

### Games & Achievements
| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/games/register` | Register a new game |
| `GET` | `/games` | List all approved games |
| `POST` | `/games/{id}/achievements` | Create achievement definition |
| `POST` | `/achievements/claim` | Submit proof and claim reward |
| `GET` | `/achievements/mine` | Player's claimed achievements |

### Tournaments
| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/tournaments` | Create tournament (admin/dev) |
| `GET` | `/tournaments` | List tournaments |
| `POST` | `/tournaments/{id}/enter` | Enter a tournament |
| `POST` | `/tournaments/{id}/finalise` | Submit ranked results (admin) |
| `POST` | `/tournaments/{id}/claim` | Claim prize |
| `POST` | `/tournaments/{id}/refund` | Claim refund after cancellation |

### Governance
| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/governance/propose` | Create proposal |
| `GET` | `/governance/proposals` | List proposals |
| `POST` | `/governance/{id}/vote` | Cast vote |
| `POST` | `/governance/{id}/finalise` | Finalise after voting period |

### Treasury
| Method | Endpoint | Description |
|---|---|---|
| `GET` | `/treasury/stats` | Per-token stats |
| `GET` | `/treasury/tokens` | Supported token list |

---

## Background Workers

```
NATS Topics
───────────────────────────────────────────────────────
chronorise.rewards.pending        → RewardWorker
chronorise.proofs.queue           → ProofWorker
chronorise.notifications.{id}     → NotificationWorker
chronorise.events.stellar         → EventIndexer

Cron Jobs
───────────────────────────────────────────────────────
*/30 * * * * *   LeaderboardWorker   refresh Redis cache
*/5  * * * *     TreasuryMonitor     poll on-chain balances
*/1  * * * *     EventIndexer        pull Stellar RPC events
```

---

## Data Storage Strategy

| Data | Store | Reason |
|---|---|---|
| Player profiles, games, achievements | Postgres | Relational, durable |
| On-chain event index | Postgres | Queryable history |
| Sessions, nonces | Redis | TTL-based, fast |
| Rate limit counters | Redis | Atomic increments |
| Leaderboard cache | Redis | Sorted sets, sub-ms reads |
| Proof files, circuit WASMs | S3 / R2 | Large binary blobs |
| ZK proving keys | S3 / R2 | Version-controlled blobs |
| Job queues | NATS | Durable async messaging |
| Notification fan-out | NATS | Pub/sub to workers |

---

> All on-chain state is the **source of truth**. Postgres is a read-optimised index of chain events — never the authority. If Postgres and the chain disagree, the chain wins.
