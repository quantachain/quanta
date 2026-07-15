# QuantaChain Testnet — V2 Release (v2.4.0)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.4.1 — CRITICAL HOTFIX (2026-07-15)**
> **NO PROTOCOL BUMP.** Critical hotfix for P2P network stability:
> * **Flapping / Ban Fix**: Fixed a massive bug where any normal TCP stream disconnection (such as a timeout or intentional drop by a peer hitting its max connection limit) triggered an automatic 100-strike malicious behavior score. This caused nodes to instantly IP-ban each other on the slightest network hiccup, leading to a complete chain reaction network collapse and BFT stall. Dead streams are now cleanly marked without triggering bans.

> **v2.4.0 — MAJOR RELEASE (2026-07-15)**
> * **BFT Stability**: Fixed a BFT session restart timing bug that could cause block production to stall for 5+ minutes at session boundaries.
> * **Locking Optimization**: Explicitly drop blockchain write locks in all paths to reduce thread latency.
> * **Docker Cleanup**: Cleaned up redundant container configuration in docker-compose.

> **v2.3.9-alpha — MANDATORY PROTOCOL UPGRADE (2026-07-15)**
> **MANDATORY UPDATE (PROTOCOL V14, MAGIC=QT14).** All nodes MUST upgrade. This version permanently breaks compatibility with all prior versions to eliminate the stream corruption issue:
> * **Protocol Version Bump**: Increased `PROTOCOL_VERSION` to `14`. Nodes on v13 or below will be rejected at handshake.
> * **Network Magic Changed**: Updated magic bytes from `Q9TE` to `QT14`. Any node with the old magic will be instantly rejected before any data is exchanged, preventing TCP stream corruption.
> * **Signature Verification Logs**: Demoted `AlephBFT signature verification FAILED` from `WARN` to `DEBUG`.
> * **Decode Failure Logs**: Demoted `Failed to decode incoming AlephBFT message` from `WARN` to `DEBUG`.

> **v2.3.8-alpha — HIGH CPU & DATA CORRUPTION HOTFIX (2026-07-15)**
> **NO PROTOCOL BUMP.** Critical hotfix to resolve CPU spikes and data corruption:
> * **TCP Stream Framing Fix**: Fixed a massive bug where an I/O timeout during `send_message` or `receive_message` would leave partial bytes in the OS TCP buffer while keeping the stream open. This caused all subsequent messages to lose their framing, resulting in `Decompression read error`, `Could not decode 'NetworkDataInner'`, and extreme CPU spikes as the node repeatedly spun up tasks trying to decode garbage data or allocating massive chunks of memory (`Message too large`). Streams are now properly marked as corrupted and dead on any timeout.
> * **Broadcast CPU Optimization**: The BFT broadcasting loop now explicitly drops dead peers instantly instead of spinning up thousands of `tokio::spawn` tasks that all independently wait for network timeouts.

> **v2.3.7-alpha — LOG SPAM HOTFIX 2 (2026-07-15)**
> **NO PROTOCOL BUMP.** Cleaned up remaining terminal output:
> * **BFT Unicast Spam Fix**: Demoted a secondary `Unicast AlephBFT to ... failed` log to `DEBUG`. This fixes the remaining terminal spam when attempting to unicast to a newly disconnected validator.

> **v2.3.7-alpha — REMOVED DEBUG LOGS (2026-07-15)**
> **NO PROTOCOL BUMP.** Cleaned up terminal output:
> * **BFT Observability**: Removed the verbose diagnostic logs displaying the quorum size (`f` and `2f+1`) that were added during the consensus stall debugging phase, returning to the standard session start logs.

> **v2.3.6-alpha — LOG SPAM HOTFIX (2026-07-15)**
> **NO PROTOCOL BUMP.** Hotfix to resolve terminal log spam during node disconnections:
> * **BFT Broadcast Spam Fix**: Demoted the `Failed to send message to peer` log from `WARN` to `DEBUG`. This prevents the node from spamming the terminal with hundreds of warnings per second when it attempts to broadcast consensus messages to a recently disconnected peer before the dead-peer cleanup cycle removes them.

> **v2.3.5-alpha — BFT CONSENSUS RETRY HOTFIX (2026-07-15)**
> **MANDATORY UPDATE (PROTOCOL V13).** Critical hotfix to resolve consensus stalling when validators drop units during initialization:
> * **Protocol Version Bump**: Increased `PROTOCOL_VERSION` to `13` to strictly isolate the network from older versions that drop retry messages.
> * **BFT Gossip Relay Fix**: Fixed a bug where the P2P LRU cache aggressively dropped all AlephBFT retries from being relayed, permanently starving validators and stalling the DAG consensus. Retries are now properly identified and gossiped across the network.
> * **BFT Quorum Observability**: Added verbose diagnostic logging in the `BFT Proposer` task to clearly display validator index, total committee size, fault tolerance ($f$), and required quorum size ($2f+1$) during session initialization.

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
