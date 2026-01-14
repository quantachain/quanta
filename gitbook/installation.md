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

## Docker Installation

You can also run Quanta using Docker:

```bash
docker-compose up -d
```

For testnet deployment:

```bash
docker-compose -f docker-compose.testnet.yml up -d
```
