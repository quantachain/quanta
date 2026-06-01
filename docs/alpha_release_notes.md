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

## Server Setup (Ubuntu / VPS) for V2 Genesis Launch

**1. Generate 7 HD/Raw Wallets for your Validators:**
```bash
cargo run --bin quanta-wallet -- new-raw --file node1.qua
# Repeat for node2 through node7. Save the passwords safely!
```

**2. Inject your Public Keys into the Source Code:**
Use `cargo run --bin quanta-wallet -- --wallet node1.qua address` to get your Falcon-512 Public Keys and Addresses. Paste them into the `testnet_faucets` and `genesis_validators` array in `src/consensus/blockchain.rs`.

**3. Update Genesis Hash:**
```bash
cargo run --bin get_testnet_hash
```
Copy the output and update `TESTNET_GENESIS_HASH` in `src/consensus/blockchain.rs`.

**4. Compile and Start the Seed Node:**
```bash
cargo build --release
./target/release/quanta --validator node1.qua
```

**5. Start the Remaining 6 Nodes:**
```bash
./target/release/quanta --validator node2.qua --seed <IP_OF_NODE_1>:8333
```
Once 5 of the 7 nodes connect, Block 1 will instantly finalize and the V2 network is live!

---

## Wallet Management

```bash
# New HD Wallet (Recommended)
quanta-wallet new-hd --file my_wallet.qua

# New Raw Wallet
quanta-wallet new-raw --file my_raw.qua

# AI Headless Mode (Set env var to skip password prompts)
export QUANTA_WALLET_PASSWORD="your_password"
quanta-wallet --wallet my_wallet.qua balance
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

Apache 2.0 — see [LICENSE](LICENSE)

"Quanta" and "QuantaChain" are trademarks. Forks may not use these names without permission.
