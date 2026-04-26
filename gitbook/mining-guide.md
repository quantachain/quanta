# Mining Guide

QUANTA uses Adaptive Proof-of-Work with SHA3-256 double-hashing. Mining is CPU-friendly — no specialized ASICs are required or advantaged.

---

## Mining Rewards (Year 1)

| Parameter | Value |
|-----------|-------|
| Block reward | 100 QUA |
| Miner immediate share (47.5%) | 47.5 QUA/block |
| Miner locked share (47.5%) | 47.5 QUA/block — locked 6 months (~52,560 blocks) |
| Treasury (5%) | 5 QUA/block → `ms69216b1d10425689704d5ae3b2a4aa17049f59b1` |
| Fee share to miner | 10% of block fees |
| Target block time | 30 seconds |
| Daily blocks | ~2,880 |
| Daily immediate emission | ~136,800 QUA |

**Anti-Dump Vesting**: 47.5% of your mining rewards are locked for 6 months. This prevents sell cascades at launch and aligns miner incentives with network health.

**Coinbase Maturity**: Rewards require 100 block confirmations before they can be spent.

### Reward Schedule

| Year | Block Reward | Notes |
|------|-------------|-------|
| 1 | 100 QUA | |
| 2 | 85 QUA | 15% reduction |
| 3 | 72 QUA | |
| 5 | 52 QUA | |
| 10 | 23 QUA | |
| 20+ | 5 QUA | Perpetual floor |

The reward never drops below **5 QUA**, ensuring a permanent security budget.

---

## System Requirements

| Parameter | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 4 cores @ 2 GHz | 8+ cores |
| RAM | 8 GB | 16 GB |
| Storage | 1 TB SSD | 2 TB NVMe |
| Bandwidth | 50/20 Mbps | 100/50 Mbps |

Multi-core CPUs significantly improve signature verification throughput during block validation. Mining itself is single-threaded per miner instance.

---

## Start Mining — Docker

### 1. Start the Node

The node must be running and synced before you start mining.

```bash
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

Wait for the node to sync:
```bash
docker exec -it quanta-node quanta print_height --rpc-port 7782
```

### 2. Create a Wallet

```bash
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json
docker exec -it quanta-node quanta wallet_address --file hd_wallet.json
```

Note your address — it starts with `0x`.

### 3. Start Mining

```bash
# Runs in background inside the container
docker exec -d quanta-node quanta start_mining YOUR_WALLET_ADDRESS --rpc-port 7782
```

### 4. Monitor Mining

```bash
# Mining status — hashrate, last block mined, uptime
docker exec -it quanta-node quanta mining_status --rpc-port 7782

# Current chain height
docker exec -it quanta-node quanta print_height --rpc-port 7782

# Check your balance
curl http://localhost:3000/accounts/YOUR_ADDRESS/balance
```

### 5. Stop Mining

```bash
docker exec -it quanta-node quanta stop_mining --rpc-port 7782
```

---

## Start Mining — Source Build

```bash
# Start the node (daemon mode)
./target/release/quanta start -c quanta.toml --detach

# Start mining
./target/release/quanta start_mining YOUR_ADDRESS --rpc-port 7782

# Monitor
./target/release/quanta mining_status --rpc-port 7782
```

---

## Mining on a VPS

For 24/7 mining, a VPS is ideal. The node runs headlessly with Docker.

```bash
# On Ubuntu VPS — install Docker
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $USER && newgrp docker

# Open ports
sudo ufw allow 8333/tcp
sudo ufw allow 3000/tcp
sudo ufw allow ssh
sudo ufw --force enable

# Run the node with host networking (for RPC nodes) or standard port mapping (for miners)
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest

# Start mining
docker exec -d quanta-node quanta start_mining YOUR_ADDRESS --rpc-port 7782
```

Keep the mining process alive even after SSH disconnects — the node's `--restart always` flag handles this automatically.

---

## Difficulty Adjustment

Difficulty adjusts every **2,016 blocks** (~16.8 hours at 30-second blocks) using the Median-Time-Past formula.

- Maximum increase per adjustment: **×1.15** (15% cap)
- Maximum decrease per adjustment: **×0.85** (15% floor)
- Minimum difficulty: 4
- Prevents oscillation and hash-rate collapse death spirals

---

## Fee Distribution

When your mined block includes transactions, you receive a portion of fees:

```
Total block fees = sum of all tx.fee values

Fee burn:      70%  → permanently destroyed (deflationary)
Fee treasury:  20%  → treasury multisig
Fee miner:     10%  → your address (added to coinbase)
```

---

## Optimization Tips

1. **More CPU cores = faster block validation** — use a machine with 4+ physical cores
2. **Fast SSD reduces sync time** — NVMe preferred for archive nodes
3. **Good bandwidth matters** — the node propagates compressed blocks (~500 KB each) to peers
4. **Run on the same machine as the node** — the mining process communicates over RPC (port 7782)
5. **Monitor Prometheus metrics** at `http://localhost:9090` for hashrate, block time, and peer count

---

## Troubleshooting

**Mining starts but I'm not finding blocks**

The testnet difficulty adjusts automatically. At low hashrate, blocks may take longer. Check your mining status:

```bash
docker exec -it quanta-node quanta mining_status --rpc-port 7782
```

**RPC connection refused**

Ensure port 7782 is mapped and the node is running:
```bash
docker ps
docker logs quanta-node --tail 20
```

**Balance not showing rewards**

Mining rewards require 100 block confirmations. Locked rewards (47.5%) appear after ~52,560 blocks (~6 months).
