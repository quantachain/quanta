# QUANTA

**The First Quantum-Resistant Blockchain Built for the Future**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-green.svg)]()
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
[![DOI](https://img.shields.io/badge/DOI-10.5281%2Fzenodo.18753528-blue)](https://doi.org/10.5281/zenodo.18753528)
[![Docker](https://img.shields.io/badge/docker-xd637%2Fquanta--node-blue)](https://hub.docker.com/r/xd637/quanta-node)

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)

---

## What is QUANTA?

QUANTA is a production-ready blockchain operating as a **Post-Quantum Institutional Settlement Layer**. While Bitcoin and Ethereum use cryptography vulnerable to future quantum computers, QUANTA is designed as an impenetrable digital vault for institutional capital and sovereign wealth.

It deliberately **omits smart contracts** to minimize attack surfaces, focusing entirely on providing the most secure store-of-value network ever built.

**Built with:**
- **Falcon-512** — NIST-standardized post-quantum signatures
- **Kyber-1024** — post-quantum wallet encryption
- **SHA3-256** — quantum-resistant double-hash Proof-of-Work
- **Rust** — memory-safe, high-performance implementation

---

## Quick Links

| Resource | Description |
|----------|-------------|
| [Documentation](https://quantachain.gitbook.io/quantachain-docs) | Node setup, mining, API reference, wallets |
| [Whitepaper](WHITEPAPER.md) | Complete technical specification and architecture |
| [Tokenomics](TOKENOMICS.md) | Economic model, supply schedule, and incentive design |
| [Governance](GOVERNANCE.md) | Treasury multisig, PoS transition, on-chain voting roadmap |
| [Contributing](CONTRIBUTING.md) | Development guidelines and how to contribute |
| [Security](SECURITY.md) | Vulnerability reporting and security practices |
| [Docker Hub](https://hub.docker.com/r/xd637/quanta-node) | Official node image |
| [Website](https://www.quantachain.org) | Official project website |

---

## Quick Start — Docker (Recommended)

Docker is the recommended way to run a Quanta node. No Rust toolchain required.

```bash
docker pull xd637/quanta-node:latest

docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

**Check it's running:**
```bash
docker logs quanta-node --tail 20 -f
curl http://localhost:3000/health
```

### Or with Docker Compose

```bash
# Download and start
docker compose -f docker-compose.single.yml up -d

# Check status
docker exec -it quanta-node quanta status --rpc-port 7782
```

For VPS deployment with NGINX and HTTPS, see the [full deployment guide](https://quantachain.gitbook.io/quantachain-docs/docker-deployment).

---

## Node Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| **3000** | HTTP | REST API |
| **8333** | TCP | P2P networking |
| **7782** | TCP | RPC server (CLI to node) |
| **9090** | HTTP | Prometheus metrics |

---

## Wallet & Validator Setup (via Docker)

```bash
# Create HD wallet (24-word recovery phrase — recommended)
docker exec -it quanta-node quanta new_hd_wallet --file hd_wallet.json

# Show wallet address
docker exec -it quanta-node quanta wallet_address --file hd_wallet.json

# Stake QUA to become a BFT validator
docker exec -it quanta-node quanta stake --wallet hd_wallet.json --amount 10000000

# Check validator status
docker exec -it quanta-node quanta status --rpc-port 7782
```

---

## Build from Source

If you prefer to build the binary directly:

**Prerequisites**: Rust 1.70+, Git, OpenSSL 1.1.1+

```bash
git clone https://github.com/quantachain/quanta.git
cd quanta

cargo build --release

# Run node
./target/release/quanta start -c quanta.toml
```

---

## CLI Reference

All commands are available on the `quanta` binary (inside Docker via `docker exec -it quanta-node quanta ...`).

### Node Management

```bash
quanta start [--config FILE] [--network mainnet|testnet]
             [--port API_PORT] [--network-port P2P_PORT] [--rpc-port RPC_PORT]
             [--db PATH] [--bootstrap host:port,...] [--no-network] [--detach]

quanta status            [--rpc-port 7782]   # Running node status
quanta print_height      [--rpc-port 7782]   # Current block height
quanta peers             [--rpc-port 7782]   # Connected peers
quanta stop              [--rpc-port 7782]   # Graceful shutdown
```

### Wallet Management

```bash
quanta new_wallet        [--file wallet.qua]                     # Single keypair wallet
quanta new_hd_wallet     [--file hd_wallet.json] [--accounts 3]  # HD wallet (24-word mnemonic)
quanta wallet            [--file wallet.qua]                     # Show balance
quanta wallet_address    [--file wallet.qua]                     # Show address (offline)
quanta hd_wallet         [--file hd_wallet.json]                 # Show HD wallet info
```

### Validator Management

```bash
quanta stake    --wallet W --amount QUA   # Register as BFT validator
quanta unstake  --wallet W                # Begin unbonding (~4.2 day wait)
```

### Blockchain Operations

```bash
quanta get_block <HEIGHT>    [--rpc-port 7782]       # Block by height
quanta stats                 [--db ./quanta_data]    # Chain statistics
quanta validate              [--db ./quanta_data]    # Validate chain integrity
quanta send --wallet W --to ADDR --amount QUA        # Send transaction
```

---

## REST API (Port 3000)

```bash
curl http://localhost:3000/health
curl http://localhost:3000/blocks/latest
curl http://localhost:3000/blocks/100
curl http://localhost:3000/accounts/0xYOUR_ADDRESS/balance
curl http://localhost:3000/accounts/0xYOUR_ADDRESS/nonce
curl http://localhost:3000/mempool
curl http://localhost:3000/network/stats

# Submit transaction
curl -X POST http://localhost:3000/transactions \
  -H "Content-Type: application/json" \
  -d '{"sender":"0x...","recipient":"0x...","amount":5000000,...}'
```

---

## Technology Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 2021 |
| Async | Tokio 1.35 |
| Database | sled 0.34 (embedded, crash-safe) |
| P2P | Tokio TCP + custom protocol (bincode, zstd) |
| REST API | Axum 0.7 (port 3000) |
| RPC | Custom TCP JSON-RPC (port 7782) |
| PQ Signatures | pqcrypto-falcon 0.3.0 (pinned) — Falcon-512 |
| PQ Encryption | pqcrypto-kyber 0.8 — Kyber-1024 |
| Hashing | sha3 0.10 — SHA3-256 double-hash |
| KDF | argon2 0.5 — Argon2id wallet protection |
| HD Wallet | bip39 2.0 + hmac 0.12 — BIP39/BIP32 Falcon keys |

---

## Key Features

- **Falcon-512 signatures** — NIST Level 1, 897-byte pubkey, stateless, unlimited reuse
- **120+ TPS** — parallel Rayon verification, LRU sig cache (100k entries), bloom filter mempool
- **Account model** — balance + nonce + locked_balances; not UTXO
- **6-second BFT slot time** — `SLOT_SECONDS = 6`, 1,200 TX/block max, 2 MB limit
- **DPoS + BFT finality** — `>2/3` committee signatures required; instant finality
- **7-validator testnet** — permissioned bootstrap; 21 validators at mainnet genesis
- **Checkpoint system** — hardcoded hashes prevent deep chain reorgs
- **3-of-5 Treasury Multisig** — `ms69216b1d10425689704d5ae3b2a4aa17049f59b1`, consensus-enforced
- **AI agent native** — `payload: Vec<u8>` in every tx; signed by Falcon-512; carries agent instructions or stablecoin intents

### Transaction Types

```
Transfer                              — Standard value transfer
TimeLockTransfer { unlock_height }    — Cryptographic escrow / vaulting
MultiSigTransfer { signers_required } — Institutional M-of-N multisig
Stake { validator_pubkey }            — Register as BFT validator
Unstake                               — Deregister validator, begin unbonding
ContractDeploy { template_id }        — Deploy native contract (Escrow, etc.)
ContractCall { method, args }         — Invoke deployed contract
```

### Validator Economics (Year 1 — 7 testnet validators)

| Parameter | Value |
|-----------|-------|
| Block reward | **50 QUA** |
| QEF allocation (8%) | 4 QUA/block |
| Proposer reward (92%) | 46 QUA/block |
| Fee split | 50% burn / 35% proposer / 15% QEF |
| Daily blocks | ~14,400 (at 6s slots) |
| Per-validator annual income | ~34,537,000 QUA (~$34,537 at $0.001/QUA) |

---

## Roadmap

### ✅ Phase 1: Testnet Preparation (Q1 2026) — Complete

- Core blockchain, P2P, REST API, RPC server
- HD Wallet (BIP39/BIP32), Multisig (M-of-N Falcon-512)
- 3-of-5 Treasury Multisig — live on-chain
- Parallel Falcon-512 verification, LRU sig cache, bloom filter mempool
- Headers-first sync, cumulative work-based peer selection
- Cross-chain replay protection (`network_id`)
- Checkpoint system with hardcoded hashes
- Docker image + monitoring setup

### 🔄 Phase 2: Public Testnet (Q2 2026) — **ACTIVE** (chain at 91,000+ blocks)

- ✅ Public testnet live — chain at 91,000+ blocks, 2 active miners, 276 kH/s network hashrate
- ✅ Block explorer live at [scan.quantachain.org](https://scan.quantachain.org)
- ✅ Docker image: `xd637/quanta-node:latest`
- 🔄 External security audits + bug bounty program
- 🔄 Developer SDK and tooling

### Phase 3: Security Hardening (Q3 2026)

Comprehensive audit, penetration testing, protocol finalization

### Phase 4: Mainnet Preparation (Q4 2026)

Code freeze, genesis configuration, multi-region bootstrap nodes, block explorer, desktop wallets

### Phase 5: Mainnet Launch (Q1 2027)

Genesis event, block explorer, production wallets, exchange integrations

### Phase 6: Expansion (2027+)

Light client (SPV), hardware wallet support, AI agent SDK v2, DEX bridge integrations

---

## Developer Ecosystem

| Package | Description |
|---------|-------------|
| [`quanta-sdk`](https://github.com/quantachain/quanta-sdk) | JS/TS SDK — build wallets, explorers, and integrations |
| [`quanta-wasm`](https://github.com/quantachain/quanta-wasm) | Rust→WASM Falcon-512 crypto for browser and Node.js |
| [`quantascan`](https://github.com/quantachain/quantascan) | Block explorer |
| [`quanta-wallet`](https://github.com/quantachain/quanta-wallet) | Chrome extension wallet |

---

## Frequently Asked Questions

**Q: What makes QUANTA different from other PQC chains (QRL, QAN)?**
A: QUANTA uses NIST-standardized Falcon-512, delivers 120+ TPS with 6-second BFT finality, and is purpose-built as an **AI agent execution layer** — not a general-purpose L1. QUA is gas for AI agents, not a speculative store-of-value.

**Q: When is mainnet?**
A: Planned Q1 2027, after public testnet (Q2 2026) and security hardening (Q3 2026).

**Q: Is there a token sale or ICO?**
A: No ICO. QUA is distributed via block rewards (BFT validators) and a genesis allocation to founding validators. No VC dump, no unlock cliff.

**Q: Why DPoS + BFT instead of Proof-of-Work?**
A: AI agents cannot wait for 6-confirmation PoW finality. BFT provides **instant finality** in 6 seconds with `>2/3` validator quorum. DPoS enables energy-efficient staking with real validator accountability (unbonding period).

**Q: What hardware do I need to validate?**
A: 4-core CPU, 4 GB RAM. No ASICs, no GPUs — Falcon-512 BFT is CPU-friendly. VPS cost ~$20–40/month.

**Q: Why Falcon-512 (Level 1) and not Falcon-1024?**
A: Level 1 gives 64-bit post-quantum security. Falcon-1024 would roughly double transaction sizes, cutting throughput by half. The `sig_scheme` field enables a soft-fork upgrade if cryptanalysis improves.

**Q: Can AI agents pay in USDC/USDT?**
A: Yes — the `payload: Vec<u8>` field in every transaction is signed by Falcon-512 and can carry stablecoin settlement intents. QUA pays for execution (gas); USDC/USDT is the settlement currency embedded in the payload. No competition.

---

## Contributing

We welcome contributions. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

```bash
git clone https://github.com/YOUR_USERNAME/quanta.git
git checkout -b feature/your-feature-name
cargo fmt && cargo clippy && cargo test
git commit -m "feat: your feature description"
git push origin feature/your-feature-name
```

---

## Research Paper

> **QUANTA: Engineering a Production-Ready Post-Quantum Blockchain with Falcon-512 Lattice Signatures**
> Kishore K — February 2026
> DOI: [10.5281/zenodo.18753528](https://doi.org/10.5281/zenodo.18753528)

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

## License

MIT License — see [LICENSE](LICENSE) for details.

Copyright (c) 2026 Kishore K — QUANTA Development Team

---

**Build for the Future. Secure Against Quantum.**

*Kishore K, Founder — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)*
