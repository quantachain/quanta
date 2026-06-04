# QuantaChain Testnet — V2 Alpha (v2.0.0)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.0.0 — MAJOR ARCHITECTURE UPGRADE: BFT Consensus & AI Agent Support**
> **TESTNET RESET REQUIRED.** All nodes must wipe their databases (`rm -rf ./quanta_testnet_data`) and sync from the new genesis block.
> Mining has been completely removed. The network now operates on a deterministic 6-second block time via Proof of Stake BFT.

This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## 🚨 V2 Hard Fork Details 🚨
- **Consensus Engine:** Migrated from SHA3-256 Proof of Work to AlephBFT (Asynchronous Byzantine Fault Tolerance).
- **Network Isolation:** Updated network magic bytes to `Q2T3` to prevent old nodes from connecting to the new consensus network.
- **Block Time:** Exact 6-second deterministic slots (previously ~30s random).
- **Mining Removed:** All `start_mining` commands and the `quanta-miner` binary have been removed.
- **AI Agent Support:** Added headless `QUANTA_WALLET_PASSWORD` environment variable support for automated AI escrow workflows.
- **HD Wallets:** The CLI wallet has been completely rewritten to support deterministic hierarchical generation natively.
- **Persistent Crash Recovery:** AlephBFT DAG state is now persisted to disk (`alephbft_backup.dat`), allowing seamless recovery and network rejoin after node restarts.

---

## Genesis Block

| Parameter | Value |
|---|---|
| Network | Testnet (QUA7) |
| Timestamp | `1748736001` (2025-06-01 00:00:01 UTC) |
| Testnet Genesis Hash | `48119d35c293531f1438b29a50d674575e4d5002e789699fe8efbd955eea2115` |
| Block Time | Exactly 6 seconds |
| TPS Limit | ~250 - 300 TPS (assuming 2MB block limit) |

---

## 🔄 Clean Start Guide (Wipe Data & Resync from Genesis)

> **You MUST perform a clean start to join the V2 Testnet!**
> The V2 BFT consensus engine uses a different block structure and state machine. It will crash if it reads old V1 PoW blocks.

### Bare Metal / VPS (no Docker)

```bash
pkill -f "quanta start"    # stop the old node
rm -rf ./quanta_testnet_data  # WIPE THE OLD POW CHAIN DATA!
cargo build --release      # compile the new V2 binary
./target/release/quanta start -c quanta.toml
```

---

## Validator Setup (Docker)

> **⚠️ ATTENTION:** This is currently a strictly permissioned testnet designed only for testing. Only the validators explicitly hardcoded in the Genesis set can run a node and produce blocks.
> Once the network matures, we will implement full DPoS, allowing anyone to stake and participate in consensus. Until then, if you would like early access to participate, please email: **contact@quantachain.org**

If you have been selected as a validator, follow these exact instructions to spin up your validator node and connect to the core network using Docker:

**1. Create a Wallet and Get Your Key**
You must generate a raw wallet and provide the public key to the core team to be whitelisted in the Genesis block.
```bash
docker run --rm -it xd637/quanta-node:latest quanta-wallet new-raw --file /tmp/validator.qua
```

**2. Directory Setup**
Create the directory where your blockchain data will live. We recommend adding `_v2` to avoid mixing it with any old testnet data:
```bash
mkdir -p ~/quanta_data_v2
```

**3. Place Your Wallet File**
Move your validator wallet file (e.g., `validator.qua` or whatever you named it) directly into the `~/quanta_data_v2` directory you just created.

Your folder should look exactly like this:
```text
~/quanta_data_v2/
└── validator.qua
```
*(Note: You do NOT need the `genesis.json` or `quanta.toml` files! The latest network configuration is securely baked directly into the V2 Docker image.)*

**4. Pull the Latest Image & Start the Node**
Run the following Docker commands to pull the latest V2 build, launch your node, connect to the Bootstrap node, and begin proposing blocks. 

> [!IMPORTANT]
> **Before running the second command below, you MUST change two things:**
> 1. Change `"YOUR_PASSWORD_HERE"` to your actual wallet password.
> 2. Change `validator.qua` at the very end of the command to match the exact name of your wallet file.

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-validator \
  --restart always \
  --network host \
  -v ~/quanta_data_v2:/home/quanta/quanta_data \
  -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 79.137.78.1:8333
```

---

## Wallet Management

```bash
# New HD Wallet (Recommended)
quanta-wallet new --file my_wallet.json

# New Raw Wallet
quanta-wallet new-raw --file my_raw.qua

# AI Headless Mode (Set env var to skip password prompts)
export QUANTA_WALLET_PASSWORD="your_password"
quanta-wallet info --file my_wallet.json
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

## License

QUANTACHAIN operates under an **Open-Core Dual License** model:
1. **Core Protocol:** Licensed under the [GNU AGPLv3](../LICENSE).
2. **Native Templates & APIs:** Licensed under a [Proprietary Commercial License](../COMMERCIAL_LICENSE.md).

