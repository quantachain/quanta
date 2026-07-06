# QuantaChain Testnet — V2 Release (v2.1.1-alpha)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.1.1-alpha — SOFT UPDATE (2026-07-06)**
> **NO WIPE REQUIRED.** A bypass was added for block 12615 to resolve the state root consensus bug. The blockchain data format and network magic (`Q2T9`) have not changed. DO NOT delete your `quanta_data` folder!
> 
> **How to update and start your node:**
> 
> ```bash
> docker pull xd637/quanta-node:latest
> 
> docker run -d \
>   --name quanta-validator \
>   --restart always \
>   --network host \
>   -v ~/quanta_data_v2:/home/quanta/quanta_data \
>   -e QUANTA_WALLET_PASSWORD="YOUR_PASSWORD_HERE" \
>   xd637/quanta-node:latest \
>   quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 34.87.128.33:8333
> ```

This is a pre-release testnet build. Do not use real funds. APIs and chain parameters may change between alpha releases.

---

## What Changed in v2.1.0-alpha

- **Consensus & Network Resilience**
  - **Fixed BFT session termination**: Replaced `return None` with an async wait in `get_data` to ensure the session remains active while waiting for blocks.
  - **Fixed 'Backup state behind' errors**: BFT backups are now exclusively wiped during actual session transitions rather than upon restarts.
  - **Sync-Wait Loop added**: Nodes will now fully sync the blockchain before initializing BFT sessions, preventing old-session network spam.
  - **Partition Cascade fixed**: Lowered the aggressive connection-failure peer ban duration from 24 hours to 60 seconds, preventing network halt during sequential node restarts.
- **Documentation & DX**
  - Refactored README, updated badges, and synced GitBook with V2 CLI/API updates.
  - Added a comprehensive JSON-RPC guide.
- **Codebase Optimization**
  - Cleaned up extensive dead code across the consensus, network, core, and RPC modules.
  - Fixed database pruning mechanics.

---

## What Changed in v2.0.2-alpha

- **Dynamic Validator Expansion** — `MAX_COMMITTEE_SIZE` increased to 21 to allow dynamic expansion of the active validator set.
- **Genesis Sybil Protection** — Genesis validators removed from the liquid `testnet_faucets` array. This ensures genesis validators only receive locked stake and no liquid QUA, mathematically preventing Sybil attacks.
- **Network Isolation** — Network magic bytes bumped to `Q2T9` for testnet reset.
- **AlephBFT unicast routing fixed** — BFT vote/signature messages now route to the single intended validator instead of broadcasting to all. Cuts ~80% of inter-node BFT traffic (was O(N²), now O(N)).
- **Peer flapping loop fixed** — Dead peers now evicted within 30 s instead of holding their IP slot for 180 s. Stops the "Connection reset by peer" reconnect storm that was generating hundreds of MB/hr of spurious mempool traffic.
- **Heartbeat interval corrected** — was firing every 10 s, now correctly 60 s per protocol spec. ~83% fewer Ping/Pong messages.
- **GetMempool on reconnect guarded** — no longer fires unconditionally on every peer connect; only pulls if local mempool is empty.

**Estimated bandwidth impact (7 validators): ~5 GB/day → ~1 GB/day per node.**

---

## What Changed in v2.0.1

- **Hard Fork (Network Magic Bump)** — Network magic bytes bumped to `Q2T7` to isolate v2.0.1 nodes due to breaking block size and consensus changes.
- **Smart Contracts V3** — Full AI contract layer with 5 native templates (Escrow, AgentJob, AgentBid, Stream, AgentRegistry).
- **Staking & Slashing** — Full validator staking, slashing, and unbonding logic. Open validator registration.
- **Performance Fixes** — Block size increased to 4MB, TPS ceiling ~400. Round-robin proposer timeout removed to guarantee 6s blocks.
- **Validator Setup** — Added `setup-validator.sh` script for secure, verifiable deployment.
- **HD Wallet Key Derivation** — Deterministic Falcon-512 keypair derivation from account seed.
- **Block timing fixed (v2.0.0)** — Blocks were slowing from 6s to 1–2h due to timestamp drift.
- **Genesis Reset (v2.0.0)** — Replaced lost validator wallet, rotated faucet wallets, new genesis hash.

---

## 🚨 V2 Hard Fork Details 🚨
- **Consensus Engine:** Migrated from SHA3-256 Proof of Work to AlephBFT (Asynchronous Byzantine Fault Tolerance).
- **Network Isolation:** Updated network magic bytes to `Q2T4` to prevent old nodes from connecting to the new consensus network.
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
| Timestamp | `1780704001` (2026-06-06 00:00:01 UTC) |
| Testnet Genesis Hash | `ae37fe2f40a7e7dbe6d2d1337f260d57185ef5fb169008e2600f245809fd1fbf` |
| Faucet 0 (API sender) | `0xec4f49553e31f22b27a83036a044aff7d697f524` |
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

### Docker Validators (Safe Wipe)

If you are an existing validator using Docker, you must wipe the old chain data but **KEEP your validator wallet file**. Since you may have named your wallet file something other than `validator.qua`, you must be extremely careful.

> [!CAUTION]
> If you run the `rm -rf` command below without successfully moving your wallet out first, you will delete your validator wallet forever!

**Step 1:** Stop your node and move your wallet out of the data folder. (Replace `validator.qua` with your actual wallet filename if it is different!)
```bash
docker stop quanta-validator
mv ~/quanta_data_v2/validator.qua ~/validator.qua.backup
```

**Step 2:** Verify your wallet is safe in your home directory, then wipe the old data.
```bash
ls ~/validator.qua.backup     # Verify it exists!
sudo rm -rf ~/quanta_data_v2/*
```

**Step 3:** Move your wallet back in (again, replacing `validator.qua` with your actual filename if different), and restart the node.
```bash
mv ~/validator.qua.backup ~/quanta_data_v2/validator.qua
docker start quanta-validator
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
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap 34.87.128.33:8333
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

| Component | License |
|---|---|
| Core Protocol | [GNU AGPLv3](https://github.com/quantachain/quanta/blob/main/LICENSE) |
| Native Templates & APIs | [QuantaLabs Commercial License](https://github.com/quantachain/quanta/blob/main/COMMERCIAL_LICENSE.md) |

For commercial licensing: **contact@quantachain.org**
