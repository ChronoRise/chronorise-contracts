# treasury

Soroban smart contract for managing the Chronorise protocol treasury.

## Project Structure

```text
.
├── contracts
│   └── treasury
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Custodies protocol-owned funds (entry fees, protocol revenue).
- Admin and governance-controlled withdrawals and allocations.
- Tracks cumulative deposits and disbursements per token.
