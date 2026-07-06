# badge_nft

Soroban smart contract for minting and managing non-fungible badge tokens in Chronorise.

## Project Structure

```text
.
├── contracts
│   └── badge-nft
│       ├── src
│       │   ├── lib.rs
│       │   └── test.rs
│       └── Cargo.toml
├── Cargo.toml
└── README.md
```

## Overview

- Mint unique badge NFTs tied to on-chain achievements or tournament outcomes.
- Each badge has a type, metadata URI, and owner address.
- Supports transfer and burn operations with admin-controlled minting.
