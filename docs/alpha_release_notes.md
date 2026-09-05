# Quanta Alpha Release Notes

## Current Version: v3.2.5-alpha (BFT Peer Resolution Fix)

This release fixes a critical bug introduced in v3.2.2-alpha where BFT unicast messages were being silently dropped due to a missing peer `node_id` resolution in the message handler. This restores BFT consensus.

### Key Changes
- **Peer Resolution**: Fixed `handle_message` to correctly use the resolved `actual_peer` reference across all handlers. This ensures `node_id` is updated correctly during the `P2PMessage::Version` handshake so validators can identify each other for unicast routing.

## Previous Fixes (Summarized)

### Previous Release: v3.2.4-alpha (Connection Tracking Fix)

This fixes a critical bug where nodes were overwhelmed by dropped peers reconnecting, leading to "Dropping inbound stream because we are at capacity" and node freezing.

### Key Changes
- **Connection Tracking**: Fixed `SwarmCommand::Disconnect` and `SwarmEvent::ConnectionClosed` race conditions that permanently leaked libp2p connections. Outdated nodes are now cleanly dropped and disconnected without leaving ghost connections that spam `yamux` streams.
- **Protocol Bump**: Bumped to `v60` to enforce a clean network upgrade.

### Previous Release: v3.2.3-alpha (BW-FIX-5: Strict Protocol Version Enforce)

This is a fast follow-up to BW-FIX-4. We are deploying a strict network handshake check that actively drops legacy `v58` connections.

### Key Changes
- **Strict Protocol Version Enforcement**: Reject outdated `v58` nodes that attempt to connect over `RequestResponse`.
- **Log Noise Reduction**: The `libp2p_gossipsub` duplicate publish warnings have been filtered out of the console output.

## Previous Fixes (Summarized)

### Previous Release: v3.2.2-alpha (The "Sync & Stability" Release)
This was a critical protocol version bump to force a network upgrade, finalizing the fix for the massive egress bandwidth spikes caused by gossipsub fallback loops, and fixing the chain sync stalls.

**What was fixed:**
- **Protocol Version Bump**: Bumped `PROTOCOL_VERSION` from 58 to 59. This hard-forks un-upgraded nodes that are still aggressively spamming duplicate `BftValidation` gossipsub requests when validators go offline.
- **Egress Spam Disabled**: Disabled the aggressive gossipsub fallback mechanism in AlephBFT unicast.
- **Sync Stall Fixed**: Replaced the O(N) database reads in `cumulative_work_at` with an O(1) mathematical calculation, fixing the massive 10-30 second network freeze during `GetHeaders` and ensuring nodes can sync instantly past height 195,331.
- **Peer Liveness Fixed**: `last_seen` timestamps are now correctly updated when messages are received, preventing active peers from silently timing out and disappearing from the active routing pool after 60 seconds.

---

### 🔴 This is a mandatory upgrade. v3.2.2 nodes (QT59) are incompatible with older nodes (QT58 and below).

---

### Previous Release: v3.2.0-alpha (The "Swarm" Release)
This release introduces a massive architectural overhaul of the Quanta P2P networking stack. The custom raw TCP loops have been entirely replaced with the industry-standard `libp2p` stack.

**Key Changes:**
- **Gossipsub Protocol**: Blocks and transactions are now propagated using efficient `Gossipsub` publish/subscribe mechanics instead of linear peer-loop unicasting.
- **Kademlia DHT**: Node discovery is now powered by Kademlia, augmenting the legacy `PeerDiscovery` DNS seeding.
- **Lock Starvation Eliminated**: Moving connection handling and socket I/O into the `Swarm` event loop eliminates the async lock contention that caused BFT nodes to freeze under heavy sync load.

---

### 🔴 This is a mandatory upgrade. v3.2.0 nodes (QT57) are incompatible with v3.1.5 nodes (QT56).

---

### Previous Release: v3.1.5-alpha (State Actor Refactoring & Sync Deadlock)
This release addresses a critical bottleneck in the Quanta consensus engine where heavy P2P sync and API requests would cause thread starvation and node freezing due to `RwLock<Blockchain>` contention, as well as Tokio thread pool exhaustion.

**Key Changes:**
- **State Actor Model**: The entire core blockchain state is now running inside a dedicated `tokio::mpsc` message-passing loop (`BlockchainActor`).
- **Lock-Free P2P & API**: The API (`handlers.rs`), RPC server (`server.rs`), and Networking (`network.rs`) layers now communicate with the state asynchronously using `BlockchainHandle`, completely removing read/write locks.
- **Sync Deadlock Fix**: Bypassed concurrent pre-verification for synchronized blocks to avoid saturating the Tokio blocking thread pool, resolving the 60s timeout issue.
- **Improved Uptime**: Network nodes will no longer deadlock or drop peers when processing heavy mempool operations.
- **PQC Intact**: Post-Quantum Cryptography implementations (Falcon-512) remain untouched and secure.

---

### Previous Release: v3.1.4-alpha (2026-08-29)

#### 1. Operators stuck at 1 peer — root cause fixed (AddrMan)
**All node operators were connecting to only 1 peer** (the bootstrap relay) despite 20+ validators being online and synced. The root cause was the AddrMan "verified" peer system introduced in v3.1.0:

- The bootstrap node (`node1.quantachain.org`) only has **inbound** connections from validators — it never dials them outbound.
- Under v3.1.0 rules, a peer can only be "verified" after a successful **outbound** connection.
- So every validator's entry in node1's table was forever `verified=false` → excluded from `GetAddr` gossip.
- When any operator sent `GetAddr` to node1: **empty response**. No peer discovery. Everyone stayed at 1 peer.

**Fix**: Inbound peers that pass TLS + Falcon-512 handshake and self-report a `listen_addr` (not an ephemeral source port) are now marked `verified=true` immediately. The relay can now gossip all 20+ validators back to the network, allowing direct mesh connections.

#### 2. "Backup state behind unit collection state" crash loop — fixed
Nodes with an oversized backup file (>10 MB) were stuck in a **permanent crash loop**:
```
WARN  Wiping backup alephbft_backup_3425.dat (10.6 MB)
INFO  BFT Proposer: running session_id=3425 ...
ERROR Backup state behind unit collection state. collection: 11, backup: 0
WARN  Session 3425 ended. Restarting in 30s...
```
The wipe cleared the file but AlephBFT restarted with the **same session_id**. It read an empty backup (round=0) but found in-memory state at round 11 → fatal mismatch → instant crash → repeat every 30 seconds.

**Fix**: `session_id` is incremented by +1 after any backup wipe. AlephBFT enters a fresh session with no prior state to reconcile. Crash loop eliminated.

#### 3. Watchdog → stuck session CPU loop — fixed
The WATCHDOG (kills sessions with no block for 10 min) did not wipe the backup before terminating. On restart, the same oversized backup was reloaded → immediately stuck again → WATCHDOG fired again → CPU pinned at ~100% permanently.

**Fix**: WATCHDOG wipes backups >5 MB before sending the kill signal. The restart loop sees a missing file, bumps `session_id`, starts a fresh session. Loop broken.

#### 4. Docker CPU cap: 3.0 → 2.0 cores
The 3.0-core limit was allowing the AlephBFT spin-loop to consume 118% CPU across all cores and trigger Docker OOM restarts. Reduced to 2.0 cores.

---

### Previous Release: v3.1.3-alpha (2026-08-20)

**Network Stall Fix at block 163,174**: Nodes that had their AlephBFT backup files wiped by the 10MB auto-wipe couldn't rejoin the current BFT session, causing "Backup state behind unit collection state" errors and stalled consensus. A hard-fork session rotation at height 163,174 forced a clean DAG restart across the network.
Protocol bumped to `54` (`QT54`).

---

### Previous Release: v3.1.2-alpha (2026-08-20)

1. **Block Sync CPU/Memory Spike**: Fixed 2000 concurrent goroutines spawned during block sync. Block streaming is now sequential, reducing peak RAM from ~2 GB to ~10 MB.
2. **AddrMan Cascade Ban**: Fixed a regression where failed Cloudflare ephemeral-port dial-backs were decreasing peer reputation and eventually banning Cloudflare edge nodes, shattering the network.

---

### Previous Release: v3.0.13-alpha (2026-08-19)

### The "Memory & CPU Stabilization" Release
This release resolves critical resource exhaustion issues:
1. **350% CPU Starvation (Consensus)**: Added a bounded LRU signature cache to `QuantaKeychain`. AlephBFT no longer endlessly re-verifies the exact same Falcon-512 signatures during DAG traversals, dropping the node's baseline CPU usage from ~350% to roughly 10-30%.
2. **6.4 GiB Memory Leak (OOM)**: Fixed a catastrophic memory leak where the P2P layer pumped decompressed BFT messages into an unbounded channel (`mpsc::unbounded_channel`) faster than the CPU-starved consensus could consume them. Replaced with a strictly bounded channel with a drop-on-full policy (`try_send`) to keep RAM flat.

**Network compatibility**: This release maintains network compatibility with v47 nodes.

---

### Previous Release: v3.0.12-alpha (2026-08-19)

### The "Networking" Release
This release fixes two major networking bugs:
1. **Network Disconnect Deadlock**: A synchronous lock during P2P block broadcasting could cause the node to stop responding to `Ping` messages during heavy sync, leading to mass peer disconnections and an apparent node stall at the sync tip.
2. **Bootstrap Node Spam**: A logic bug caused nodes with only inbound connections to aggressively fallback to bootstrap nodes, spamming the console with `Connecting to peer` every 30 seconds.

**Network compatibility**: This release bumps the protocol version to **47** (`QT47`), isolating it from v46 nodes. All node operators MUST update.

### Upgrade Instructions (For Validators & Full Nodes)

> **No database wipe required for v3.1.5-alpha.** This is a network-layer and consensus-stability fix only — chain data is fully compatible. Simply pull the new image and restart.

```bash
# 1. Stop your existing container
docker stop quanta_node1  # or: docker compose down

# 2. Pull the new image
docker pull xd637/quanta-node:latest

# 3. Restart
docker compose up -d
# — OR if running docker run directly:
docker run -d \
  --memory=4g \
  --name "quanta-validator" \
  --restart always \
  --network host \
  -v "/root/quanta_data:/home/quanta/quanta_data" \
  -e QUANTA_WALLET_PASSWORD="your-wallet-password" \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap node1.quantachain.org:8333
```

---

## Past Releases

> **v3.0.11-alpha — STATE SYNC INFINITE LOOP (2026-08-18)**
> - **Infinite snap-sync retry loop**: Fixed an infinite snap-sync retry loop at block 110,001.
> - **Protocol Bump (v46)**: Network isolated.
>

> **v3.0.10-alpha — SYNC BUG HOTFIX (2026-08-16)**
> - **Simulation logic fix**: Corrected forward-verification to perfectly match standard block application.
> - **Protocol Bump (v45)**: Network isolated.
>
> **v3.0.3-alpha — PERMANENT SYNC FIX (2026-08-13)** ✅
> - **Block 110,000 — Ethereum-Style Canonical State Root Checkpoint**: Permanently fixes node stalling at block 110,000 with `Invalid state root`. Due to 110,000 blocks of accumulated dust from the epoch pool 999-divisor bug, syncing nodes could not reproduce the exact pre-heal state the original proposer had. Added a `TESTNET_STATE_ROOT_CHECKPOINTS` system (like Ethereum's DAO fork) that hardcodes the canonical state root at block 110,000. **No database wipe required.** Full state root enforcement resumes from block 110,001.
> - **Network Isolation**: Bumped `PROTOCOL_VERSION` to `38` and `TESTNET_MAGIC` to `QT38`.
>
> **v3.0.2-alpha — SYNC BUG HOTFIX (2026-08-12)**
> - **Incomplete patch**: Extended the historical 999-divisor bug replication from `< 105,000` to `< 110,000`. This was necessary but not sufficient — see v3.0.3-alpha for the full permanent fix.
>
> **v3.0.1-alpha — SYNC BUG FIX (2026-08-07)**
> - Initial attempt to fix block 110,000 sync by reinstating the historical 999-divisor for blocks `< 105,000`. Incomplete.

> **v3.0.0-alpha (hotfix 2) — DATABASE MIGRATION FIX (2026-08-05)**
> - **No wipe required.** The node now seamlessly migrates your existing V2 database to the V3 format on first boot. Old blocks and account state are fully preserved.
>
> **v3.0.0-alpha — KATENET LAUNCH: MANDATORY UPDATE (2026-08-05)**
> - **Delegated Proof of Stake (DPoS)**: Consensus is no longer just for whales. Native delegation allows any QUA holder to lock their tokens behind a trusted BFT validator to secure the network and earn a share of block rewards.
> - **Network Isolation**: Bumped `PROTOCOL_VERSION` to `35` and `TESTNET_MAGIC` to `QT35` to cleanly evict all V2 nodes.

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
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap node1.quantachain.org:8333
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
      --bootstrap node1.quantachain.org:8333
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
./target/release/quanta start -c quanta.toml --validator-wallet ./quanta_data/validator.qua --bootstrap node1.quantachain.org:8333
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
