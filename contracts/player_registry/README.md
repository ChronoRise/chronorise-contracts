# player_registry

Soroban smart contract for registering and managing player profiles in Chronorise.

## Project Structure

```text
.
├── contracts
│   └── player-registry
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Maintains an on-chain mapping of player `Address` → `PlayerProfile`.
- Admin-controlled registration with optional self-registration support.
- Tracks player metadata: username, rank, total wins, and registration ledger.
