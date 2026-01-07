# QUANTA

Quantum-resistant blockchain using Falcon-512 post-quantum signatures and Kyber-1024 encryption.

## Documentation

- **[Whitepaper](WHITEPAPER.md)** - Complete technical specification
- **[Tokenomics](TOKENOMICS.md)** - Economic model and supply schedule
- **[Security Audit](SECURITY_AUDIT.md)** - Pre-testnet security assessment
- **[Contributing](CONTRIBUTING.md)** - Development guidelines

## Why QUANTA?

### The Quantum Threat is Real

Current blockchain systems (Bitcoin, Ethereum, etc.) rely on elliptic curve cryptography (ECDSA/EdDSA) for transaction signatures. These are vulnerable to Shor's algorithm, which can be executed efficiently on sufficiently powerful quantum computers. Conservative estimates suggest such quantum computers could exist within 10-15 years.

### Why Build Now?

- **NIST PQC Standards Finalized (2024)**: Cryptographic primitives are mature and vetted
- **Migration is Hard**: Upgrading existing chains is significantly more difficult than building correctly from the start  
- **Future-Proofing**: Infrastructure decisions made today will persist for decades
- **No False Urgency**: This is insurance, not speculation. QUANTA is secure against classical attacks today and quantum attacks tomorrow.

### What Makes QUANTA Different?

QUANTA is not a research project or academic prototype. It's a production-ready blockchain built with:

1. **Post-Quantum Security**: NIST-standardized Falcon-512 signatures and Kyber-1024 encryption
2. **Sustainable Economics**: Adaptive tokenomics with perpetual mining incentives
3. **Operational Excellence**: Production-grade security, monitoring, and tooling
4. **Fair Launch**: No pre-mine, no ICO - 100% distributed through mining

## System Requirements

### Minimum Requirements (Full Node)
- **CPU**: 4 cores (2.0 GHz or higher)
- **RAM**: 8 GB
- **Storage**: 1 TB SSD (recommended for first year)
- **Bandwidth**: 50 Mbps down, 20 Mbps up
- **OS**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+

### Pruned Node
- **CPU**: 2 cores
- **RAM**: 4 GB
- **Storage**: 100 GB SSD
- **Bandwidth**: 25 Mbps down, 10 Mbps up

### Light Client (Planned)
- **CPU**: 1 core
- **RAM**: 1 GB
- **Storage**: 1 GB
- **Bandwidth**: 5 Mbps

### Storage Estimates

**Year 1 Projections (10 second blocks, 2000 tx/block average)**:
- **Block headers**: ~2 GB
- **Transactions**: ~350 GB
- **Signatures**: ~4.2 TB (Falcon-512 at ~666 bytes/signature)
- **State database**: ~50 GB
- **Total**: ~4.6 TB (full archival node)

**Pruned mode** (keeps only recent 6 months):
- **Total**: ~500 GB

**Note**: Signature compression and pruning strategies are planned for Phase 3 to reduce storage requirements.

### Bandwidth Estimates
- **Full node**: ~13 GB/day download, ~5 GB/day upload
- **Pruned node**: ~6 GB/day download, ~2 GB/day upload
- **Peak**: Up to 50 GB/day during initial sync

## Features

### Cryptography
- **Post-Quantum Signatures**: Falcon-512 (NIST Level 1, lattice-based)
- **Post-Quantum Encryption**: Kyber-1024 (NIST Level 5)
- **Quantum-Resistant Hashing**: SHA3-256 (Keccak)
- **Key Derivation**: Argon2id (memory-hard, quantum-safe)

### Consensus & Security
- Adaptive Proof-of-Work with dynamic difficulty adjustment
- Account-based model with nonce-based replay protection
- Transaction expiry (24-hour window)
- Merkle trees for SPV support
- Checkpoint system for deep reorganization prevention
- DoS protection (mempool limits, message size limits, peer banning)

### Economics
- Fair launch (no pre-mine, no ICO)
- Adaptive block rewards (100 QUA → 5 QUA floor over 20 years)
- 70% fee burning (deflationary pressure)
- 20% development treasury
- 50% mining reward lock (6-month vesting for anti-dump)
- Early adopter incentives (first 100k blocks)

### Infrastructure  
- Full P2P networking with DNS seed discovery
- REST API and JSON-RPC 2.0 daemon control
- Persistent blockchain storage (Sled database)
- HD wallets (BIP39 24-word mnemonic)
- Prometheus metrics export
- TOML configuration files
- Comprehensive test suite

## Tokenomics Summary

For complete economic specification, see [TOKENOMICS.md](TOKENOMICS.md).

### Supply Schedule
- **Year 1 Base Reward**: 100 QUA per block
- **Annual Reduction**: 15% per year (exponential decay)
- **Reward Floor**: 5 QUA (ensures perpetual mining incentive)
- **Asymptotic Maximum Supply**: ~1.5 billion QUA (year 15-20)
- **Block Time**: 10 seconds (~3.15 million blocks per year)

### Incentive Mechanisms

**Early Adopter Bonus**
- First 100,000 blocks (~11.5 days): 1.5x multiplier
- Attracts initial miners without long-term distortion

**Network Usage Multiplier**
- First 315,360 blocks (~36 days): up to 2x based on transaction fees
- Rewards genuine economic activity
- Fee-based calculation prevents spam attacks

**Anti-Dump Mechanism**
- 50% of mining rewards locked for 6 months (157,680 blocks)
- Aligns miner incentives with long-term network health
- Reduces immediate sell pressure during critical launch period

### Fee Distribution
- **70% Burned**: Permanent supply reduction (deflationary pressure)
- **20% Treasury**: Development funding, audits, grants
- **10% Miner**: Additional validator reward beyond block reward

### Long-Term Sustainability

Unlike Bitcoin's fixed supply model where mining revenue ends, QUANTA maintains perpetual security through:
- Minimum 5 QUA block reward (never reaches zero)
- Growing fee market as adoption increases
- Transition from reward-dominant to fee-dominant security over 20 years

## Security

For complete security analysis, see [WHITEPAPER.md](WHITEPAPER.md) and [SECURITY_AUDIT.md](SECURITY_AUDIT.md).

### Cryptographic Security
- **Classical Security**: SHA3-256 (2^256 operations), Falcon-512 (2^128 operations)
- **Quantum Security**: Lattice-based signatures (no known quantum attacks), Grover-resistant hashing
- **Key Protection**: Argon2id prevents brute-force attacks on encrypted wallets

### Network Security
- DoS protection: 2MB message limit, 5000 transaction mempool cap, peer connection limits
- Replay protection: Monotonic nonces, 24-hour transaction expiry
- 51% attack resistance: Checkpoint system, high reward attracts honest miners
- Timestamp validation: Blocks must be within 2 hours of current time

### Operational Security  
- Graceful shutdown handling (SIGINT/SIGTERM)
- Persistent state across restarts (atomic database transactions)
- Health check endpoints for monitoring
- Localhost-only RPC binding (no remote exposure)

### Attack Resistance
- **Double-Spend**: Confirmation depth mitigates (6 blocks recommended)
- **Transaction Replay**: Nonce-based prevention
- **Memory Exhaustion**: Orphan block limits (100 max), mempool size caps
- **Sybil Attack**: Proof-of-work requirement, connection limits
- **Harvest Now, Decrypt Later**: Kyber-1024 provides 256-bit quantum security

### Explicitly NOT Protected
- 51% attacks (inherent to PoW, mitigated by checkpoints)
- Eclipse attacks on network-isolated nodes
- Physical key extraction from compromised devices

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
- Prometheus metrics export (port 9090)
- Real-time blockchain metrics
- Network statistics
- Transaction throughput tracking
- Compatible with Grafana dashboards

## Roadmap

### Phase 1: Testnet (Q2-Q3 2026)
- **Q2 2026**: Public testnet launch
- Core functionality validation and stress testing
- Community building and documentation
- External security audits (2-3 independent firms)
- Bug bounty program launch
- Performance optimization

### Phase 2: Mainnet Preparation (Q4 2026)
- Address testnet findings and vulnerabilities
- Final security audit and code freeze
- Genesis block preparation
- Bootstrap node deployment (5+ geographic regions)
- Exchange partnership negotiations
- Third-party wallet integrations

### Phase 3: Mainnet Launch (Q1 2027)
- **Mainnet genesis** with coordinated launch
- Initial exchange integrations
- Official block explorer deployment
- Desktop wallet releases (Windows, macOS, Linux)
- Network monitoring and incident response readiness

### Phase 4: Expansion (Q2-Q4 2027)
- Light client protocol (SPV) implementation
- Signature aggregation and pruning optimization
- Mobile wallet releases (iOS, Android)
- Hardware wallet support (Ledger/Trezor partnership)
- Developer documentation and SDK

### Phase 5: Ecosystem (2028+)
- Smart contract layer (post-quantum VM research and implementation)
- Developer grants program
- DApp ecosystem growth
- Cross-chain bridges (requires quantum-resistant relay protocols)
- Layer 2 solutions exploration
- Privacy feature research (confidential transactions)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development guidelines.

## License

MIT
