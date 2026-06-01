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
The binary will be located at `./target/release/quanta`.

## Docker (Recommended)

You do not need to build from source if you use Docker.
```bash
docker pull xd637/quanta-node:latest
```

