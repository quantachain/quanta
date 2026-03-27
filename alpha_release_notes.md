# QuantaChain Testnet — Alpha v0.3.0 (Testnet V2)

Post-quantum secure blockchain using Falcon-512 signatures and SHA3-256 Proof of Work.

> **⚠️ CRITICAL: TESTNET RESET ⚠️**
> This release includes a new genesis block. If you are running an older alpha node, you **MUST delete your `quanta_data/` folder** before starting this update. The old chain is incompatible.

This is a **pre-release testnet build**. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## Genesis Block

| Parameter | Value |
|---|---|
| Network | Testnet |
| Timestamp | `1774828800` (2026-04-01 00:00:00 UTC) |
| Testnet Genesis Hash | `0000001a2cbe8311e347945a5d0c35563b3b17b7423f6cc471b9c623ef10b77f` |
| Mainnet Genesis Hash | `1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0` |
| Difficulty | 8,343,908 (Testnet) / 16,777,216 (Mainnet) |
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
1. Open Docker Desktop and find `xd637/quanta-node:v0.3.0-alpha` (or `:latest`) in your Images.
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
docker pull xd637/quanta-node:v0.3.0-alpha

# Run directly (Ensure data persistence!)
docker run -d \
  --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:v0.3.0-alpha
```

### Option 3: Docker Compose (Recommended)

**If updating from an older version**, delete your old named volume first:
```bash
docker compose down -v
```

Then start the node:
```bash
docker compose -f docker-compose.single.yml up -d
```

---

## Server Setup (Ubuntu / VPS)

The following instructions guide you through setting up a persistent, always-on Quanta node on a fresh Ubuntu server.

**1. Update system and install Docker:**
```bash
sudo apt update && sudo apt upgrade -y
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker ubuntu && newgrp docker
```

**2. Open necessary ports:**
```bash
sudo iptables -I INPUT -p tcp --dport 8333 -j ACCEPT
sudo iptables -I INPUT -p tcp --dport 7782 -j ACCEPT
sudo iptables -I INPUT -p tcp --dport 3000 -j ACCEPT
sudo apt install -y iptables-persistent
sudo netfilter-persistent save
```

**3. Set up directory and start the node (using Host Networking for API Security):**
```bash
mkdir -p ~/quanta_data
sudo chmod 777 ~/quanta_data

docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```
*(Need public Web Wallet access? See `TESTNET_RPC_SETUP.md` for NGINX & SSL setup)*

**4. Check logs:**
```bash
docker logs quanta-node --tail 30 -f
```

**5. Update / Clean Restart (REQUIRED FOR v0.3.0):**

Due to the Testnet V2 reset, you MUST delete your old blockchain data before restarting:

```bash
docker stop quanta-node && docker rm quanta-node

# ⚠️ CRITICAL: Delete old blockchain data
sudo rm -rf ~/quanta_data/*

docker pull xd637/quanta-node:latest
docker run -d \
  --name quanta-node \
  --restart always \
  --network host \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

---

## Manual Build from Source

```bash
git clone https://github.com/quantachain/quanta
cd quanta
git checkout v0.3.0-alpha
cargo build --release

# Run node
./target/release/quanta start -c quanta.toml
```

---

## Wallet Management

**Create a new wallet natively:**
```bash
./target/release/quanta new-wallet --file wallet.qua
```

**Create a raw encrypted wallet using Docker:**
```bash
docker exec -it quanta-node quanta new_wallet --file wallet.qua
```

**Create a new HD Wallet (Recommended! Gives JSON + 24-word recovery phrase):**
```bash
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json
```

---

## Mining (Proof of Work)

**Start CPU miner natively:**
```bash
./target/release/quanta start_mining YOUR_WALLET_ADDRESS --rpc-port 7782
```

**Start CPU miner using Docker (Background):**
```bash
docker exec -d quanta-node quanta start_mining YOUR_WALLET_ADDRESS --rpc-port 7782
```

**Check Mining Logs in Docker:**
```bash
docker logs quanta-node --tail 30 -f
```

**Stop Mining using Docker:**
```bash
docker exec -it quanta-node quanta stop_mining --rpc-port 7782
```

---

## Node Status & Blockchain Info

**Print Current Blockchain Height:**
```bash
docker exec -it quanta-node quanta print_height --rpc-port 7782
```

**View Full Node Status (Peers, Height, Mempool):**
```bash
docker exec -it quanta-node quanta status --rpc-port 7782
```

**View Dynamic Mining Status (Difficulty, Blocks Mined, Rewards):**
```bash
docker exec -it quanta-node quanta mining_status --rpc-port 7782
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

## What Changed in Alpha v0.3.0 (Testnet V2)

- **Testnet V2 Genesis Reset:** Restarted the testnet with a realistic genesis difficulty (`8,343,908`) to properly enforce ~30s block times.
- **Difficulty Adjustment Fix:** Removed the broken ±15% bounding cap on difficulty adjustments. The algorithm is now mathematically equivalent to Bitcoin's formula with a 4x clamp, resolving the issue where difficulty failed to adjust correctly.
- **Weighted Peer Reputation:** Replaced the flat 3-strike system with a Bitcoin-style DoSMan weighted scoring system (0-100). Serious consensus violations (e.g., invalid blocks) result in immediate bans (Score: +50), while minor issues like invalid txs (Score: +10) or message floods (Score: +20) accumulate gradually.
- **Wallet & Mining CLI Improvements:** 
  - `quanta new_wallet` now clearly prints your generated address to the terminal.
  - `quanta mining_status` no longer silently truncates your public address, and clearly states if mining is idle instead of looking stuck.
  - Mining will no longer start unnecessarily if your node is out of sync.
- **Block Explorer API:** Added full support for paginated address history (`/api/address/:address/txs`), address lookup (`/api/address/:address`), $O(1)$ transaction lookup by hash (`/api/tx/:hash`), and latest block feeds (`/api/blocks/latest`).
- **Network Routing Fixes:** Nodes no longer request duplicate blocks during broadcasts, fixing the persistent sync stall bug ("stuck at 272").

---

## What Changed in Alpha v0.2.0

- Mnemonic-based faucet wallet system (10 reserve wallets, BIP-39 derived)
- Genesis timestamp updated to 2026-03-21
- Security audit 2 applied: nonce atomicity, coinbase validation, MTP timestamp rule, per-sender mempool cap, state root enforcement
- Block size increased to 2 MB to accommodate Falcon-512 transaction sizes
- License changed from MIT to Apache 2.0

## Security Notice

This release has undergone internal audit only. It has NOT been formally verified by a third-party security firm. Do not use for real financial transactions.

Falcon-512 and Kyber-1024 implementations are based on NIST PQC Round 3 finalists.

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

"Quanta" and "QuantaChain" are trademarks. Forks may not use these names without permission.
