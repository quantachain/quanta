# Configuration

Quanta nodes are configured via `quanta.toml`. When using Docker, the configuration is baked into the image — you can mount a custom `quanta.toml` to override defaults.

---

## Full Configuration Reference

```toml
[node]
# REST API port
api_port = 3000

# P2P networking port
network_port = 8333

# RPC server port (used by CLI to control the node)
rpc_port = 7782

# Path to the blockchain database directory
db_path = "./quanta_data"

# Disable P2P networking (useful for offline testing)
no_network = false

# Node storage mode:
#   archive = keep all blocks from genesis (default)
#   pruned  = keep last N days only
#   light   = headers only (planned)
mode = "archive"

# Only used when mode = "pruned"
prune_days = 30

[network]
# Maximum number of peer connections
max_peers = 125

# Bootstrap peers to connect to on startup
bootstrap_nodes = [
  "testnet-us-east.quanta.network:8333",
  "testnet-eu-west.quanta.network:8333"
]

# DNS seeds for automatic peer discovery
dns_seeds = [
  "seed.testnet.quanta.network",
  "nodes.testnet.quanta.network",
  "peers.testnet.quanta.network"
]

[security]
# Maximum number of pending transactions in the mempool
max_mempool_size = 5000

# Reject transactions older than this many seconds (24 hours)
transaction_expiry_seconds = 86400

[metrics]
# Enable Prometheus metrics endpoint
enabled = true

# Prometheus metrics port
port = 9090

# Consensus engine:
#   proof_of_work = live (default)
#   proof_of_stake = not yet implemented; node refuses to start if set
consensus_engine = "proof_of_work"
```

---

## Using a Custom Config with Docker

Mount your `quanta.toml` into the container:

```bash
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v ~/quanta.toml:/home/quanta/quanta.toml \
  -v ~/quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:latest
```

---

## Command-Line Overrides

Most config values can be overridden at startup without editing `quanta.toml`:

```bash
quanta start \
  --config quanta.toml \
  --network testnet \
  --port 3000 \
  --network-port 8333 \
  --rpc-port 7782 \
  --db ./quanta_data \
  --bootstrap 192.168.1.10:8333,192.168.1.11:8333 \
  --no-network \
  --detach
```

---

## Running Multiple Nodes (Local Testing)

Each node needs unique ports and a separate data directory:

```bash
# Node 1 — bootstrap node
./quanta start --detach \
  --network-port 8333 --port 3000 --rpc-port 7782 \
  --db ./node1_data

# Node 2 — connects to Node 1
./quanta start --detach \
  --network-port 8334 --port 3001 --rpc-port 7783 \
  --db ./node2_data \
  --bootstrap 127.0.0.1:8333

# Check both
./quanta peers --rpc-port 7782
./quanta peers --rpc-port 7783
```

---

## Network: Testnet vs. Mainnet

The `--network` flag sets the magic bytes used in P2P handshakes, preventing cross-network contamination:

| Network | Magic Bytes | Genesis Hash |
|---------|-------------|-------------|
| `testnet` | `QUAX` (0x51554158) | `00000012d3a2cbb7eb9579330ccdaa4f83ca9e6e016bfe6d2c8a38539cf3733b` |
| `mainnet` | `QUAM` (0x5155414D) | `1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0` |

Testnet nodes and mainnet nodes cannot communicate — the genesis hashes are different and the magic bytes prevent handshake completion.

---

## Bootstrap Nodes (Testnet)

```
testnet-us-east.quanta.network:8333
testnet-us-west.quanta.network:8333
testnet-eu-west.quanta.network:8333
testnet-eu-central.quanta.network:8333
testnet-ap-southeast.quanta.network:8333
testnet-ap-northeast.quanta.network:8333
```

These are configured automatically in the Docker image. You do not need to set them manually unless you are building from source with a custom `quanta.toml`.
