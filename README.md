# QUANTA

**The Post-Quantum BFT Settlement Layer**

[![License: AGPLv3](https://img.shields.io/badge/License-AGPLv3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
[![Rust Version](https://img.shields.io/badge/rust-1.70%2B-blue.svg)](https://www.rust-lang.org)

**Founder**: Kishore K — admin@quantachain.org — quantachain.org

---

## Overview

QUANTA V2 is a production-ready, post-quantum blockchain engineered for extreme performance and determinism. Moving away from legacy Proof-of-Work, QUANTA is built on a custom Delegated Proof-of-Stake (DPoS) engine combined with Asynchronous Byzantine Fault Tolerance (AlephBFT).

This architecture provides the instant finality, high throughput, and cryptographic security required for high-frequency institutional settlement and automated agent networks.

For full technical specifications, architecture diagrams, and node operation guides, please refer to the **[Official QUANTA Documentation](https://github.com/quantachain/quanta/tree/main/gitbook)**.

---

## Chain Features & Optimizations

QUANTA has been heavily optimized at the networking, consensus, and storage layers:

- **Asynchronous BFT Consensus**: Integrates AlephBFT for deterministic, 6-second block finality. The network cannot fork, and transactions are irreversible once included in a block.
- **Quantum-Resistant Cryptography**: Secures all transactions using NIST-standardized Falcon-512 signatures and SHA3-256 hashing, safeguarding against future cryptographic breakthroughs.
- **Parallel Execution**: Leverages Rayon for thread-safe, parallel transaction verification and multi-core signature validation.
- **O(1) Mempool Filtering**: Implements Bloom filters for constant-time duplicate transaction detection, dramatically reducing memory overhead during network spikes.
- **Optimized Storage Engine**: Utilizes Sled KV with Zstd compression, an LRU block cache, and amortized O(1) state pruning to keep node storage requirements lightweight.
- **Native Smart Templates**: Built-in, audited contract templates (Escrow, Multisig, Agent Jobs) execute at the native protocol level, avoiding the overhead and attack vectors of Turing-complete virtual machines.
- **Institutional Treasury**: Native 3-of-N Multisig governance enforced directly by the consensus layer.

---

## Quick Start (Docker)

We strongly recommend running QUANTA via Docker to ensure environment consistency. 

```bash
# Pull the latest image
docker pull xd637/quanta-node:latest

# Run a detached background node
docker run -d \
  --name quanta-node \
  --restart always \
  -p 3000:3000 -p 8333:8333 -p 7782:7782 -p 9090:9090 \
  -v quanta-data:/home/quanta/quanta_data \
  -v quanta-logs:/home/quanta/logs \
  xd637/quanta-node:latest
```

Check the node status:
```bash
curl -X POST http://127.0.0.1:7782/ -H "Content-Type: application/json" -d '{"jsonrpc": "2.0", "method": "node_status", "params": {}, "id": 1}'
```

For instructions on building from source or running a validator node, see the [Node Operator Guide](gitbook/node-operator-guide.md).

---

## Documentation Directory

The complete documentation is hosted in the `/gitbook` directory:
- **[Quick Start](gitbook/quick-start.md)**
- **[Node Operator Guide](gitbook/node-operator-guide.md)**
- **[Wallet CLI & Staking](gitbook/wallet-cli.md)**
- **[JSON-RPC API Guide](gitbook/rpc-node-guide.md)**
- **[Architecture & Consensus](gitbook/consensus-bft.md)**

---

## License

QUANTA operates under an Open-Core Dual License model:
1. **Core Protocol**: Licensed under the GNU AGPLv3.
2. **Native Templates**: Licensed under a Proprietary Commercial License.

Managed by QuantaLabs Pvt Ltd.
