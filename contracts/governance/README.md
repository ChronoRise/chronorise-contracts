# governance

Soroban smart contract for on-chain governance of the Chronorise protocol.

## Project Structure

```text
.
├── contracts
│   └── governance
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Token-weighted proposal and voting system.
- Configurable voting period, quorum, and approval threshold.
- Executed proposals can call arbitrary contract functions via cross-contract invocation.
