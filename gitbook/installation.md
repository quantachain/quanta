# Installation

This page covers all methods to run a Quanta node. **Docker is the recommended method** — it requires no Rust toolchain and works on any operating system.

---

## System Requirements

| Type | CPU | RAM | Storage | Bandwidth |
|------|-----|-----|---------|-----------|
| **Full Node / Mining** | 4 cores @ 2 GHz | 8–16 GB | 1 TB SSD | 50/20 Mbps |
| **Pruned Node** | 2 cores @ 2 GHz | 4 GB | 400 GB SSD | 25/10 Mbps |

**OS Support**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+

---

## Method 1: Docker (Recommended)

Docker is the easiest and safest way to run a Quanta node. The official image is published to Docker Hub as `xd637/quanta-node`.

### Prerequisites

- [Docker Desktop](https://www.docker.com/products/docker-desktop/) (Windows/macOS) or Docker Engine (Linux)

### Pull and Run

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 \
  -p 8333:8333 \
  -p 7782:7782 \
  -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

### Verify It's Running

```bash
docker logs quanta-node --tail 30 -f
curl http://localhost:3000/health
```

A healthy response looks like:
```json
{"status":"healthy","blockchain_height":12345,"peer_count":8,"uptime_seconds":86400}
```

---

## Method 2: Docker Compose (Recommended for VPS)

For servers and production deployments, Docker Compose provides automatic restarts and cleaner configuration.

```bash
# Download the repository
git clone https://github.com/quantachain/quanta.git
cd quanta

# Start with the single-node compose file
docker compose -f docker-compose.single.yml up -d

# View logs
docker compose -f docker-compose.single.yml logs -f
```

To upgrade to a new release:
```bash
docker compose -f docker-compose.single.yml pull
docker compose -f docker-compose.single.yml up -d
```

No data wipe required between alpha releases (unless the release notes state otherwise).

---

## Method 3: Build from Source

Build the `quanta` binary directly from the Rust source code.

### Prerequisites

- **Rust** 1.70 or higher — install from [rustup.rs](https://rustup.rs/)
- **Git**
- **OpenSSL** 1.1.1+ (Linux) or LibreSSL 3.0+ (macOS)

### Steps

```bash
# Clone the repository
git clone https://github.com/quantachain/quanta.git
cd quanta

# Build the release binary (takes 5–15 minutes on first build)
cargo build --release

# Verify
./target/release/quanta --help
```

### Run the Node

```bash
./target/release/quanta start -c quanta.toml
```

---

## Node Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| **3000** | HTTP | REST API |
| **8333** | TCP | P2P networking |
| **7782** | TCP | RPC server (CLI to node) |
| **9090** | HTTP | Prometheus metrics |

Open all required ports on your firewall:

```bash
sudo ufw allow 8333/tcp
sudo ufw allow 3000/tcp
sudo ufw allow 7782/tcp
sudo ufw allow ssh
sudo ufw --force enable
```

---

## Testnet vs. Mainnet

The current public network is the **testnet**. Mainnet is planned for Q1 2027.

| Parameter | Testnet | Mainnet |
|-----------|---------|---------|
| Network ID | QUA7 | — |
| Genesis Hash | `00000012d3a2cbb7eb9579330ccdaa4f83ca9e6e016bfe6d2c8a38539cf3733b` | — |
| REST API Port | 3000 | 3000 |
| P2P Port | 8333 | 8333 |
| Do not use real funds | ✅ | — |

---

## Next Steps

- [Quick Start](quick-start.md) — start your node, create a wallet, mine your first block
- [Node Operator Guide](node-operator-guide.md) — full VPS and production setup
- [Mining Guide](mining-guide.md) — mining rewards, commands, and optimization
