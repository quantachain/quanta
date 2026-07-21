# QuantaChain Testnet — V2 Release (v2.4.32-alpha)

Post-quantum secure blockchain using Falcon-512 signatures and **Asynchronous Byzantine Fault Tolerance (AlephBFT)**.

> **v2.4.32-alpha — BFT BLOCK TIME REGRESSION FIX: (2026-07-21)**
> **MANDATORY UPDATE**.
> - **BFT Block Time Fix**: Reverted the v2.4.31 `unit_creation_delay` targeted backoff which caused block times to inflate from ~6s to 30s+. Restored a strict constant 500ms for all rounds — the 600s session watchdog handles partition CPU spikes.

> **v2.4.31-alpha** — State convergence fix, reward visibility events, state root exemption to 105,000, Protocol v30 / QT30.

> **v2.4.30-alpha** — Network Isolation (Protocol v29 / QT29) to remove nodes with v2.4.27 backoff.
> **v2.4.29-alpha** — Reverted v2.4.27 `unit_creation_delay` backoff (30s block time fix).
> **v2.4.28-alpha** — State root exemption window extended to 100,000–102,000 (MANDATORY).
> **v2.4.27-alpha** — BFT CPU spike hotfix (linear backoff — **REVERTED in v2.4.29**).
> **v2.4.25-alpha** — BFT CPU spike & network spam isolation.
> **v2.4.23-alpha** — Mid-session unstake hotfix (Protocol v23 / QT23).
> **v2.4.22-alpha** — Unicast routing hotfix (Protocol v22 / QT22).
> **v2.4.21-alpha** — AlephBFT CPU spike filter removal.
> **v2.4.20-alpha** — Network isolation (Protocol v21 / QT21) to fix collisions.
> **v2.4.19-alpha** — DAG corruption recovery (Hard Fork Session 1362).

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
