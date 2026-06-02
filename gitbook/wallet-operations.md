# Wallet Operations

The `quanta-wallet` CLI has been rewritten for V2 to support HD Wallets (BIP-39), Raw Wallets, and Headless AI execution. All cryptography utilizes Post-Quantum **Falcon-512** signatures.

## Creating a Wallet
For human users, use HD Wallets:
```bash
quanta-wallet new --file mywallet.json
```
*(Provides a 24-word recovery phrase)*

For servers or AI agents, you can use Raw Wallets:
```bash
quanta-wallet new-raw --file agent.qua
```

## AI Agent Headless Mode
Quanta V2 is explicitly designed as a settlement layer for AI Agents. To allow agents to autonomously sign transactions without interactive password prompts, set the following environment variable:

```bash
export QUANTA_WALLET_PASSWORD="your_password"
```

Example AI commands:
```bash
# Check wallet balance and info
quanta-wallet info --file mywallet.json

# Deploy an escrow contract
quanta-wallet deploy-escrow --beneficiary <WORKER_ADDR> --secret-hash <HASH> --amount 5.0
```
