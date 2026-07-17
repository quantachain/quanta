# QuantaChain Testnet — V2 Release (v2.4.20-alpha)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.4.20-alpha — NETWORK ISOLATION HOTFIX (2026-07-17)**
> **MANDATORY UPDATE.** 
> - **Protocol version 21 & Magic QT21**: Bumps network identifiers to explicitly reject community nodes still running the corrupted `v18` image. This completely stops the noisy `unexpected end of file` bincode errors caused by `v18` and `v19` colliding on `QT19` magic bytes despite differing payload formats.

> **v2.4.19-alpha — DAG CORRUPTION RECOVERY / HARD FORK (2026-07-17)**
> **CRITICAL UPDATE: Required to rescue the stalled network.**
> - **Reverted v2.4.18-alpha**: The previous release contained an incorrect assumption about a bincode discriminant shift that broke compatibility. `v2.4.19-alpha` restores the exact binary wire format of `v15`-`v17`.
> - **Session 1361 Rescue**: The network permanently stalled at height 81664 because node operators manually deleted their multi-GB AlephBFT backup files (`alephbft_backup_1361.dat`) to recover from OOM crashes. Deleting these files mid-session permanently corrupts the cryptographic DAG for that session.
> - **Hard Fork**: Added a consensus rule to artificially advance the AlephBFT session ID to 1362 for heights >= 81664. This forces the entire network to start a brand new DAG from Round 0, allowing consensus to resume immediately.
> - Removed the exponential round-delay from the AlephBFT config to prevent network latency from cascading into multi-hour delays.

> **v2.4.16-alpha — ALEPHBFT STARTUP CRASH HOTFIX (2026-07-17)**
> **MANDATORY UPDATE.** Fixes a crash occurring during session initialization where DAG units were parsed before the network peer list was fully synced.

> **v2.4.15-alpha — NETWORK PROTOCOL HARD FORK (2026-07-17)**
> **MANDATORY UPDATE.** Bumps the internal `PROTOCOL_VERSION` to 20 to fully isolate upgraded nodes from older nodes running v2.4.5 through v2.4.12.
> * **Background**: In v2.4.13, we added a unicast routing wrapper to AlephBFT consensus messages to fix a broadcast storm. However, older nodes did not understand this new wrapper, causing them to silently drop units without ACKing them. This forced the upgraded nodes into a hyper-aggressive retry loop that consumed 100% CPU via ZSTD compression tasks.
> * **Hard Fork Isolation**: By incrementing the `PROTOCOL_VERSION`, upgraded nodes now explicitly reject handshakes from older, incompatible validators. 
> * **NOTE ON CHAIN STALL**: Because the network is now split between v19 and v20 nodes, **neither side currently has the 2/3 + 1 quorum needed to finalize new blocks**. The chain will remain stalled at height 81664 until the remaining community validators update their nodes to v2.4.15-alpha to rejoin the v20 network and restore quorum!

> **v2.4.14-alpha — AUTOMATIC CPU/OOM RECOVERY (2026-07-17)**
> **MANDATORY UPDATE.** Fixes nodes hanging at 100% CPU indefinitely upon reboot.
> * **Bloated Backup Wiping**: Added an automatic size check in `bft_proposer.rs` that wipes the AlephBFT backup file *before* opening it if it exceeds 10 MB. This allows nodes that were previously stuck accumulating multi-GB files (due to earlier bugs) to automatically recover and boot instantly, without requiring manual `sudo rm` intervention from node operators.

> **v2.4.13-alpha — UNICAST BROADCAST STORM HOTFIX (2026-07-17)**
> **MANDATORY UPDATE.** Fixes the actual root cause of the 100% CPU lockup when quorum is lost.
> * **Broadcast Storm Prevention**: When validators were offline, AlephBFT frantically tried to send Unicast `Fetch` requests to them to catch up. Because they were offline, the network layer fell back to broadcasting these Unicast messages. Furthermore, nodes mistakenly relayed incoming Unicast messages to the entire network. This resulted in an `O(N^2)` broadcast storm of ZSTD-compression tasks, completely pegging CPU at 100% and stalling the node. Unicast messages are now correctly dropped if the recipient is not directly connected, and nodes no longer gossip Unicast messages meant for others.

> **v2.4.12-alpha — AlephBFT MEMORY LEAK HOTFIX (2026-07-17)**
> **MANDATORY UPDATE.** Fixes a massive memory leak (1.8GB+ RAM per node) caused by the v2.4.10-alpha watchdog.
> * **Bloated Backup Wipe**: When the 120-second session watchdog kills a stuck session, it now deletes the `alephbft_backup_{session}.dat` file. Previously, the file was kept, meaning every 120s AlephBFT would restart and load a massive history of useless DAG units into memory, which spiked CPU to 100% and RAM to 1.8GB+ until the VM crashed with an OOM.

> **v2.4.11-alpha — AlephBFT CPU SPIKE HOTFIX (2026-07-17)**
> **MANDATORY UPDATE.** Secondary fix for the CPU spike when quorum is lost.
> * **Progressive Unit Delay Backoff**: The previous fix added a 120-second watchdog, which still allowed AlephBFT to burn 100% CPU during that 120-second window before terminating. We now implemented a progressive backoff for DAG unit creation. If the network is stuck, the interval between unit proposals scales up from 500ms to 10 seconds. This drops CPU usage by 20x during network downtime while allowing the node to instantly recover within 10 seconds of quorum being restored.

> **v2.4.10-alpha — AlephBFT CPU SPIKE FIX (2026-07-17)**
> **MANDATORY UPDATE.** Root-cause fix for 80–90% CPU usage when the network has no block finalization (insufficient quorum):
> * **Session Watchdog**: Added a 120-second watchdog in `bft_proposer.rs`. If no block is finalized for >120s, the AlephBFT session is terminated and the node sleeps 30s before restarting. This prevents the DAG unit-creation loop (which fires every 500ms per validator via Falcon-512 signing) from running indefinitely when fewer than 9/13 validators are online.
> * **Stuck Backoff in DataProvider**: `aleph_data.rs` now sleeps 30s before proposing a block if no block has been finalized for >30s, further reducing CPU during network downtime.
> * **Root Cause**: AlephBFT creates a Falcon-512-signed DAG unit every 500ms per node. With 4 nodes and no quorum, this produced ~8 heavy crypto ops/second indefinitely — 80–90% CPU. Previously this never triggered because blocks were finalized every 6s and sessions rotated quickly.
> * **Protocol unchanged**: Still v19 / QT19. No chain reset required.

> **v2.4.9-alpha — EPOCH POOL REWARD MODEL (2026-07-17)**
> * Reward distribution switches from single-proposer-wins-all to uptime-proportional pooling at block 100,000. No chain reset required.

> **v2.4.7-alpha** \u2014 Network isolation (Protocol v19 / QT19): evicts old v2.4.5 nodes flooding network.
> **v2.4.6-alpha** \u2014 BFT infinite loop fix: strict stake/unstake validation in mempool and block template.
> **v2.4.5-alpha** \u2014 CPU/RAM DOS fix (Protocol v18 / QT18): header buffer OOM, Tokio starvation, BFT message limit.
> **v2.4.4-alpha** \u2014 Memory leak and zip bomb fix (Protocol v17 / QT17).
> **v2.4.3-alpha** \u2014 TOCTOU race condition fix (Protocol v16 / QT16).
> **v2.4.2-alpha** \u2014 Network stability and IP flapping fix (Protocol v15 / QT15).
> **v2.4.1-alpha** \u2014 Peer ban/flapping fix: dead streams no longer trigger bans.
> **v2.4.0-alpha** \u2014 BFT session restart timing fix, locking optimisation.
> **v2.3.x-alpha** \u2014 Protocol v14/QT14, stream framing fix, log spam cleanup, BFT gossip relay fix.


This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## 🛠️ How to Run a Validator Node

> **🟢 STAKING IS OPEN!** The network is fully transitioned to DPoS mechanics. Anyone who stakes at least **100,000 QUA** can run a validator node, propose blocks, and earn rewards!

To join the network, you need to point your node to the bootstrap VPS. Below are the three ways to run the node, ordered by recommendation.

### Option 1: Native Docker Run (Recommended)

This is the easiest and most direct way to run your validator. Just ensure you pass the `--bootstrap` flag to connect to the network.

**1. Pull the Latest Image**
```bash
docker pull xd637/quanta-node:latest
```

**2. Start the Node**
> [!IMPORTANT]
> Change `"YOUR_PASSWORD_HERE"` to your actual wallet password, and ensure `validator.qua` matches your wallet filename!

```bash
docker run -d \
  --name quanta-validator \
  --restart always \
  --network host \
  -v ~/quanta_data_v2:/home/quanta/quanta_data \
  -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 34.87.128.33:8333
```

---

### Option 2: Docker Compose

If you prefer managing your containers via Docker Compose, create a `docker-compose.yml` file with the following configuration:

```yaml
version: '3.8'
services:
  quanta-node:
    image: xd637/quanta-node:latest
    container_name: quanta_validator
    restart: unless-stopped
    network_mode: "host"
    volumes:
      - ~/quanta_data_v2:/home/quanta/quanta_data
    environment:
      - QUANTA_WALLET_PASSWORD=YOUR_PASSWORD_HERE
    command: >
      quanta start
      --validator-wallet /home/quanta/quanta_data/validator.qua
      --bootstrap 34.87.128.33:8333
```

**Start the Node:**
```bash
docker compose up -d
```

---

### Option 3: Native Source Build

For developers or those who prefer running natively without Docker:

**1. Clone and Build**
```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
git checkout v2.4.0
cargo build --release
```

**2. Run the Node**
```bash
./target/release/quanta start -c quanta.toml --validator-wallet ./quanta_data/validator.qua --bootstrap 34.87.128.33:8333
```

---

### Option 4: Advanced Configuration (quanta.toml Override)

If you want full control over your node's configuration instead of relying on CLI arguments like `--bootstrap`, you can provide your own `quanta.toml` file. This is useful for customizing port bindings, max peers, or API rate limits.

**When using Docker**, you can mount your local `quanta.toml` to override the default one baked into the image. Just add a `-v` flag to your `docker run` command:

```bash
docker run -d \
  --name quanta-validator \
  --restart always \
  --network host \
  -v ~/quanta_data_v2:/home/quanta/quanta_data \
  -v ~/my-local-quanta.toml:/home/quanta/quanta.toml \
  -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua
```

This completely overrides the baked configuration with your own file.

---

## Wallet Management

If you need to generate a wallet before starting your node:

```bash
# New HD Wallet (Recommended)
quanta-wallet new --file my_wallet.json

# New Raw Wallet
quanta-wallet new-raw --file my_raw.qua
```

---

## License

QUANTACHAIN operates under an **Open-Core Dual License** model:
- **Core Protocol**: [GNU AGPLv3](https://github.com/quantachain/quanta/blob/main/LICENSE)
- **Native Templates & APIs**: [QuantaLabs Commercial License](https://github.com/quantachain/quanta/blob/main/COMMERCIAL_LICENSE.md)

For commercial licensing: **contact@quantachain.org**
