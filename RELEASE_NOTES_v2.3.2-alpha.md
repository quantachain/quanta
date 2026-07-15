# QuantaChain v2.3.2-alpha Release Notes & Validator Guide

This release addresses critical network stalling issues and API visibility problems that affected the v2.2.9 release. It also establishes the standard procedure for validators to configure their nodes using Docker and native source.

## What's Changed in this Release
* **Fixed Network Discovery Bug:** Resolved an issue where nodes would not gossip inbound connections. This previously caused the network to form a "star topology," preventing AlephBFT from reaching consensus and stalling the chain at session boundaries. Nodes will now correctly discover each other and form a dense P2P mesh.
* **API Bind Configuration:** The REST API now defaults to `0.0.0.0` securely via configuration. Previously, it was hardcoded to `127.0.0.1` unless TLS was enabled, which caused nodes (like `LUA`) to appear offline to external block explorers. 
* **Added `api_bind_host`:** You can now explicitly configure the API bind interface in `quanta.toml` (defaults to `0.0.0.0`).

---

## 🛠️ How to Run a Validator Node

To ensure the network scales healthily, we recommend **all validators mount their own `quanta.toml` file**. This overrides the default baked-in configuration and allows validators to update their `bootstrap_nodes` without needing to wait for a new Docker image if IP addresses change.

### Option 1: Running via Docker (Recommended)

**1. Create your local `quanta.toml`**
Create a file named `quanta.toml` on your server. You can copy the default one and update the network section to point to the main bootstrap VPS:

```toml
[network]
max_peers = 125
# Replace 'YOUR_VPS_STATIC_IP' with the actual public IP of your primary node (e.g., LUA)
bootstrap_nodes = ["YOUR_VPS_STATIC_IP:8333"]
dns_seeds = []
```

**2. Create a `docker-compose.yml` file**
```yaml
version: '3.8'
services:
  quanta-node:
    image: xd637/quanta-node:v2.3.2-alpha
    container_name: quanta_validator
    restart: unless-stopped
    network_mode: "host"
    volumes:
      # Mount your data directory
      - ./quanta_data:/home/quanta/quanta_data
      # Mount your wallet
      - ./validator.qua:/home/quanta/quanta_data/validator.qua
      # OVERRIDE the baked configuration with your local file
      - ./quanta.toml:/home/quanta/quanta.toml
    environment:
      - QUANTA_WALLET_PASSWORD=YourSecurePasswordHere
    command: >
      quanta start
      -c /home/quanta/quanta.toml
      --validator-wallet /home/quanta/quanta_data/validator.qua
```

**3. Start the Node**
```bash
docker compose up -d
```

> **Why Volume Mounting?** 
> If a bootstrap peer goes offline, validators can simply open their local `quanta.toml`, add a new peer's IP to `bootstrap_nodes`, and restart their node (`docker compose restart`). They do not have to wait for the core team to release a new Docker image.

---

### Option 2: Running via Native Source

For developers or those who prefer running natively without Docker:

**1. Clone and Build**
```bash
git clone https://github.com/your-repo/quanta.git
cd quanta
git checkout v2.3.2-alpha
cargo build --release
```

**2. Configure `quanta.toml`**
Ensure your `quanta.toml` has the correct bootstrap IP in the `[network]` section.

**3. Run the Node**
```bash
./target/release/quanta start -c quanta.toml --validator-wallet ./quanta_data/validator.qua
```

---

## Network Architecture & Configuration Tips

* **Bootstrap Nodes vs DNS Seeds:** 
  * Currently, the network relies on **Bootstrap Nodes** (Static IPs). The central VPS running the 4 initial nodes acts as the main entry point for the network.
  * In the future, we will set up **DNS Seeds** (e.g., `seed.quantachain.org`). A DNS seed automatically resolves to a list of active node IPs, meaning validators will no longer need to manually update static IPs in their `quanta.toml`. Until then, stick to using static IPs in the `bootstrap_nodes` array.
* **Firewall Ports:** All validators MUST ensure port `8333` (TCP) is open to the public on their firewalls/routers. If this port is closed, the node cannot accept inbound connections, which weakens the P2P mesh and slows down BFT consensus.
* **API Port:** Ensure port `3000` (TCP) is open if you want your node to be visible on the public block explorer.
