# QuantaChain Alpha Testnet Release Notes

## Current Release: v3.0.11-alpha (2026-08-18)

### The "State Sync" Release — Infinite Loop Fix
This release closes an infinite loop bug that occurred at block 110,001. A syncing node that successfully applied the canonical post-heal snapshot (root `2ee3073...`) would incorrectly compare its local state to the block's hardcoded expected state root (`42db10a2...`), causing it to falsely detect divergence and continuously request the same snapshot. The sync logic now correctly validates the `2ee3...` healed state.

**Network compatibility**: This release bumps the protocol version to **46** (`QT46`), isolating it from v45 nodes. All node operators MUST update.

### Upgrade Instructions (For Validators & Full Nodes)

If your node was stuck at block 110,000 or 110,001, **you must wipe your old data and sync fresh**.
```bash
# 1. Stop your existing node container
docker stop quanta-node

# 2. Delete the corrupted local chain data (IMPORTANT: Do NOT delete your validator.qua wallet!)
rm -rf /root/quanta_data/blocks /root/quanta_data/db

# 3. Pull the new version
docker pull xd637/quanta-node:v3.0.11-alpha

# 4. Restart the node
docker run -d \
  --memory=3.5g \
  --memory-swap=3.5g \
  --name "quanta-node" \
  --restart always \
  --network host \
  -v "/root/quanta_data:/home/quanta/quanta_data" \
  -e QUANTA_WALLET_PASSWORD="your-wallet-password" \
  xd637/quanta-node:v3.0.11-alpha \
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap node1.quantachain.org:8333 --port 3002 --rpc-port 7783
```

---

## Past Releases

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
