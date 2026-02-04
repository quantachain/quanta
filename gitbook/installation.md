# Installation

Quanta requires Rust 1.70+ to build from source.

## System Requirements

- Rust 2021 edition or higher
- 4GB RAM minimum (8GB recommended)
- 20GB disk space for blockchain data
- Linux, macOS, or Windows

## Clone the Repository

```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
```

## Build with Release Optimizations

```bash
cargo build --release
```

## Run Tests

```bash
cargo test
```

## Binary Location

After building, the binary will be located at:

```bash
./target/release/quanta
```

## Docker Installation (Recommended)

The easiest way to run a Quanta node is using Docker. You can start a node with a single command:

```bash
docker run -d --name quanta-node -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 xd637/quanta-node:v0.1
```

This command will:
- Pull the latest `v0.1` image if not present
- Start the node in detached mode (`-d`)
- Expose the necessary ports:
  - `3000`: API
  - `8333`: P2P Network
  - `7782`: RPC
  - `9090`: Metrics

### Data Persistence

To ensure your blockchain data persists across restarts, mount a volume:

```bash
docker run -d --name quanta-node \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta_data:/home/quanta/quanta_data \
  xd637/quanta-node:v0.1
```
