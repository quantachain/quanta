# QUANTA

Quantum-resistant blockchain using Falcon-512 post-quantum signatures and Kyber-1024 encryption.

## Features

- Post-quantum cryptography (NIST PQC standards)
- Account-based transactions with nonce protection
- Adaptive proof-of-work mining with modern tokenomics
- Full P2P networking with peer discovery
- REST API and JSON-RPC daemon control
- Persistent blockchain storage (Sled database)
- Quantum-safe encrypted wallets
- Merkle trees for SPV support
- HD wallets (BIP39 24-word mnemonic)
- Prometheus metrics export
- TOML configuration files
- Comprehensive test suite
- Production-ready security features

## Tokenomics

### Adaptive Block Rewards (Solana-inspired)
- Year 1 base reward: 100 QUA
- Annual reduction: 15% per year
- Minimum floor: 5 QUA (reached after ~20 years)
- Total supply: Asymptotically approaches ~1.5 billion QUA

### Early Adopter Incentives
- First 100,000 blocks (~11.5 days): 1.5x multiplier
- Bootstrap phase (first month): Dynamic network usage adjustments
- Encourages early participation and transaction activity

### Anti-Dump Mechanism
- 50% of mining rewards locked for 6 months (157,680 blocks)
- Prevents immediate sell pressure
- Promotes long-term holder alignment

### Fee Distribution
- 70% burned (deflationary pressure)
- 20% to development treasury
- 10% to block validator (miner)
- Base fee: 0.001 QUA (prevents spam)

### Network Usage Multiplier
- During bootstrap phase, rewards scale with network activity
- Multiplier based on total fees paid (not transaction count)
- Prevents spam while rewarding genuine usage
- Up to 2x rewards during high economic activity

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
rpc_port = 7782
db_path = "./quanta_data"
no_network = false

[network]
max_peers = 125
bootstrap_nodes = []

[consensus]
max_block_transactions = 2000
max_block_size_bytes = 1_048_576
min_transaction_fee_microunits = 100
transaction_expiry_blocks = 8640
coinbase_maturity = 100

[security]
max_mempool_size = 5000
transaction_expiry_seconds = 86400
enable_rate_limiting = true
rate_limit_per_minute = 60
enable_peer_banning = true
require_tls = false

[mining]
year_1_reward_microunits = 100_000_000
annual_reduction_percent = 15
min_reward_microunits = 5_000_000
blocks_per_year = 3_153_600
early_adopter_bonus_blocks = 100_000
early_adopter_multiplier = 1.5
bootstrap_phase_blocks = 315_360
mining_reward_lock_percent = 50
mining_reward_lock_blocks = 157_680
fee_burn_percent = 70
fee_treasury_percent = 20
fee_validator_percent = 10
target_block_time = 10
difficulty_adjustment_interval = 10

[metrics]
enabled = true
port = 9090
```

## Quick Start

### 1. Start Node as Daemon
```bash
./target/release/quanta start --detach
```

### 2. Check Node Status
```bash
./target/release/quanta status
./target/release/quanta mining_status
./target/release/quanta print_height
```detach --network-port 8333 --port 3000 --rpc-port 7782 --db ./node1_data

# Node 2
./target/release/quanta start --detach --network-port 8334 --port 3001 --rpc-port 7783 --db ./node2_data --bootstrap 127.0.0.1:8333

# Check nodes
./target/release/quanta status --rpc-port 7782
./target/release/quanta status --rpc-port 7783
./target/release/quanta peers --rpc-port 7782
```

## REST API

### Endpoints
- GET /health - Health check and node status
- GET /api/stats - Blockchain statistics
- POST /api/balance - Get address balance
- POST /api/transaction - Submit transaction
- POST /api/mine - Mine a block
- GET /api/validate - Validate blockchain
- GET /api/peers - Get connected peers
- GET /api/metrics - Get node metrics
- GET /api/block/:height - Get specific block
- GET /api/mempool - Get pending transactions
- POST /api/merkle/proof - Get Merkle proof

## JSON-RPC Daemon Control

The RPC server (default port 7782) provides daemon control via JSON-RPC 2.0:

### RPC Methods
- `node_status` - Get node status and uptime
- `mining_status` - Get mining state and statistics
- `start_mining` - Start mining to address
- `stop_mining` - Stop mining
- `get_block` - Get block by height
- `get_balance` - Get address balance
- `get_peers` - List connected peers
- `get Commands

### Node Management
```bash
quanta start [OPTIONS]                    # Start node
quanta start --detach                     # Start as daemon
quanta status [--rpc-port PORT]           # Check node status
quanta stop [--rpc-port PORT]             # Stop daemon
quanta print_height [--rpc-port PORT]     # Show blockchain height
quanta peers [--rpc-port PORT]            # List connected peers
quanta get_block HEIGHT [--rpc-port PORT] # Get block info
```

### Wallet Management
```bash
quanta new_wallet --file FILE             # Create quantum-safe wallet
quanta new_hd_wallet --file FILE          # Create HD wallet (24-word mnemonic)
quanta wallet --file FILE                 # Show wallet info
quanta hd_wallet --file FILE              # Show HD wallet info
```

### Mining
```bash
quanta mining_status [--rpc-port PORT]    # Check mining status
quanta start_mining ADDRESS [--rpc-port PORT]  # Start mining to address
quanta stop_mining [--rpc-port PORT]      # Stop mining
quanta mine --wallet FILE --db PATH       # Mine single block (legacy)
```

### Transactions
```bash
quanta send --wallet FILE --to ADDR --amount AMOUNT --db PATH
```

### Blockchain Operations
```bash
quanta stats --db PATH                    # Show blockchain statistics
quanta validate --db PATH                 # Validate blockchain integrity
quanta demo --db PATH                     # Run demo with sample transactions
```

### Examples
```bash
# Start daemon
./quanta start --detach --port 3000 --network-port 8333 --rpc-port 7782

# Create wallet
./quanta new_wallet --file miner.qua

# Start mining
./quanta start_mining <YOUR_ADDRESS> --rpc-port 7782

# Check status
./quanta status
./quanta mining_status
./quanta print_height

# Stop everything
./quanta stop_mining
./quanta stop

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

## Security

- Falcon-512 signatures
- Kyber-1024 encryption
- SHA3-256 hashing
- Argon2 key derivation

## License

MIT
