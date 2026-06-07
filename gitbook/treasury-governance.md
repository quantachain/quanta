# Treasury & Network Governance

QuantaChain is fundamentally governed by its community and secured by its BFT validators. To manage shared protocol funds (like validator fees or community donations), Quanta utilizes a native on-chain Multi-Signature Treasury system.

This guide outlines how the treasury works and how the community manages protocol upgrades through Quanta Improvement Proposals (QIPs).

## The Multi-Sig Treasury

The treasury is controlled by a set of trusted keyholders (often elected community members or core developers) via a 3-of-N multi-signature scheme. No single entity can spend treasury funds; a quorum of signers is required to execute any transaction.

### 1. Initializing a Treasury
A new treasury configuration is created by generating a set of keys and specifying the total number of signers.

```bash
# Example: Initialize a 3-of-5 treasury (requires 3 signatures to spend)
quanta-wallet treasury-init --signers 5
```
This generates 5 distinct Falcon-512 wallet files and outputs the Treasury Address.

### 2. Proposing a Spend
Anyone holding a treasury key can propose a spend. The proposal is generated as an unsigned JSON file, which is then distributed to the other signers (e.g., via the forum or private channels).

```bash
quanta-wallet treasury-propose \
  --to 0xRecipientAddress \
  --amount 1000 \
  --fee 0.1
```
This command creates a `proposal.json` file containing the exact bytes to be signed.

### 3. Signing a Proposal
Keyholders review the `proposal.json`. If they agree with the spend (e.g., funding a community marketing campaign), they sign it with their specific key.

```bash
quanta-wallet treasury-sign \
  --key-file treasury_key0.qua \
  --key-index 0 \
  --proposal proposal.json
```
The signer then sends the output (their signature blob) to the coordinator.

### 4. Broadcasting the Executed Spend
Once the required quorum (e.g., 3 signatures) is collected, the coordinator broadcasts the fully signed transaction to the network.

```bash
quanta-wallet treasury-broadcast \
  --proposal proposal.json \
  --signatures sig0.hex,sig1.hex,sig2.hex
```
The network validates that all signatures are valid Falcon-512 signatures belonging to the authorized keys, and executes the transfer.

---

## Quanta Improvement Proposals (QIPs)

To maintain decentralization and ensure long-term stability, Quanta uses a formalized process for protocol upgrades.

A **QIP** is a design document providing information to the Quanta community, describing a new feature for the network or its processes.

### The QIP Workflow
1. **Idea Phase (Forum):** An idea is posted to the Quanta Community Forum under the *Governance & Treasury* section. The community discusses the feasibility and demand for the upgrade.
2. **Drafting:** A formal QIP document is drafted (similar to Ethereum's EIPs) detailing the technical specifications, rationale, and backwards compatibility.
3. **Community Review:** The draft is submitted to the community for review.
4. **Implementation:** If consensus is reached, developers build the upgrade.
5. **Validator Adoption:** The network upgrade is deployed. Validators must update their nodes to the new version to enforce the new consensus rules (e.g., the transition to AlephBFT).

*For major treasury spends, the QIP process should also be followed to ensure the community agrees before the multi-sig keys are used.*
