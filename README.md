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
- **Merkle trees for SPV support**
- **HD wallets (BIP39 compatible)**
- **Prometheus metrics export**
- **TOML configuration files**
- **Comprehensive test suite**
- **Production-ready security features**

## Security Features

### DoS Protection
- Mempool size limit: 5,000 transactions max
- Block size limit: 1 MB maximum
- Block transaction limit: 2,000 transactions per block
- Minimum transaction fee: 0.0001 QUA (anti-spam)
- Request timeouts: 30 seconds

### Transaction Security
- Replay protection: 24-hour expiry
- Signature verification (Falcon-512)
- Balance validation
- Duplicate transaction detection
- Fee validation

### Operational Safety
- Graceful shutdown handling (Ctrl+C)
- Persistent state across restarts
- Health check endpoint (`/health`)
- Transaction sorting by fee priority

## Installation

```bash
cargo build --release
cargo test  # Run test suite
```

## Configuration

Create a `quanta.toml` file for node configuration:

```toml
[node]
api_port = 3000
network_port = 8333
db_path = "./quanta_data"

[security]
max_mempool_size = 5000
min_transaction_fee = 0.0001

[metrics]
enabled = true
port = 9090
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

- GET /health - Health check and node status
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
quanta start [OPTIONS]               # Start node
quanta new-wallet --file FILE        # Create quantum wallet
quanta new-hd-wallet --file FILE     # Create HD wallet (24-word mnemonic)
quanta mine --wallet FILE            # Mine blocks
quanta send --wallet FILE --to ADDR --amount AMOUNT
quanta stats                         # Show statistics
quanta validate                      # Validate blockchain
```

## Advanced Features

### Merkle Trees
- Efficient transaction verification
- SPV (Simplified Payment Verification) support
- Light client ready
- Proof generation and verification

### HD Wallets
- BIP39 mnemonic (24 words)
- Multiple accounts from one seed
- Deterministic key derivation
- Easy backup and restore

### Monitoring
- Prometheus metrics export
- Real-time blockchain metrics
- Network statistics
- Transaction throughput tracking
- Compatible with Grafana dashboards

### Testing
```bash
cargo test                    # Run all tests
cargo test blockchain_tests   # Blockchain tests only
cargo test merkle_tests       # Merkle tree tests
```

## Security

- Falcon-512 signatures
- Kyber-1024 encryption
- SHA3-256 hashing
- Argon2 key derivation

## License

MIT
