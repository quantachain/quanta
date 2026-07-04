# AI & Smart Contracts Ecosystem

QuantaChain is built with Post-Quantum Cryptography (PQC) and natively supports primitives designed for trustless AI-to-AI interactions, autonomous agents, and smart contract execution. 

This guide covers the core features you can use to build AI-driven dApps using the `quanta-wallet` CLI.

## Native Smart Contracts

Quanta V2 has **5 Native AI Contracts** baked directly into the consensus layer, designed for PQC-native M2M and AI agent economies:

1. `TEMPLATE_ESCROW` (1) - HTLC hash-time locked escrow with refund support
2. `TEMPLATE_AGENT_JOB` (2) - Single-worker AI job with deadline and refund
3. `TEMPLATE_AGENT_BID` (3) - Multi-agent auction: employer picks the best result
4. `TEMPLATE_STREAM` (4) - Streaming payments (pay-per-block subscription)
5. `TEMPLATE_AGENT_REGISTRY` (5) - On-chain AI service discovery registry

### Deploying a Contract

Because these contracts are native to the chain, you don't upload code. Instead, you deploy an instance of a template by using the generic `contract-call` (wait, actually they are deployed via the API programmatically, or in the case of Escrow, there is a dedicated CLI helper).

For the Escrow contract, you can use the dedicated CLI command:
```bash
quanta-wallet deploy-escrow \
  --beneficiary 0xWorkerAddressHere \
  --secret-hash 3a7f8b9c... \
  --amount 50.0
```

For the other 4 templates, developers typically deploy them programmatically by constructing a `TransactionType::ContractDeploy` transaction via the API, passing the `template_id` (1-5) and the JSON-encoded `init_args`.

### Calling a Contract

Once a contract is deployed, you can interact with it directly via the CLI's generic contract caller.

```bash
quanta-wallet contract-call \
  --contract 0xc_YourContractAddress \
  --method "claim" \
  --args '{"preimage": "deadbeef"}'
```

---

## Trustless Escrows (Deep Dive)

The Quanta Escrow system allows an employer to lock funds on-chain, which a worker (AI agent) can only claim by providing a cryptographic proof of task completion.

### 1. Deploying an Escrow
To deploy an escrow, provide the worker's address and the SHA3-256 hash of the expected output.

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
  --contract 0xc_ContractAddressHere \
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
  --data '{"action":"scraping_complete", "result_hash":"f9a2..."}'
```

Anyone querying this transaction on QuaScan or via the API can read the payload and mathematically verify that it was signed by the sender at the block's timestamp.
