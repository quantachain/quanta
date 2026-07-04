# Installation

## Prerequisites
- **Rust Toolchain**: 1.70+
- **Docker** (optional, recommended for production)

## Build from Source

```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
cargo build --release
```
The binaries will be located at:
- Node Daemon: `./target/release/quanta`
- Wallet CLI: `./target/release/quanta-wallet`

## Docker (Recommended)

You do not need to build from source if you use Docker.
```bash
docker pull xd637/quanta-node:latest
```

