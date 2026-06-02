# QUANTA

**The First Post-Quantum Institutional Settlement and AI Execution Layer**

[![License: AGPLv3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![License: Commercial](https://img.shields.io/badge/License-Commercial-red.svg)](COMMERCIAL_LICENSE.md)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)
[![Build Status](https://img.shields.io/badge/build-passing-green.svg)]()

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)

---

## What is QUANTA?

QUANTA is a production-ready blockchain built from the ground up to be quantum-resistant and optimized as a settlement layer for autonomous AI agents, Machine-to-Machine (M2M) micro-transactions, and Decentralized Physical Infrastructure Networks (DePIN). While traditional blockchains rely on cryptography that is vulnerable to future quantum computers, QUANTA secures its network using NIST-standardized Post-Quantum Cryptography (PQC).

QUANTA V2 completely abandons Proof-of-Work in favor of Delegated Proof-of-Stake (DPoS) combined with Asynchronous Byzantine Fault Tolerance (AlephBFT). This provides the deterministic 6-second finality required for rapid AI and M2M agent settlement.

**Built with:**
- **Falcon-512** — NIST-standardized post-quantum transaction signatures
- **Kyber-1024** — Post-quantum wallet encryption
- **AlephBFT** — Asynchronous Byzantine Fault Tolerance consensus
- **SHA3-256** — Quantum-resistant hashing
- **Rust** — Memory-safe, high-performance implementation

---

## Key Features

- **PQC + AI Execution Layer**: Every transaction includes a `payload: Vec<u8>` field signed by Falcon-512, which can carry agent instructions or stablecoin settlement intents. The network acts as the execution and gas layer for AI agents.
- **DPoS + AlephBFT Finality**: Instant mathematical finality in 6 seconds. Consensus requires greater than 2/3 committee signatures. The network uses a delegated staking model with an unbonding period for strict validator accountability.
- **Native Contract Templates**: To minimize the attack surface of Turing-complete smart contracts, QUANTA provides built-in, highly audited native templates:
  - **Escrow** (`TEMPLATE_ESCROW`): Cryptographic escrow using pre-image hashes.
  - **Agent Job** (`TEMPLATE_AGENT_JOB`): Trustless execution contracts between employers and AI workers.
- **High Throughput**: Parallel Rayon verification, LRU signature caching, and O(1) bloom filter mempool duplicate detection enable 120+ TPS.
- **Institutional Treasury**: Native 3-of-N Multisig enforced directly at the consensus layer.

---

## Transaction Types

The protocol supports a distinct set of transaction types tailored for financial and AI operations:

- `Transfer`: Standard value transfer
- `TimeLockTransfer`: Cryptographic vaulting locked until a specific block height
- `MultiSigTransfer`: Institutional M-of-N multisig transfer
- `Stake`: Register as a DPoS validator
- `Unstake`: Deregister validator and begin the unbonding period
- `ContractDeploy`: Instantiate a native contract template
- `ContractCall`: Invoke a method on a deployed native contract

---

## Quick Start — Docker (Recommended)

Docker is the recommended way to run a Quanta node. No Rust toolchain is required.

### 1. Wipe Old Data
If you ran a V1 node, you must wipe the old Proof-of-Work data:
```bash
rm -rf ./quanta_data
```

### 2. Run the Node
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

### 3. Check Status
```bash
docker logs quanta-node --tail 20 -f
curl http://localhost:3000/blocks/latest
```

---

## Node Operator Guide (Validators)

To participate in consensus on the permissioned testnet, you must inject your wallet into the node.

1. **Create a Raw Wallet**
```bash
docker exec -it quanta-node quanta-wallet new-raw --file validator.qua
```
2. **Start the Node as Validator**
Pass your wallet and password into the container to enable block production:
```bash
docker run -d \
  --name quanta-validator \
  --restart always \
  -e QUANTA_WALLET_PASSWORD="your_password" \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v /absolute/path/to/validator.qua:/home/quanta/validator.qua \
  xd637/quanta-node:latest \
  quanta start --validator-wallet /home/quanta/validator.qua
```

---

## Build from Source

**Prerequisites**: Rust 1.70+, Git, OpenSSL 1.1.1+

```bash
git clone https://github.com/quantachain/quanta.git
cd quanta
cargo build --release

./target/release/quanta start -c quanta.toml
```

---

## Wallet Operations

The `quanta-wallet` supports Headless AI Mode via environment variables.

```bash
export QUANTA_WALLET_PASSWORD="your_password"

# Create a new HD Wallet
quanta-wallet new --file wallet.json

# Check Balance
quanta-wallet info --file wallet.json

# Deploy an Agent Job Contract
quanta-wallet deploy-escrow --beneficiary <WORKER_ADDR> --secret-hash <HASH> --amount 5.0
```

---

## Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| **3000** | HTTP | REST API |
| **8333** | TCP | P2P networking |
| **7782** | TCP | RPC server |
| **9090** | HTTP | Prometheus metrics |

---

## License

QUANTA operates under an **Open-Core Dual License** model:
1. **Core Protocol:** Licensed under the [GNU AGPLv3](LICENSE).
2. **Native Templates & APIs:** Licensed under a [Proprietary Commercial License](COMMERCIAL_LICENSE.md).

Managed by QuantaLabs Pvt Ltd.
