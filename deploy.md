# ChronoRise — Testnet Deployment

**Network:** Stellar Testnet (`Test SDF Network ; September 2015`)
**Deployed:** 2026-07-08
**Deployer Public Key:** `GDSDRD6PHZGDC7JJJ6ZCXZOWMNZUVNO3K5UZPWKRXA6SDPTQWM6BETSU`
**Stellar CLI Version:** 27.0.0

---

## Deployed Contracts

| Contract | Contract ID | Explorer |
|---|---|---|
| `claim_orchestrator` | `CBZMSLUGBS5DSHI2VQ6HICVXGCBLAZQXWN6AZHJCWGW3ZA6R23BGCGHL` | [View](https://stellar.expert/explorer/testnet/contract/CBZMSLUGBS5DSHI2VQ6HICVXGCBLAZQXWN6AZHJCWGW3ZA6R23BGCGHL) |
| `zk_verifier` | `CBSEWCQCELKFUTD4DZATPHDGZR7YHZDMCKFILHY7HEWUOVRQ5XZ4WOHA` | [View](https://stellar.expert/explorer/testnet/contract/CBSEWCQCELKFUTD4DZATPHDGZR7YHZDMCKFILHY7HEWUOVRQ5XZ4WOHA) |
| `reward_pool` | `CA7FC3G2USG2PCWJYRCL4355ZKGQYJG5F7SX6TFZBQHEZJ3EZ4J7CGCR` | [View](https://stellar.expert/explorer/testnet/contract/CA7FC3G2USG2PCWJYRCL4355ZKGQYJG5F7SX6TFZBQHEZJ3EZ4J7CGCR) |
| `badge_nft` | `CCTYQLOKWXRQLNRYW75RJ3RRVUB7I2HXV7UMKFYZYGBPOEPUOLOHIAVU` | [View](https://stellar.expert/explorer/testnet/contract/CCTYQLOKWXRQLNRYW75RJ3RRVUB7I2HXV7UMKFYZYGBPOEPUOLOHIAVU) |
| `player_registry` | `CBL2HWTX3KOE3ZH5QEZV63XZJRJ6U34Y5NXV5M4WFD4LHWL2FN77YVR7` | [View](https://stellar.expert/explorer/testnet/contract/CBL2HWTX3KOE3ZH5QEZV63XZJRJ6U34Y5NXV5M4WFD4LHWL2FN77YVR7) |
| `achievement_registry` | `CAU2ZVPXM2EBXEQ4X7ADTVGN6QN2ZHHUKUG6SB22V3QBXKRAKEDFGOWH` | [View](https://stellar.expert/explorer/testnet/contract/CAU2ZVPXM2EBXEQ4X7ADTVGN6QN2ZHHUKUG6SB22V3QBXKRAKEDFGOWH) |
| `treasury` | `CC56QR5ZDSIZKP6FRBHFLVO54TXYZ4XJXS7VSKMCPWU4DONYCDSAGP67` | [View](https://stellar.expert/explorer/testnet/contract/CC56QR5ZDSIZKP6FRBHFLVO54TXYZ4XJXS7VSKMCPWU4DONYCDSAGP67) |
| `tournament_rewards` | `CBMHNUPZSPBFLZHAMXCBYZVOBNCRPP5NNTJZ3LMZA5JNODOUSJJZSOBO` | [View](https://stellar.expert/explorer/testnet/contract/CBMHNUPZSPBFLZHAMXCBYZVOBNCRPP5NNTJZ3LMZA5JNODOUSJJZSOBO) |
| `governance` | `CAG67XXNCR7QWZPOS6N3I77WBZOWTFAPBZRF2Z4OYW7KONECBPQ5OK3M` | [View](https://stellar.expert/explorer/testnet/contract/CAG67XXNCR7QWZPOS6N3I77WBZOWTFAPBZRF2Z4OYW7KONECBPQ5OK3M) |

---

## Deployer Keypair

Stored locally under the alias `deployer` in `~/.config/stellar/identity/deployer.toml`.

```
Public Key:  GDSDRD6PHZGDC7JJJ6ZCXZOWMNZUVNO3K5UZPWKRXA6SDPTQWM6BETSU
Secret Key:  SCH6RUM2JCPUB3QOQM63F7SUREMTPNQCTOVRUGM7MTEBBHIL2WG5APBJ
```

> **Keep the secret key safe.** Do not commit it to version control. It is already set in `chronorise-backend/.env`.

---

## Backend `.env` — Contract IDs

All contract IDs are pre-filled in `chronorise-backend/.env`. The `STELLAR__SIGNER_SECRET` is also set.

---

## Database Setup

PostgreSQL is running but needs the `chronorise` role and database created. Run this once as a superuser:

```bash
sudo -u postgres psql <<'EOF'
CREATE USER chronorise WITH PASSWORD 'chronorise_dev';
CREATE DATABASE chronorise OWNER chronorise;
GRANT ALL PRIVILEGES ON DATABASE chronorise TO chronorise;
EOF
```

Then run migrations:

```bash
cd chronorise-backend
sqlx migrate run
# or if using the Docker setup:
# docker compose -f Docker/docker-compose.yml up -d db
# sqlx migrate run
```

---

## NATS Setup

NATS is not installed by default. Either install it or run via Docker:

```bash
docker run -d --name nats -p 4222:4222 nats:latest
```

Or install natively:

```bash
# Ubuntu/Debian
curl -L https://github.com/nats-io/nats-server/releases/latest/download/nats-server-v2.10.18-linux-amd64.zip -o nats.zip
unzip nats.zip && sudo mv nats-server /usr/local/bin/
nats-server &
```

---

## Running the Stack

Once Postgres, Redis, and NATS are up:

```bash
# Backend
cd chronorise-backend
cargo run

# Frontend (separate terminal)
cd chronorise-web
npm run dev
```

---

## Re-deploying Contracts

If you need to redeploy (e.g. after code changes):

```bash
cd chronorise-contracts/contracts/<contract_name>
stellar contract build
stellar contract deploy \
  --wasm target/wasm32v1-none/release/<contract_name>.wasm \
  --source deployer \
  --network testnet
```

Update the new contract ID in `chronorise-backend/.env`.
