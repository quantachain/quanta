# QUANTA

Quantum-resistant blockchain using Falcon-512 and Kyber-1024.

## Features

- Post-quantum cryptography
- UTXO transactions
- Proof-of-work mining
- P2P networking
- REST API
- Persistent storage
- Encrypted wallets

## Installation

```bash
cargo build --release
```

## Quick Start

```bash
./target/release/quanta new-wallet --file miner.qua
./target/release/quanta start --port 3000
curl -X POST http://localhost:3000/api/mine -H "Content-Type: application/json" -d '{"miner_address": "ADDRESS"}'
```

## P2P Network

```bash
# Node 1
./target/release/quanta start --network-port 8333 --port 3000 --db ./node1

# Node 2
./target/release/quanta start --network-port 8334 --port 3001 --db ./node2 --bootstrap 127.0.0.1:8333
```

## API

- GET /api/stats
- POST /api/balance
- POST /api/mine
- POST /api/mine/start
- POST /api/mine/stop
- GET /api/mine/status
- GET /api/block/:height
- GET /api/mempool
- GET /api/peers

## CLI

```bash
quanta start [OPTIONS]
quanta new-wallet --file FILE
quanta mine --wallet FILE
quanta send --wallet FILE --to ADDRESS --amount AMOUNT
quanta stats
```

## Security

- Falcon-512 signatures
- Kyber-1024 encryption
- SHA3-256 hashing
- Argon2 key derivation

## License

MIT
