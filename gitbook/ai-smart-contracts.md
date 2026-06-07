# AI & Smart Contracts Ecosystem

QuantaChain is built with Post-Quantum Cryptography (PQC) and natively supports primitives designed for trustless AI-to-AI interactions, autonomous agents, and smart contract execution. 

This guide covers the core features you can use to build AI-driven dApps using the `quanta-wallet` CLI.

## Trustless Escrows

The Quanta Escrow system allows an employer (or human) to lock funds on-chain, which a worker (or AI agent) can only claim by providing a cryptographic proof of task completion.

This solves the "trust" problem in AI task outsourcing: the AI knows it will get paid if it does the work, and the human knows they won't pay unless the work is verifiably completed.

### 1. Deploying an Escrow
To deploy an escrow, you must provide the worker's address and the SHA3-256 hash of the expected output.

```bash
# Example: The employer locks 50 QUA for a specific task output hash
quanta-wallet deploy-escrow \
  --beneficiary 0xWorkerAddressHere \
  --secret-hash 3a7f8b9c... \
  --amount 50.0
```

*This command generates a new Contract Address (starting with `0xc_`). Share this address with the worker.*

### 2. Claiming an Escrow
Once the AI worker finishes the task, it proves completion by submitting the raw data (the preimage) whose hash matches the one committed during deployment.

```bash
# Example: The worker claims the 50 QUA by submitting the hex-encoded task output
quanta-wallet claim-escrow \
  --contract-address 0xc_ContractAddressHere \
  --preimage deadbeef...
```

If the `SHA3-256(preimage)` matches the `secret-hash`, the 50 QUA is atomically transferred to the worker.

---

## AI Data Provenance (Send With Data)

When an AI agent produces research, data scraping results, or financial analysis, proving the *provenance* (origin and timestamp) of that data is critical.

Quanta allows you to cryptographically bind a data payload to a standard transaction. Because transactions are signed with the agent's Falcon-512 private key, the payload is permanently anchored to their identity on the ledger.

### Anchoring Data
```bash
# Example: Sending 0.01 QUA with a JSON payload attached
quanta-wallet send-with-data \
  --to 0xRecipientAddressHere \
  --amount 0.01 \
  --payload '{"action":"scraping_complete", "result_hash":"f9a2..."}'
```

Anyone querying this transaction on QuaScan or via the API can read the payload and mathematically verify that it was signed by the sender at the block's timestamp.

---

## Native Smart Contracts

If you are developing custom protocol templates, you can interact with them directly via the CLI's generic contract caller.

### Deploying a Contract
```bash
quanta-wallet contract-deploy \
  --template-id 1 \
  --init-args '{"name": "MyToken"}' \
  --amount 0
```

### Calling a Contract
```bash
quanta-wallet contract-call \
  --contract-address 0xc_YourContractAddress \
  --method "transfer" \
  --call-args '{"to": "0x...", "amount": 100}'
```
