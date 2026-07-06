# tournament_rewards

Soroban smart contract for distributing tournament prize pools in Chronorise.

## Project Structure

```text
.
├── contracts
│   └── tournament-rewards
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Creates tournament prize pools funded by entry fees or direct deposits.
- Records ranked results and distributes rewards proportionally to winners.
- Supports claiming, refunds for cancelled tournaments, and fee collection.
