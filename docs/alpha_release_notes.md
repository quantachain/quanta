# Quanta Alpha Release Notes

## Current Version: v3.2.13-alpha

This release delivers the ultimate, bulletproof fix for the Cloudflare TCP Proxy mesh isolation issue, matching industry standards for node deployments behind complex networks.

### v3.2.13-alpha — TCP Proxy Mesh Fix (`--advertise-addr`)
- **Proxy Blindness Fix**: Added a new CLI flag `--advertise-addr <IP>` to allow validators hidden behind Layer 4 TCP proxies (like Cloudflare Spectrum) or complex NATs to explicitly declare their real public IP to the network. This IP is embedded directly into the P2P `Version` handshake, allowing the bootstrap node to gossip the real routable IP instead of the useless proxy socket IP. This restores full mesh connectivity and AlephBFT consensus block production.

#### 🛠️ Guide: How to use `--advertise-addr`
If your validator is connecting to a bootstrap node that is behind Cloudflare Spectrum, the bootstrap node cannot see your real IP. You **must** provide your validator's real, public VPS IP when starting the node. 
* **Example Usage:** 
  `./quanta --bootstrap node1.quantachain.org:8333 --advertise-addr 20.1.2.3`
* Replace `20.1.2.3` with the actual public IP address of the server running the validator. If you do not provide this flag, the bootstrap node will fail to verify you, and you will remain isolated from the rest of the network.
* **Custom Ports:** If you are running your node on a non-standard port (not 8333), you can include the port directly in the flag: `--advertise-addr 20.1.2.3:8334`. If you omit the port, it will automatically use your configured listen port.

## Previous Versions

### v3.2.12-alpha — GetAddr Starvation Fix
- **GetAddr Starvation (Self-Healing)**: Fixed a race condition where 21 validators connecting simultaneously to the bootstrap node received empty peer lists because the node hadn't verified anyone yet. The `maintain_peers` loop now detects if the discovery table is exhausted and periodically broadcasts `GetAddr` to connected peers to self-heal the network.

## Previous Releases (Summary)

| Version | Date | Summary |
|---|---|---|
| v3.2.12-alpha | 2026-09-07 | Mesh peer exchange (GetAddr), AlephBFT unicast relay fix, Gossipsub re-relay fix, misbehavior tracking security fix. Protocol `68` / Magic `QT68` |
| v3.2.11-alpha | 2026-09-07 | Mesh Discovery & Consensus Security Audit. Protocol `67` / Magic `QT67` |

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
  quanta start --validator-wallet /home/quanta/quanta_data/validator.qua --bootstrap node1.quantachain.org:8333 --advertise-addr YOUR_SERVER_IP
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
      --advertise-addr YOUR_SERVER_IP
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
