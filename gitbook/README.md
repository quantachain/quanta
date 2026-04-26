# QuantaChain Documentation

Welcome to the official Quanta Protocol documentation.

Quanta is a post-quantum blockchain built for institutional settlement. Every transaction is signed with Falcon-512 — a NIST-standardized lattice-based signature scheme immune to Shor's algorithm.

---

## Documentation Structure

| Section | Description |
|---------|-------------|
| [Installation](installation.md) | Run a node with Docker or build from source |
| [Quick Start](quick-start.md) | Node, wallet, and mining in 10 minutes |
| [Release Notes](release-notes.md) | Alpha v0.7.1 and v0.7.0 changes |
| [Node Operator Guide](node-operator-guide.md) | VPS, NGINX, HTTPS, monitoring |
| [Docker Deployment](docker-deployment.md) | Docker, custom images, upgrade workflow |
| [Configuration](configuration.md) | quanta.toml reference |
| [Mining Guide](mining-guide.md) | Rewards, commands, optimization |
| [Wallet Operations](wallet-operations.md) | HD wallets, transfers, security |
| [API Reference](api-reference.md) | REST endpoints (port 3000) |
| [SDK Integration](sdk-integration.md) | Build with quanta-sdk (JS/TS) |
| [Technical Specifications](technical-specs.md) | Consensus, block structure, network |
| [Quantum Resistance](quantum-resistance.md) | Falcon-512, Kyber-1024, SHA3-256 |
| [Security](security.md) | Threat model, attack mitigations |
| [Contributing](contributing.md) | How to contribute |

---

## Current Version

**Testnet Alpha v0.7.1** — no mainnet yet. Do not use real funds.

```bash
docker pull xd637/quanta-node:latest
```

---

## Quick Reference

**Public RPC**: `https://rpc.quantachain.org`

| Port | Purpose |
|------|---------|
| 3000 | REST API |
| 8333 | P2P networking |
| 7782 | RPC (CLI to node) |
| 9090 | Prometheus metrics |

---

## Links

- [Website](https://www.quantachain.org)
- [GitHub](https://github.com/quantachain/quanta)
- [Docker Hub](https://hub.docker.com/r/xd637/quanta-node)
- [NPM: quanta-sdk](https://www.npmjs.com/package/quanta-sdk)
- [NPM: quanta-wasm](https://www.npmjs.com/package/quanta-wasm)
