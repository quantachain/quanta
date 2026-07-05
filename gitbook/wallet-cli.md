# Wallet CLI

The `quanta-wallet` CLI has been rewritten for V2 to support HD Wallets (BIP-39), Raw Wallets, and Headless AI execution. All cryptography utilizes Post-Quantum **Falcon-512** signatures.

> **Note on Installation:** You do not need to compile the CLI from source or run Docker to use it. Pre-compiled binaries for Windows, macOS, and Linux are automatically attached to the GitHub Releases page. Just download the executable for your OS and run it!

## Global Configuration

You can set a default node and wallet file so you don't have to specify `--node` or `--wallet` for every single command:

```bash
# Set your default node URL and wallet file
quanta-wallet config set --node https://rpc.quantachain.org --wallet mywallet.json

# View your current configuration
quanta-wallet config get
```

## Creating & Managing a Wallet

For human users, use HD Wallets:
```bash
quanta-wallet new --file mywallet.json
```
*(Provides a 24-word recovery phrase)*

For servers or AI agents, you can use Raw Wallets (no recovery phrase):
```bash
quanta-wallet new-raw --file agent.qua
```

Other wallet commands:
```bash
# Check your address
quanta-wallet address --file mywallet.json

# Reveal your 24-word recovery mnemonic (HD Wallets only)
quanta-wallet show-mnemonic --file mywallet.json

# Export your raw Falcon-512 private key (for raw .qua validator wallets)
quanta-wallet export-private-key --file validator.qua

# Restore a wallet from a mnemonic phrase
quanta-wallet restore --file mywallet.json

# Check wallet balance (requires a running node)
quanta-wallet info --file mywallet.json --node http://localhost:3000
```

## Basic Transactions

```bash
# Send QUA to an address
quanta-wallet send --to <ADDR> --amount 10.5 --fee 0.001

# Send QUA with an attached data payload (AI agent data provenance)
quanta-wallet send-with-data --to <ADDR> --amount 10.5 --data '{"task": "complete"}'
```

## BFT Validator Staking

To participate in consensus, you must stake QUA.

```bash
# Export your public key to a validator.json file for the genesis block
quanta-wallet export-validator --wallet wallet.qua --out validator.json

# Register as a BFT validator dynamically by staking QUA (minimum 100,000)
quanta-wallet stake --wallet validator.qua --amount 100000.0

# Deregister and begin the unbonding period (funds locked for 2 epochs)
quanta-wallet unstake --wallet validator.qua
```

## AI Smart Contracts (Escrow)

```bash
# Deploy a trustless Escrow contract
# The beneficiary can claim the funds only by providing the preimage 
# whose SHA3-256 hash matches the --secret-hash.
quanta-wallet deploy-escrow --beneficiary <WORKER_ADDR> --secret-hash <HASH> --amount 5.0

# Claim funds from an escrow contract
quanta-wallet claim-escrow --contract <ADDR> --preimage <HEX>
```

## AI Agent Headless Mode

Quanta V2 is explicitly designed as a settlement layer for AI Agents. To allow agents to autonomously sign transactions without interactive password prompts, set the following environment variable:

```bash
export QUANTA_WALLET_PASSWORD="your_password"
```
