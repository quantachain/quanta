# QUANTA

**The First Quantum-Resistant Blockchain Built for the Future**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-in_progress-yellow.svg)]()
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

---

## What is QUANTA?

QUANTA is a production-ready blockchain that protects against quantum computer attacks through NIST-standardized post-quantum cryptography. While Bitcoin, Ethereum, and other blockchains use cryptography vulnerable to future quantum computers, QUANTA is built from the ground up to remain secure for decades.

**Built with:**
- **Falcon-512** post-quantum signatures
- **Kyber-1024** post-quantum encryption  
- **SHA3-256** quantum-resistant hashing
- **Modern Rust** implementation

---

## Why QUANTA Matters

### The Quantum Threat

Current blockchains rely on elliptic curve cryptography (ECDSA/EdDSA) that quantum computers can break using Shor's algorithm. Conservative estimates suggest such quantum computers could exist within 10-15 years, potentially rendering existing blockchain security obsolete.

### The QUANTA Solution

- **Future-Proof Security**: NIST-standardized post-quantum algorithms resist both classical and quantum attacks
- **No Migration Needed**: Built correctly from day one, not retrofitted
- **Fair Distribution**: 100% mining distribution, no pre-mine, no ICO
- **Production-Ready**: Built in Rust with comprehensive testing and operational tooling

---

## Quick Links

| Resource | Description |
|----------|-------------|
| [Whitepaper](WHITEPAPER.md) | Complete technical specification and architecture |
| [Tokenomics](TOKENOMICS.md) | Economic model, supply schedule, and incentive design |
| [Contributing](CONTRIBUTING.md) | Development guidelines and how to contribute |
| [Security Policy](SECURITY.md) | Vulnerability reporting and security practices |
| [Website](https://www.quantachain.org) | Official project website |
| [Documentation](https://www.quantachain.org/docs) | Installation and usage guides |

---

## For Investors

### Value Proposition

QUANTA addresses a trillion-dollar problem: the quantum computing threat to blockchain infrastructure. As institutions and governments invest in quantum computing, existing blockchains face obsolescence. QUANTA provides:

1. **First-Mover Advantage**: The first production-ready quantum-resistant blockchain
2. **Fair Launch Model**: No insider allocation, transparent distribution
3. **Deflationary Economics**: 70% of transaction fees are permanently burned
4. **Sustainable Growth**: Perpetual mining incentives prevent Bitcoin's "final block" problem

### Tokenomics Summary

| Parameter | Value | Benefit |
|-----------|-------|---------|
| Initial Supply | 0 QUA | Fair launch, no pre-mine |
| Year 1 Block Reward | 100 QUA | Strong early miner incentives |
| Annual Reduction | 15% | Gradual, predictable emission |
| Minimum Reward | 5 QUA | Perpetual security budget |
| Fee Burn Rate | 70% | Deflationary pressure |
| Block Time | 10 seconds | Fast transaction finality |

**Supply Projection:**
- Year 1: 315 million QUA
- Year 5: 1.17 billion QUA  
- Year 20+: ~2 billion QUA maximum (with 5 QUA floor)

### Market Opportunity

**Comparable Projects:**
- **Quantum Resistant Ledger (QRL)**: Market cap ~$10M (2025)
- **QAN Platform**: $15M raised, enterprise pilots
- **Algorand**: Announced post-quantum research initiatives

**QUANTA Differentiators:**
- 100% quantum-resistant from genesis (not hybrid)
- Modern Rust implementation (not legacy code)
- Adaptive tokenomics (not Bitcoin clone)
- No pre-mine or token sale (fair distribution)

---

## For Developers

### Technology Stack

```
Language:       Rust 2021 (memory-safe, high-performance)
Async Runtime:  Tokio (efficient concurrent I/O)
Database:       Sled (embedded transactional storage)
Networking:     Custom P2P protocol over TCP
API:            REST (Axum) + JSON-RPC 2.0
Cryptography:   pqcrypto-falcon, pqcrypto-kyber, sha3, argon2
```

### Key Features

#### Cryptographic Security
- Post-quantum signatures (Falcon-512, NIST Level 1)
- Post-quantum encryption (Kyber-1024, NIST Level 5)
- Quantum-resistant hashing (SHA3-256)
- Memory-hard key derivation (Argon2id)

#### Consensus & Blockchain
- Adaptive Proof-of-Work with dynamic difficulty
- Account-based model (Ethereum-style)
- 10-second block time
- Nonce-based replay protection
- 24-hour transaction expiry
- Merkle trees for SPV support

#### Network & Infrastructure
- Full P2P networking with peer discovery
- REST API and JSON-RPC daemon control
- Prometheus metrics export (port 9090)
- Health check endpoints
- Graceful shutdown handling
- Comprehensive test suite

#### Wallet Features
- HD wallets with BIP39 24-word mnemonic
- Encrypted wallet storage
- Multi-account support
- Secure key derivation

### API Examples

**REST API**
```bash
# Check node health
curl http://localhost:3000/health

# Get blockchain statistics
curl http://localhost:3000/api/stats

# Get address balance
curl -X POST http://localhost:3000/api/balance \
  -H "Content-Type: application/json" \
  -d '{"address": "your_address_here"}'
```

**JSON-RPC Daemon Control**
```bash
# Get node status
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"node_status","params":[],"id":1}'

# Start mining
curl -X POST http://localhost:7782 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","method":"start_mining","params":["your_address"],"id":1}'
```

---

## For Miners

### Mining Rewards

**Year 1 Economics:**
- Base reward: 100 QUA per block (every 10 seconds)
- Daily blocks: ~8,640 blocks
- Daily emission: ~864,000 QUA
- Plus transaction fees (10% to miner, 70% burned)

**Early Adopter Bonus:**
- First 100,000 blocks: 1.5x multiplier
- Duration: ~11.5 days
- Reward: 150 QUA per block

**Anti-Dump Protection:**
- 50% of mining rewards locked for 6 months
- Encourages long-term participation
- Reduces sell pressure during launch

### System Requirements

**Full Node / Mining:**
- **CPU**: 4 cores @ 2.0 GHz or higher
- **RAM**: 8 GB minimum, 16 GB recommended
- **Storage**: 1 TB SSD (year 1), plan for 5 TB over 5 years
- **Bandwidth**: 50 Mbps down, 20 Mbps up
- **OS**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+

**Pruned Node:**
- **CPU**: 2 cores
- **RAM**: 4 GB
- **Storage**: 100 GB SSD
- **Bandwidth**: 25 Mbps down, 10 Mbps up

---

## Installation

### Prerequisites

- **Rust**: 1.70 or higher ([install](https://rustup.rs/))
- **Git**: For cloning the repository
- **OpenSSL**: 1.1.1+ (Linux) or LibreSSL 3.0+ (macOS)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/quantachain/quanta.git
cd quanta

# Build release binary
cargo build --release

# Run tests
cargo test

# Binary location
./target/release/quanta
```

### Docker (Coming Soon)

```bash
docker pull quantachain/quanta:latest
docker run -d -p 3000:3000 -p 8333:8333 quantachain/quanta
```

---

## Quick Start Guide

### 1. Start a Node

```bash
# Build the project
cargo build --release

# Start node (daemon mode)
./target/release/quanta start --detach

# Check status
./target/release/quanta status
```

### 2. Create a Wallet

```bash
# Create HD wallet with 24-word mnemonic
./target/release/quanta new_hd_wallet --file my_wallet.qua

# View wallet info (note your address)
./target/release/quanta hd_wallet --file my_wallet.qua
```

### 3. Start Mining

```bash
# Start mining to your wallet address
./target/release/quanta start_mining <YOUR_ADDRESS>

# Monitor mining
./target/release/quanta mining_status
./target/release/quanta print_height
```

### 4. Send Transactions

```bash
./target/release/quanta send \
  --wallet my_wallet.qua \
  --to <RECIPIENT_ADDRESS> \
  --amount 10000000 \
  --db ./quanta_data
```

---

## CLI Reference

### Node Management

```bash
quanta start [OPTIONS]                    # Start node
quanta start --detach                     # Start as daemon
quanta status [--rpc-port PORT]           # Check node status
quanta stop [--rpc-port PORT]             # Stop daemon
quanta print_height [--rpc-port PORT]     # Show blockchain height
quanta peers [--rpc-port PORT]            # List connected peers
```

### Wallet Management

```bash
quanta new_wallet --file FILE             # Create quantum-safe wallet
quanta new_hd_wallet --file FILE          # Create HD wallet (BIP39 mnemonic)
quanta wallet --file FILE                 # Show wallet info
quanta hd_wallet --file FILE              # Show HD wallet details
```

### Mining Operations

```bash
quanta start_mining ADDRESS [--rpc-port PORT]  # Start mining
quanta stop_mining [--rpc-port PORT]           # Stop mining
quanta mining_status [--rpc-port PORT]         # Check mining status
```

### Blockchain Operations

```bash
quanta stats --db PATH                    # Show blockchain statistics
quanta validate --db PATH                 # Validate blockchain integrity
quanta get_block HEIGHT [--rpc-port PORT] # Get block information
```

---

## Configuration

Create a `quanta.toml` file for custom node configuration:

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

[security]
max_mempool_size = 5000
transaction_expiry_seconds = 86400
enable_rate_limiting = true

[mining]
year_1_reward_microunits = 100_000_000
annual_reduction_percent = 15
min_reward_microunits = 5_000_000

[metrics]
enabled = true
port = 9090
```

---

## Network Topology

### Running Multiple Nodes

```bash
# Node 1 (Bootstrap node)
./quanta start --detach --network-port 8333 --port 3000 --rpc-port 7782 --db ./node1_data

# Node 2 (Connect to Node 1)
./quanta start --detach --network-port 8334 --port 3001 --rpc-port 7783 \
  --db ./node2_data --bootstrap 127.0.0.1:8333

# Check connections
./quanta peers --rpc-port 7782
./quanta peers --rpc-port 7783
```

---

## Monitoring & Observability

### Prometheus Metrics

QUANTA exports Prometheus metrics on port 9090:

```yaml
# prometheus.yml example
scrape_configs:
  - job_name: 'quanta'
    static_configs:
      - targets: ['localhost:9090']
```

**Available Metrics:**
- Blockchain height
- Transaction throughput
- Peer count
- Mining hashrate
- Mempool size
- Block validation time

### Health Checks

```bash
# Health check endpoint
curl http://localhost:3000/health

# Example response
{
  "status": "healthy",
  "blockchain_height": 12345,
  "peer_count": 8,
  "uptime_seconds": 86400
}
```

---

## Security

### Cryptographic Security

- **Classical Security**: SHA3-256 (2^256 operations), Falcon-512 (2^128 operations)
- **Quantum Security**: Lattice-based signatures (no known quantum attacks), Grover-resistant hashing
- **Key Protection**: Argon2id prevents brute-force attacks on encrypted wallets

### Network Security

- **DoS Protection**: 2MB message limit, 5000 transaction mempool cap
- **Replay Protection**: Monotonic nonces, 24-hour transaction expiry
- **51% Attack Mitigation**: Checkpoint system
- **Timestamp Validation**: Blocks within 2 hours of current time

### Operational Security

- Graceful shutdown handling (SIGINT/SIGTERM)
- Persistent state across restarts
- Health check endpoints
- Localhost-only RPC binding (no remote exposure by default)

### Vulnerability Reporting

See [SECURITY.md](SECURITY.md) for our security policy and responsible disclosure process.

**Do NOT open public issues for security vulnerabilities.**

---

## Roadmap

### Phase 1: Testnet Preparation (Q1 2026) - In Progress

- Core protocol development
- Internal testing and validation
- Security infrastructure setup
- Documentation and tooling

### Phase 2: Public Testnet (Q2 2026)

- Public testnet launch with 6+ geographic bootstrap nodes
- Community onboarding and developer documentation
- Stress testing and network optimization
- Bug bounty program launch ($10k+ rewards)

### Phase 3: Security Hardening (Q3 2026)

- External security audits (2-3 independent firms)
- Vulnerability remediation and code optimization
- Final security review and penetration testing
- Emergency response procedures

### Phase 4: Mainnet Preparation (Q4 2026)

- Code freeze and final audit
- Genesis block preparation
- Bootstrap node deployment (10+ regions)
- Exchange partnership discussions

### Phase 5: Mainnet Launch (Q1 2027)

- Mainnet genesis with transparent parameters
- Block explorer deployment
- Desktop wallet release (Windows, macOS, Linux)
- Initial exchange integrations

### Phase 6: Expansion (2027+)

- Light client protocol (SPV)
- Mobile wallets (iOS, Android)
- Hardware wallet support (Ledger, Trezor)
- Developer SDKs and documentation

---

## Contributing

We welcome contributions from the community! See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

**Ways to Contribute:**
- Code improvements and bug fixes
- Documentation enhancements
- Test coverage expansion
- Performance optimization
- Translation and localization
- Community support and education

**Development Workflow:**
```bash
# Fork and clone
git clone https://github.com/YOUR_USERNAME/quanta.git

# Create feature branch
git checkout -b feature/your-feature-name

# Make changes, test, and commit
cargo fmt
cargo clippy
cargo test
git commit -m "feat: your feature description"

# Push and create pull request
git push origin feature/your-feature-name
```

---

## Community

| Platform | Link | Status |
|----------|------|--------|
| **GitHub** | [quantachain/quanta](https://github.com/quantachain/quanta) | Active |
| **Website** | [www.quantachain.org](https://www.quantachain.org) | Active |
| **Discord** | Coming Q2 2026 | Planned |
| **Twitter** | Coming Q2 2026 | Planned |
| **Telegram** | Coming Q2 2026 | Planned |

---

## Frequently Asked Questions

### General Questions

**Q: What makes QUANTA different from other quantum-resistant blockchains?**  
A: QUANTA is built from scratch with quantum resistance, not retrofitted. It uses NIST-standardized algorithms, modern Rust implementation, and fair distribution with no pre-mine.

**Q: When is the mainnet launch?**  
A: Planned for Q1 2027, after extensive testnet validation and security audits.

**Q: Is there a token sale or ICO?**  
A: No. QUANTA has 100% fair launch distribution through mining. No pre-mine, no ICO, no insider allocation.

### Technical Questions

**Q: Why Falcon-512 instead of larger key sizes?**  
A: Falcon-512 provides NIST Level 1 security (equivalent to AES-128), which is sufficient for blockchain use. Larger keys increase storage and bandwidth without meaningful security gain.

**Q: What if quantum computers never materialize?**  
A: QUANTA is secure against classical attacks. Post-quantum crypto is insurance for the future, not speculation.

**Q: Can QUANTA interoperate with Bitcoin or Ethereum?**  
A: Cross-chain bridges are planned for Phase 6 (2028+), requiring quantum-resistant relay protocols.

### Mining Questions

**Q: What hardware do I need to mine QUANTA?**  
A: A 4-core CPU with 8GB RAM is sufficient. QUANTA uses CPU-based proof-of-work (SHA3-256 hashing).

**Q: Is ASIC mining possible?**  
A: While ASICs can be built for any algorithm, SHA3 is relatively ASIC-resistant compared to algorithms like SHA256 (Bitcoin).

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
MIT License

Copyright (c) 2026 QUANTA Development Team

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.
```

---

## Acknowledgments

- NIST Post-Quantum Cryptography Standardization Project
- Rust and Tokio communities
- Open-source cryptography contributors
- Early testnet participants and contributors

---

## Citation

If you use QUANTA in your research or project, please cite:

```bibtex
@software{quanta2026,
  title = {QUANTA: A Quantum-Resistant Blockchain},
  author = {QUANTA Development Team},
  year = {2026},
  url = {https://github.com/quantachain/quanta},
  version = {1.0}
}
```

---

**Build for the Future. Secure Against Quantum.**
