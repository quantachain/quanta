# QuantaChain Testnet — V2 Release (v2.3.4-alpha)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.3.4-alpha — P2P FLAPPING & PORT CONFLICT HOTFIX (2026-07-15)**
> **MANDATORY UPDATE (PROTOCOL V12).** Critical hotfix to resolve consensus stalling and multi-node Docker deployments:
> * **Protocol Version Bump**: Increased `PROTOCOL_VERSION` to `12` to strictly isolate the network from old v11 nodes causing silent TCP connection resets and P2P flapping loops.
> * **Handshake Observability**: Added explicit `WARN` logging for handshake timeouts and version mismatches instead of silently dropping peers.
> * **Docker Port Conflicts Fixed**: Fixed a critical bug where multi-node `docker-compose` clusters crashed in an infinite loop due to environment variable configurations (e.g., `QUANTA_NODE__API_PORT`) being ignored. The recommended `docker-compose` topology now uses explicit CLI port overrides (`--port`, `--rpc-port`, `--network-port`).
>
> **v2.3.3-alpha — NETWORK STALL & API BIND HOTFIX (2026-07-15)**
> **NO WIPE REQUIRED.** Critical hotfix to restore block production and API visibility:
> * **Network Discovery Fix**: Re-enabled gossiping of inbound connections. This prevents the network from forming a disconnected "star topology" around bootstrap nodes, allowing AlephBFT validators to discover each other and reach consensus.
> * **API Bind Fix**: The REST API now securely defaults to `0.0.0.0` (configurable via `api_bind_host` in `quanta.toml`), fixing the issue where LUA and other nodes appeared OFFLINE to block explorers.
> * **Network Magic Bumped**: Changed network magic to `Q9TE` and protocol version to `11`. Ensure all nodes in your cluster are updated.

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
git checkout v2.3.4-alpha
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
