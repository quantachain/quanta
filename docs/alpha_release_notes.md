# QuantaChain Testnet — V2 Alpha (v2.0.0)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.0.0 — MAJOR ARCHITECTURE UPGRADE: BFT Consensus & AI Agent Support**
> **TESTNET RESET REQUIRED.** All nodes must wipe their databases (`rm -rf ./quanta_testnet_data`) and sync from the new genesis block.
> Mining has been completely removed. The network now operates on a deterministic 6-second block time via Proof of Stake BFT.

This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## 🚨 V2 Hard Fork Details 🚨
- **Consensus Engine:** Migrated from SHA3-256 Proof of Work to AlephBFT (Asynchronous Byzantine Fault Tolerance).
- **Network Isolation:** Updated network magic bytes to `Q2T2` to prevent old nodes from connecting to the new consensus network.
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
| Timestamp | `1775001600` (2026-04-01 00:00:00 UTC) |
| Testnet Genesis Hash | *(Run `cargo run --bin get_testnet_hash` after injecting your keys)* |
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

If you have been selected as a validator, follow these steps to run your node using Docker:

**1. Create a Wallet and Get Your Key**
You must generate a raw wallet and provide the public key to the core team to be whitelisted in the Genesis block.
```bash
docker run --rm -it xd637/quanta-node:latest quanta-wallet new-raw --file /tmp/validator.qua
```

**2. Start the Validator Node**
Make sure to replace `<YOUR_PASSWORD>` with the password you used to create the wallet, and map your local wallet file into the container:

```bash
docker run -d \
  --name quanta-validator \
  --restart always \
  -e QUANTA_WALLET_PASSWORD="<YOUR_PASSWORD>" \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v /absolute/path/to/validator.qua:/home/quanta/validator.qua \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/validator.qua --bootstrap 79.137.78.1:8333
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

