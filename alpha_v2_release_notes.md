# QuantaChain Testnet — Alpha v2

Post-quantum secure blockchain using Falcon-512 signatures and SHA3-256 Proof of Work.

This is a **pre-release testnet build**. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## Genesis Block

| Parameter | Value |
|---|---|
| Network | Testnet |
| Timestamp | `1774051200` (2026-03-21 00:00:00 UTC) |
| Testnet Genesis Hash | `fd1b98c04051c3f413dd605ca44f3b200a95752efada30a6e2d142bcfaf094d3` |
| Mainnet Genesis Hash | `1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0` |
| Difficulty | 4 (Testnet) / 6 (Mainnet) |
| Block Time | 30 seconds |

---

## Testnet Faucet Wallets (Genesis Premine)

Each address below received **1,000,000 QUA** at genesis. Faucet account 0 is the active sender used by the faucet API.

| Index | Address | Role |
|---|---|---|
| 0 | `0x1683be267318d2ddd8cee8df4a4548dcffb1e088` | Faucet Sender (active) |
| 1 | `0xd528c18ce7a8844e4a4dcd841975b20ae599b020` | Faucet Reserve |
| 2 | `0xfd6e36bfa2b2798d08592802206c943d5513adfb` | Faucet Reserve |
| 3 | `0xed15573ad312d41aaef74cff56a8ef28122ec2db` | Faucet Reserve |
| 4 | `0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb` | Faucet Reserve |
| 5 | `0xbf5ee055f399323fdd0cefe3d4aa923678d46107` | Faucet Reserve |
| 6 | `0x1dc9637b183093d723ea8d1fb18083b06490facb` | Faucet Reserve |
| 7 | `0xa2270f30ca1aad922510375508bf68cd95509f29` | Faucet Reserve |
| 8 | `0xe15a689775685ae324559ea9a492fc650354ca0b` | Faucet Reserve |
| 9 | `0x005dcff212d27b55e7a74bf745e1349ab44ca25d` | Faucet Reserve |

---

## Treasury

| Parameter | Value |
|---|---|
| Treasury Address | `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` |
| Multisig Scheme | 3-of-5 Falcon-512 |
| Block Reward to Treasury | 5% per block |
| Fee to Treasury | 20% of transaction fees |

---

## Tokenomics

| Parameter | Value |
|---|---|
| Year 1 Block Reward | 100 QUA |
| Annual Reward Reduction | 15% per year |
| Minimum Reward Floor | 5 QUA (reached ~year 20) |
| Mining Reward Lock | 50% locked for 6 months |
| Fee Burn | 70% of all transaction fees |
| Fee to Treasury | 20% of all transaction fees |
| Fee to Miner | 10% of all transaction fees |
| Max Block Transactions | 1,200 |
| Max Block Size | 2 MB |
| Min Transaction Fee | 0.0001 QUA |

---

## Quick Start with Docker

### Option 1: Docker Desktop (Graphical Interface)
1. Open Docker Desktop and find `xd637/quanta-node:alpha-v2` (or `:latest`) in your Images.
2. Click **Run**.
3. Under **Optional settings**, configure:
   - **Container name**: `quanta-node`
   - **Ports**: Map `3000`, `7782`, `8333`, `9090` to themselves.
   - **Volumes**: 
     - Add host path `quanta-data` to container path `/home/quanta/quanta_data`
     - Add host path `quanta-logs` to container path `/home/quanta/logs`
4. Click **Run**.

### Option 2: Docker CLI
```bash
# Pull the image
docker pull xd637/quanta-node:alpha-v2

# Run directly (Ensure data persistence!)
docker run -d \
  --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:alpha-v2
```

### Option 3: Docker Compose (Recommended)
```bash
docker compose -f docker-compose.single.yml up -d
```

---

## Manual Build from Source

```bash
git clone https://github.com/quantachain/quanta
cd quanta
git checkout alpha-v2
cargo build --release

# Run node
./target/release/quanta start -c quanta.toml
```

---

## Wallet Management

**Create a new HD wallet natively:**
```bash
./target/release/quanta-wallet new-hd
```

**Create a new HD wallet using Docker:**
```bash
docker exec -it quanta-node quanta-wallet new-hd
```

---

## Mining (Proof of Work)

**Start CPU miner natively:**
```bash
./target/release/quanta-miner start --address YOUR_WALLET_ADDRESS
```

**Start CPU miner using Docker:**
```bash
docker exec -it quanta-node quanta-miner start --address YOUR_WALLET_ADDRESS
```

---

## Ports

| Port | Service |
|---|---|
| `3000` | REST API |
| `8333` | P2P Network |
| `7782` | RPC |
| `9090` | Prometheus Metrics |

---

## What Changed in Alpha v2

- Mnemonic-based faucet wallet system (10 reserve wallets, BIP-39 derived)
- Rate-limited faucet API: 1 QUA per IP per wallet per day
- Genesis timestamp updated to 2026-03-21
- Security audit 2 applied: nonce atomicity, coinbase validation, MTP timestamp rule, per-sender mempool cap, state root enforcement
- Difficulty adjustment interval increased to 2016 blocks for stability
- Block size increased to 2 MB to accommodate Falcon-512 transaction sizes
- Mining reward lock extended to 6 months (anti-dump)
- License changed from MIT to Apache 2.0

---

## Security Notice

This release has undergone internal audit only. It has NOT been formally verified by a third-party security firm. Do not use for real financial transactions.

Falcon-512 and Kyber-1024 implementations are based on NIST PQC Round 3 finalists.

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

"Quanta" and "QuantaChain" are trademarks. Forks may not use these names without permission.
