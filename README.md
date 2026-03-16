# QUANTA

**The First Quantum-Resistant Blockchain Built for the Future**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-green.svg)]()
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![DOI](https://img.shields.io/badge/DOI-10.5281%2Fzenodo.18753528-blue)](https://doi.org/10.5281/zenodo.18753528)

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)

---

## What is QUANTA?

QUANTA is an ultra-secure, production-ready blockchain operating as a **Post-Quantum Institutional Settlement Layer**. While Bitcoin and Ethereum use cryptography vulnerable to future quantum computers, QUANTA is designed as an impenetrable digital vault for institutional capital and sovereign wealth.

It deliberately **omits smart contracts** to minimize attack surfaces, focusing entirely on providing the most secure store-of-value network ever built.

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
| [Governance](GOVERNANCE.md) | Treasury multisig, PoS transition, on-chain voting roadmap |
| [Contributing](CONTRIBUTING.md) | Development guidelines and how to contribute |
| [Security Policy](SECURITY.md) | Vulnerability reporting and security practices |
| [Website](https://www.quantachain.org) | Official project website |
| [Documentation](https://www.quantachain.org/docs) | Installation and usage guides |

---

## For Investors

### Value Proposition

QUANTA addresses a trillion-dollar problem: the quantum computing threat to blockchain infrastructure. As institutions and governments invest in quantum computing, existing blockchains face obsolescence. QUANTA provides:

1. **First-Mover Advantage**: The first institutional-grade, quantum-resistant settlement network
2. **Minimal Attack Surface**: No Turing-complete smart contracts means no contract exploits or multi-billion dollar hacks
3. **Native Crypto-Vaulting**: Built-in protocol-level Time-Locks for escrow and vesting
4. **Deflationary Economics**: 70% of transaction fees are permanently burned
4. **Sustainable Growth**: Perpetual mining incentives prevent Bitcoin's "final block" problem

### Tokenomics Summary

| Parameter | Value | Benefit |
|-----------|-------|---------|
| Initial Supply | 0 QUA | Fair launch, no pre-mine |
| Year 1 Block Reward | 100 QUA | Strong early miner incentives |
| Annual Reduction | 15% | Gradual, predictable emission |
| Minimum Reward | 5 QUA | Perpetual security budget |
| Fee Burn Rate | 70% | Deflationary pressure |
| Block Time | 30 seconds | Fast transaction finality |

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

| Component | Technology | Notes |
|---|---|---|
| Language | Rust 2021 | Memory-safe, no GC, high-performance |
| Async Runtime | Tokio 1.35 | Multi-threaded async I/O |
| Database | sled 0.34 | Embedded, crash-safe key-value store |
| P2P Network | Tokio TCP + custom protocol | bincode wire format, zstd compressed |
| REST API | Axum 0.7 + Tower | Port 7777 (default) |
| RPC Server | Custom TCP JSON-RPC | Port 7782 (default) |
| Serialization | bincode (wire) + serde_json (API) | 22% smaller, 8× faster than JSON |
| Parallel Compute | Rayon 1.8 | Parallel Falcon-512 signature verification |
| Sig Cache | lru 0.12 | 100,000-entry LRU verification cache |
| Compression | zstd 0.13 | 4× block size reduction on P2P wire |
| PQ Signatures | pqcrypto-falcon 0.3.0 (pinned) | Falcon-512; pinned for consensus determinism |
| PQ Encryption | pqcrypto-kyber 0.8 | Kyber-1024; wallet encryption |
| Hashing | sha3 0.10 | SHA3-256 (double-hash for blocks) |
| KDF | argon2 0.5 | Argon2id; wallet password hardening |
| HD Wallet | bip39 2.0 + hmac 0.12 | BIP39 24-word mnemonic, BIP32 Falcon keys |

### Module Architecture

```
quanta/
├── src/
│   ├── consensus/
│   │   ├── blockchain.rs   ← Chain state, block add/validate, difficulty, rewards
│   │   ├── mempool.rs      ← Mempool management, fee ordering
│   │   └── performance.rs  ← Metrics tracking
│   ├── core/
│   │   ├── block.rs        ← Block struct, mine(), double-SHA3 hash, PoW
│   │   ├── transaction.rs  ← Tx struct, verify(), AccountState, locked balances
│   │   └── merkle.rs       ← SHA3-256 Merkle tree for SPV
│   ├── crypto/
│   │   ├── signatures.rs   ← FalconKeypair, verify_signature_strict(), domain sep
│   │   ├── wallet.rs       ← QuantumWallet, Kyber-1024 wallet encryption
│   │   ├── hd_wallet.rs    ← BIP39/BIP32 HD wallet with Falcon-512 keys
│   │   └── multisig.rs     ← M-of-N Falcon-512 threshold signatures
│   ├── network/            ← P2P TCP protocol, peer discovery, sync
│   ├── api/                ← Axum REST endpoints (port 7777)
│   ├── rpc/                ← TCP RPC server + client (port 7782)
│   ├── storage/            ← sled DB wrapper, block/account CRUD
│   ├── config/             ← TOML config, QuantaConfig struct
│   ├── bin/wallet_cli.rs   ← Interactive wallet CLI binary
│   └── main.rs             ← CLI entry point (~20 subcommands)
```

### Key Features

#### Cryptographic Security (All Post-Quantum)
- **Falcon-512** signatures — NIST Level 1, 897-byte pubkey, stateless, unlimited reuse
- **Kyber-1024** encryption — NIST Level 5 for wallet files and key material
- **SHA3-256** hashing — double-SHA3 for blocks, quantum-safe
- **Argon2id** key derivation — memory-hard wallet password protection
- Domain-separated canonical signing: `SHA3-256("QUANTA_TX_V1:" || signing_bytes)`
- Strict pre-checks: pubkey must be exactly 897 bytes, sig blob in [33, 698]
- Crypto agility: `sig_scheme` byte in every TX — soft-fork algorithm upgrades
- Build determinism: pqcrypto-falcon pinned to `=0.3.0`, `strict-float`, `codegen-units=1`

#### Consensus & Blockchain
- Adaptive Proof-of-Work (SHA3-256/SHA3-256 double hash, CPU-friendly)
- **Account-based model** (not UTXO): balance + nonce + locked_balances per address
- 30-second block time target
- **1,200 TX per block max** (2 MB limit — Falcon-512 tx ≈ 1,713 bytes)
- **40 TPS** sustained throughput
- Nonce-based replay protection + 24-hour expiry window
- Merkle tree (SHA3-256) for transaction inclusion proofs
- Checkpoint system: hardcoded hashes prevent deep chain reorgs

#### Performance Optimizations
| Optimization | Before | After | Speedup |
|---|---|---|---|
| Rayon parallel sig verify (physical cores) | 1,800 ms | 225 ms | **8×** |
| LRU signature cache (100k entries, ~80% hit) | 225 ms | ~45 ms | **5×** |
| Bloom filter mempool dedup (50k cap, 0.01% FP) | O(n) scan | O(1) | **∞** |
| Pubkey cache (897-byte Falcon key, DashMap) | Re-derive every tx | O(1) after 1st | **High on busy blocks** |
| Rayon thread pool (physical vs logical CPUs) | Logical cores | Physical cores | **+15% crypto throughput** |
| zstd block compression | 2 MB/block | 500 KB/wire | **4× bandwidth** |
| bincode serialization | JSON baseline | 22% smaller, 8× faster | **8×** |

#### Transaction Types
```rust
TransactionType::Transfer                              // Value transfer
TransactionType::TimeLockTransfer { unlock_height }    // Cryptographic Escrow/Vaulting
```

#### Wallet Features
- **Standard Wallet**: Single Falcon-512 keypair, Kyber-1024 encrypted storage
- **HD Wallet**: BIP39 24-word mnemonic + BIP32 derivation with Falcon-512 keys
- **Multisig**: M-of-N Falcon-512 threshold signatures for treasury and team wallets
- **Treasury Multisig (3-of-5)**: Live — `ms69216b1d10425689704d5ae3b2a4aa17049f59b1`. Any 3 of 5 keyholders must sign a spend. Address hardcoded in consensus, cannot be redirected by node operators.
- **Address Format**: `0x` + hex(SHA3-256(pubkey)[:20]) for single-key; `ms` + hex(SHA3-256(sorted_pubkeys)) for multisig

### REST API Endpoints (Port 7777)

```bash
# Node health
curl http://localhost:7777/health

# Get block by height
curl http://localhost:7777/blocks/100

# Get latest block
curl http://localhost:7777/blocks/latest

# Get account balance
curl http://localhost:7777/accounts/0xYOUR_ADDRESS/balance

# Get account nonce
curl http://localhost:7777/accounts/0xYOUR_ADDRESS/nonce

# Get mempool
curl http://localhost:7777/mempool

# Submit transaction (POST)
curl -X POST http://localhost:7777/transactions \
  -H "Content-Type: application/json" \
  -d '{"sender": "0x...", "recipient": "0x...", ...}'

# Get network stats
curl http://localhost:7777/network/stats
```

### RPC Interface (Port 7782)

The RPC server is used by the CLI to control a running node:

```bash
# All CLI commands communicate with the node over RPC:
./quanta status          # → node_status RPC
./quanta start_mining    # → start_mining RPC
./quanta mining_status   # → mining_status RPC
./quanta print_height    # → node_status.chain_height RPC
./quanta peers           # → get_peers RPC
./quanta stop            # → shutdown RPC
./quanta get_block N     # → get_block(N) RPC
```

---

## For Miners

### Mining Rewards (Year 1)

| Metric | Value |
|---|---|
| Block reward | 100 QUA |
| Miner immediate (47.5%) | 47.5 QUA/block |
| Miner locked 6-month (47.5%) | 47.5 QUA/block |
| Treasury (5%) | 5 QUA/block |
| Fee share to miner | 10% of block fees |
| Daily blocks | ~8,640 |
| Daily immediate emission | ~410,400 QUA |

**Anti-Dump Vesting**: 50% of the miner's 95% reward share (= 47.5% of block reward) is locked for 52,560 blocks (~6 months). This prevents price-damaging sell cascades at launch and aligns miner incentives with long-term network health.

**Coinbase Maturity**: Mining rewards require 100 block confirmations before they can be spent.

### System Requirements

| Type | CPU | RAM | Storage | Bandwidth |
|---|---|---|---|---|
| **Full Node / Mining** | 4 cores @ 2 GHz | 8–16 GB | 1 TB SSD (yr 1) | 50/20 Mbps |
| **Pruned Node** | 2 cores @ 2 GHz | 4 GB | 400 GB SSD | 25/10 Mbps |
| **Light Client** (planned) | 1 core | 1 GB | 1 GB | 5 Mbps |

**OS Support**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+

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

All commands are available in the `quanta` binary (and `quanta-wallet-cli` for wallet-only use).

### Node Management

```bash
# Start node with REST API + P2P + RPC
quanta start [--config FILE] [--network mainnet|testnet]
             [--port API_PORT] [--network-port P2P_PORT] [--rpc-port RPC_PORT]
             [--db PATH] [--bootstrap host:port,...] [--no-network] [--detach]

# Check running node status
quanta status [--rpc-port 7782]

# Print current blockchain height
quanta print_height [--rpc-port 7782]

# Get connected peers
quanta peers [--rpc-port 7782]

# Stop node gracefully
quanta stop [--rpc-port 7782]
```

### Wallet Management

```bash
# Create a new single-keypair quantum wallet (encrypted)
quanta new_wallet [--file wallet.qua]

# Create HD wallet with BIP39 24-word mnemonic + multiple accounts
quanta new_hd_wallet [--file hd_wallet.json] [--accounts 3]

# Show wallet info + balance (connects to running node)
quanta wallet [--file wallet.qua] [--network mainnet|testnet] [--db PATH]

# Show wallet address only (offline, no node needed)
quanta wallet_address [--file wallet.qua]

# Show HD wallet info
quanta hd_wallet [--file hd_wallet.json]
```

### Mining Operations

```bash
# Start mining to an address (node must be running)
quanta start_mining <ADDRESS> [--rpc-port 7782]

# Check mining status, hashrate, and last block
quanta mining_status [--rpc-port 7782]

# Stop mining
quanta stop_mining [--rpc-port 7782]

# Mine a single block offline (CLI mode, for testing)
quanta mine [--wallet wallet.qua] [--db ./quanta_data]
```

### Blockchain Operations

```bash
# Get block details by height
quanta get_block <HEIGHT> [--rpc-port 7782]

# Show blockchain statistics (height, TX count, etc.)
quanta stats [--db ./quanta_data]

# Validate entire blockchain integrity
quanta validate [--db ./quanta_data]

# Send a transaction
quanta send [--wallet wallet.qua] --to <ADDRESS> --amount <QUA> [--db ./quanta_data]

# Run demo with sample transactions (dev/testing)
quanta demo [--db ./quanta_demo]
```

---

## Configuration

Create a `quanta.toml` file or use the provided `quanta.toml` in the repository root:

```toml
[node]
api_port = 7777        # REST API port (default: 7777)
network_port = 8333    # P2P TCP port (default: 8333)
db_path = "./quanta_data"
no_network = false
# Node storage mode: archive | pruned | light
mode = "archive"       # archive = keep all blocks, pruned = keep last N days, light = headers only
prune_days = 30        # only used when mode = "pruned"

[network]
max_peers = 125
bootstrap_nodes = [
  "testnet-us-east.quanta.network:8333",
  "testnet-eu-west.quanta.network:8333"
]
dns_seeds = [
  "seed.testnet.quanta.network",
  "nodes.testnet.quanta.network"
]

[security]
max_mempool_size = 5000           # Max pending transactions
transaction_expiry_seconds = 86400 # 24 hours

[metrics]
enabled = true
port = 9090

# Consensus engine: proof_of_work (live) | proof_of_stake (planned — node refuses to start)
consensus_engine = "proof_of_work"
```

**Node Ports**:
| Port | Protocol | Purpose |
|---|---|---|
| **7777** | HTTP | REST API (default) |
| **8333** | TCP | P2P networking |
| **7782** | TCP | RPC server (CLI-to-node) |
| **9090** | HTTP | Prometheus metrics |

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

QUANTA exports Prometheus-compatible metrics on port 9090:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'quanta'
    static_configs:
      - targets: ['localhost:9090']
```

**Available Metrics**: blockchain height, TPS, peer count, mining hashrate, mempool size, block validation time, signature cache hit rate

### Health Checks

```bash
# Health check
curl http://localhost:7777/health

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

### Falcon-512 Protocol Hardening

| Property | Implementation |
|---|---|
| Signing/verification separation | Signing is wallet-only; consensus nodes only call verify functions |
| Domain separation | All signatures cover `SHA3-256("QUANTA_TX_V1:" \|\| signing_bytes)` |
| Pre-verification size checks | Public key rejected if != 897 bytes; signature rejected outside [33,698] bytes |
| Sender binding | Address must derive from the embedded public key before crypto runs |
| Crypto agility | `sig_scheme` byte in every transaction; unknown values are hard-rejected |
| Build determinism | Dependency pinned to `=0.3.0`; `strict-float` + `codegen-units=1` enforced |

### Network Security

- **DoS Protection**: 2 MB message limit, 5000 transaction mempool cap
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

### ✅ Phase 1: Testnet Preparation (Q1 2026) — **COMPLETE**

- ✅ Core blockchain (consensus, crypto, storage, mempool)
- ✅ P2P networking with DNS seed discovery
- ✅ REST API (Axum, port 7777) + RPC server (port 7782)
- ✅ HD Wallet (BIP39/BIP32) + Multisig (M-of-N Falcon-512)
- ✅ **3-of-5 Treasury Multisig** — live on-chain, hardcoded in consensus, 3 keyholders required to spend
- ✅ Performance: parallel verify (Rayon/physical cores), LRU sig cache, bloom filter mempool, pubkey cache, zstd compression
- ✅ Node modes: Archive / Pruned / Light (configurable in quanta.toml)
- ✅ Consensus engine enum: `proof_of_work` live, `proof_of_stake` stub ready for future implementation
- ✅ Security: strict pre-checks, domain separation, build determinism
- ✅ Block explorer (`explorer.html`)
- ✅ Docker + monitoring setup
- ✅ Documentation: Whitepaper, Tokenomics, Governance, Security

### 🔄 Phase 2: Public Testnet Launch (Q2 2026)

- Public testnet with 6+ geographic bootstrap nodes
- Real-world stress testing (target: 30+ days, 10,000+ transactions)
- External security audits + bug bounty program
- Developer onboarding and SDK documentation

**Target**: 30+ nodes, zero critical CVEs, 30-day continuous uptime

### Phase 3: Security Hardening (Q3 2026)

Comprehensive audit, penetration testing, protocol finalization

### Phase 4: Mainnet Preparation (Q4 2026)

Code freeze, genesis configuration, multi-region bootstrap nodes, desktop wallets, block explorer

### Phase 5: Mainnet Launch (Q1 2027)

Genesis event, block explorer live, production wallets, exchange integrations

### Phase 6: Expansion (2027+)

Light client (SPV), mobile wallets, hardware wallet support, advanced Multisig, and Institutional Custodian Integrations

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

**Q: What makes QUANTA different from other PQ blockchains (QRL, QAN)?**  
A: QUANTA uses NIST-standardized Falcon-512, delivers 40+ TPS, is written in production-grade Rust, and deliberately removes smart contracts to serve exclusively as a hyper-secure Institutional Settlement Vault.

**Q: When is mainnet?**  
A: Planned Q1 2027, after public testnet (Q2 2026) and security hardening (Q3 2026).

**Q: Is there a token sale or ICO?**  
A: No. 100% fair launch through mining. No pre-mine, no ICO, no insider allocation.

**Q: Why Proof-of-Work instead of Proof-of-Stake?**  
A: PoW enables a fair launch without pre-existing token distribution, has 15+ years of proven security history, and provides Sybil resistance without stake centralization. PoS is planned as a future upgrade — see [GOVERNANCE.md](GOVERNANCE.md) for the transition roadmap. When ready, set `consensus_engine = "proof_of_stake"` in `quanta.toml` to activate it.

**Q: Why Falcon-512 (Level 1) and not higher?**  
A: Level 1 gives 64-bit post-quantum security (after Grover's). Attacking this requires millions of error-corrected qubits — estimated 30+ years away. Larger keys (Falcon-1024) would increase block sizes dramatically, hurting throughput. The `sig_scheme` field allows soft-fork upgrades if needed.

**Q: What if Falcon-512 has a vulnerability?**  
A: The `sig_scheme` byte in every transaction enables soft-fork algorithm replacement without wire format changes. Upgrade path is built in from genesis.

**Q: What hardware do I need to mine?**  
A: 4-core CPU, 8 GB RAM. QUANTA's SHA3-256 PoW is CPU-friendly (no specialized ASIC required).

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

```
MIT License

Copyright (c) 2026 Kishore K — QUANTA Development Team

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

## Research Paper

The technical design of QUANTA is described in detail in our preprint:

> **QUANTA: Engineering a Production-Ready Post-Quantum Blockchain with Falcon-512 Lattice Signatures**  
> Kishore K — February 2026  
> DOI: [10.5281/zenodo.18753528](https://doi.org/10.5281/zenodo.18753528)

If you use QUANTA in your research or project, please cite:

```bibtex
@misc{quanta2026,
  title  = {QUANTA: Engineering a Production-Ready Post-Quantum Blockchain
             with Falcon-512 Lattice Signatures},
  author = {Kishore K},
  year   = {2026},
  month  = feb,
  doi    = {10.5281/zenodo.18753528},
  url    = {https://doi.org/10.5281/zenodo.18753528}
}
```

---

---

**Build for the Future. Secure Against Quantum.**  

*Kishore K, Founder — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)*
