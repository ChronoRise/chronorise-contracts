# shared

Shared types, constants, and utilities used across Chronorise Soroban contracts.

## Project Structure

```text
.
├── contracts
│   └── shared
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Common `contracttype` structs (e.g. `Rank`, `TournamentStatus`, `ErrorCode`).
- Shared error codes and constants.
- Helper functions reusable across contracts without duplicating logic.
