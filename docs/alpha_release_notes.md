# Quanta Alpha Release Notes

## Current Version: v3.2.8-alpha — Sync Blockchain Database Self-Heal

This release patches a critical bug where nodes restarting from a non-zero block height were refusing to sync due to reading an artificially inflated `cumulative_work` from their database.

---

### 🔴 Mandatory Upgrade — Protocol v64
This is a **mandatory upgrade**. Nodes running older protocols will be rejected. The network magic has also been bumped to `QT64`. 
To rejoin: `docker-compose pull && docker-compose up -d`.

---

### What's New

#### Fixed: Sync Blockchain (Database Self-Heal)
- **The Bug**: Due to an older `deep_reorg` bug, some nodes had an artificially inflated `cumulative_work` persisted in their Sled database. Upon restart, this inflated local work caused nodes to refuse to sync from peers, believing their own chain was the heaviest (logging `"Already on the heaviest chain — no sync needed"`). 
- **The Fix**: The node now strictly sanitizes the stored `cumulative_work` against the deterministic expected value (`height as u128`) on startup. Corrupted databases are instantly healed, and nodes will correctly sync without needing a reset from block 0.
- **Protocol Bump**: Bumped `PROTOCOL_VERSION` to `64` and `TESTNET_MAGIC` to `QT64` to enforce a clean reset of the consensus participants.

---

## Previous Releases (Summary)

| Version | Date | Summary |
|---|---|---|
| v3.2.5-alpha | 2026-09-05 | BFT peer resolution fix — unicast messages were silently dropped due to missing `node_id` resolution in handshake handler |
| v3.2.4-alpha | 2026-09-04 | Connection tracking fix — libp2p ghost connections leaked on reconnect, causing capacity errors. Protocol bumped to v60 |
| v3.2.3-alpha | 2026-09-02 | Strict v58 handshake rejection + log noise reduction |
| v3.2.2-alpha | 2026-09-02 | Sync & stability — O(1) cumulative work calc, egress spam disabled, peer liveness fix. Protocol bumped to v59 |
| v3.2.0-alpha | 2026-09-01 | Swarm release — replaced raw TCP with libp2p Gossipsub + Kademlia DHT |
| v3.1.5-alpha | 2026-09-01 | State actor refactoring + sync deadlock fix. Protocol bumped to v56 |

For full history see [CHANGELOG.md](./CHANGELOG.md).

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

## Staking & Delegation

> **Minimum stake to become a validator: 100,000 QUA**. Your wallet's Falcon-512 public key is automatically used for BFT signing.

### Become a Validator (Stake)

```bash
# Register as a BFT validator by staking QUA
quanta-wallet stake \
  --wallet /home/quanta/quanta_data/validator.qua \
  --amount 100000 \
  --fee 0.01 \
  --node http://localhost:3000
```

### Stop Validating (Unstake)

```bash
# Deregister and begin the 2-epoch unbonding period
quanta-wallet unstake \
  --wallet /home/quanta/quanta_data/validator.qua \
  --fee 0.01 \
  --node http://localhost:3000
```

### Delegate to a Validator (Non-validators)

Don't want to run a node? Delegate your QUA to an active validator and earn a share of rewards.

```bash
# Delegate QUA to an existing validator
quanta-wallet delegate \
  --wallet /home/quanta/quanta_data/my_wallet.qua \
  --validator 0x0217a3fcbadd38e31761f9f949954e9f2ac2503d \
  --amount 10000 \
  --fee 0.01 \
  --node http://localhost:3000

# Undelegate (locks for unbonding period before becoming spendable)
quanta-wallet undelegate \
  --wallet /home/quanta/quanta_data/my_wallet.qua \
  --validator 0x0217a3fcbadd38e31761f9f949954e9f2ac2503d \
  --fee 0.01 \
  --node http://localhost:3000
```

---

## License

QUANTACHAIN operates under an **Open-Core Dual License** model:
- **Core Protocol**: [GNU AGPLv3](https://github.com/quantachain/quanta/blob/main/LICENSE)
- **Native Templates & APIs**: [QuantaLabs Commercial License](https://github.com/quantachain/quanta/blob/main/COMMERCIAL_LICENSE.md)

For commercial licensing: **contact@quantachain.org**
