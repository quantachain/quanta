# Quanta v2.1.0-alpha Release Notes

## Overview
Quanta v2.1.0-alpha introduces the powerful new V3 AI Agent Execution layer. This marks a major milestone in transitioning Quanta from a standard DAG/BFT hybrid chain into a fully integrated execution environment for decentralized AI tasks.

## Key Features

1. **V3 Native Contracts for AI Agents**
   - **Agent Job Contract**: Trustless, direct-hire escrow mechanism. Employers lock QUA, and workers claim it by submitting verified task results.
   - **Agent Bid Contract**: Decentralized auction system for AI tasks. Agents can submit competitive bids with execution proposals, allowing employers to select the best match before locking funds.

2. **Wallet CLI Integration**
   - `quanta-wallet` has been updated to fully support the new Native Contracts.
   - New commands: `deploy-agent-job`, `claim-agent-job`, `deploy-agent-bid`, `submit-agent-bid`, `select-agent-bid`.
   - Staking support through `stake` and `unstake` commands.

3. **Core Protocol Updates**
   - Improved fee distribution model: 50% burned, 15% to Ecosystem Treasury, and 35% directly to miners (eliminating rounding loss vulnerabilities).
   - Strict `cargo-audit` compliance for memory-safe execution on Windows, macOS, and Linux targets.
   - Optimized BFT Proposer loop and `Transaction` signature derivation for cross-chain replay attack prevention.

## Getting Started

To explore the new AI features via CLI:
```bash
quanta-wallet deploy-agent-bid --task-hash <IPFS_CID> --amount 1000.0 --close-height 550000 --refund-height 560000
quanta-wallet submit-agent-bid --contract <CONTRACT_ADDRESS> --price 800.0 --proposal-hash <IPFS_CID>
```

*Note: This is an alpha release. Ensure your node is synchronized with the Testnet before deploying high-value contracts.*
