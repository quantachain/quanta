# QuantaChain V2 Documentation

Welcome to the official documentation for **QuantaChain V2**, the first Post-Quantum Institutional Settlement Layer built with AlephBFT consensus and Falcon-512 lattice signatures.

## What is Quanta V2?

Quanta V2 is a massive architectural upgrade from the V1 era. It completely removes Proof-of-Work mining in favor of **Asynchronous Byzantine Fault Tolerance (AlephBFT)**, providing deterministic 6-second block finality.

### Key Features
*   **AlephBFT Consensus:** 6-second deterministic block times. Once a block is committed, it is instantly final.
*   **Post-Quantum Cryptography:** All network signatures utilize **Falcon-512**, and wallets are encrypted via **Kyber-1024**.
*   **AI Agent Native:** Transactions include a `payload` field signed by Falcon-512. The `quanta-wallet` supports headless execution via the `QUANTA_WALLET_PASSWORD` environment variable, enabling AI agents to autonomously hold and transfer funds via Escrow contracts.
*   **Institutional Treasury:** Native 3-of-N Multisig (Falcon-512) for secure treasury management.

## Getting Started

If you want to spin up a node quickly, head over to the [Quick Start](quick-start.md) guide. If you want to dive into the technical details of the consensus shift, read the [AlephBFT Consensus](consensus-bft.md) documentation.
