# QUANTA WHITEPAPER

**A Quantum-Resistant Blockchain Built for the AI Agent Era**

Version 3.0 | August 2026

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)  
**Repository**: [github.com/quantachain/quanta](https://github.com/quantachain/quanta)

---

## Executive Summary

QUANTA is the first production-ready blockchain purpose-built with post-quantum cryptography from inception. While current blockchains face existential risk from quantum computers capable of breaking elliptic curve cryptography, QUANTA provides future-proof security through NIST-standardized algorithms that resist both classical and quantum attacks.

**Key Highlights of V3 (Katenet):**
- **Quantum-Resistant Security**: NIST-standardized Falcon-512 signatures and Kyber-1024 encryption — deployed from genesis, not retrofitted.
- **AlephBFT Consensus & DPoS**: Version 3 introduces a high-performance Delegated Proof of Stake (DPoS) system secured by leaderless Asynchronous Byzantine Fault Tolerance (AlephBFT) with 6-second block finality.
- **Native Delegation**: Any QUA holder can delegate their stake to a trusted validator, earn proportional rewards, and withdraw at any time subject to the unbonding period.
- **Sustainable Economics**: A dual-stream epoch pool — block issuance plus 35% of all transaction fees — creates validator income that scales automatically with AI agent network usage.
- **Production-Ready Performance**: Up to 400 TPS with 4MB blocks, parallel Falcon-512 signature verification, bloom filter mempools, and zstd compression.
- **Native AI Agent Contracts**: Five production-grade, auditable native contract templates replace Turing-complete VMs: Escrow (HTLC), Agent Job, Agent Bid (auction), Payment Stream, and Agent Registry.
- **Minimal Exploit Surface**: No bytecode interpreter, no re-entrancy, no logic bugs. Every contract template is hardcoded in Rust and auditable at the protocol level.

---

## 1. Introduction

### 1.1 The Quantum Threat

Current blockchain systems rely on elliptic curve cryptography (ECDSA, EdDSA) for transaction signing. These algorithms are vulnerable to Shor's algorithm, which can be efficiently executed on sufficiently powerful quantum computers. Conservative estimates suggest that quantum computers capable of breaking 256-bit ECDSA could exist within 10-15 years. Over $1.7 trillion in crypto assets rely on ECDSA today — none of them are quantum-safe.

### 1.2 The Shift to DPoS (Version 3)

QUANTA was initially launched fairly through Proof-of-Work to ensure wide distribution without pre-mines or ICOs. In Version 3, the network evolves to Delegated Proof of Stake (DPoS) combined with AlephBFT consensus. This allows the network to process blocks every 6 seconds with absolute finality, while drastically reducing energy consumption and providing sustainable yield for token holders who secure the network.

---

## 2. Cryptographic Foundations

### 2.1 Post-Quantum Cryptography (PQC)

QUANTA implements NIST-standardized post-quantum algorithms as consensus-critical primitives:

#### Falcon-512 (Digital Signatures)
- **Type**: Lattice-based signature scheme (NTRU lattices).
- **Security Level**: NIST Level 1 (equivalent to AES-128 classical; 64-bit post-quantum via Grover).
- **Key Sizes**: Public key: 897 bytes; Signature: up to 666 bytes.
- **Performance**: ~0.8 ms signing, ~0.1 ms verification (pre-quantum hardware).

#### Kyber-1024 (Key Encapsulation)
- **Type**: Module-LWE-based key encapsulation mechanism (ML-KEM).
- **Security Level**: NIST Level 5 (equivalent to AES-256).
- **Use Case**: Wallet encryption, secure key storage, HD wallet seed protection.

#### SHA3-256 (Hashing)
- **Type**: Keccak-based cryptographic hash function.
- **Security**: 256-bit collision resistance; quantum-safe.

---

## 3. Consensus Mechanism: AlephBFT & DPoS

### 3.1 Delegated Proof of Stake (DPoS)

QUANTA Version 3 eliminates mining and replaces it with permissionless staking.

- **Minimum Validator Stake**: 100,000 QUA (100,000,000,000 microunits).
- **Committee Size**: Up to 21 active validators per epoch, ranked by total stake (self-stake + delegated stake).
- **Epoch Mechanics**: The chain is organized into Epochs of 1,000 blocks each (~100 minutes). The validator committee is computed deterministically from on-chain stake at the start of each epoch.
- **Unbonding Period**: Validators and delegators must wait 60 epochs (~4.2 days) before staked or delegated QUA is returned after unstaking or undelegating.

#### Native Delegation
Any QUA holder can delegate their stake to an active validator without running a node:
- **Delegate**: Lock QUA behind a chosen validator, increasing their committee weight and proportional reward share.
- **Reward Split**: At each epoch boundary, the validator receives a minimum 10% commission. The remaining 90% of the validator's total epoch reward is distributed proportionally among all delegators by their delegated amount.
- **Undelegate**: Initiates the 60-epoch unbonding period.

### 3.2 Asynchronous Byzantine Fault Tolerance (AlephBFT)

- **Block Time**: Fixed 6-second slots (SLOT_SECONDS = 6).
- **Leaderless Consensus**: AlephBFT is leaderless by design. All committee validators propose blocks simultaneously once the slot gate opens. AlephBFT's internal DAG mechanism deterministically selects one block per slot, achieving instant, provable finality without a single point of failure or targeted leader DoS.
- **Session Model**: Validators participate in sessions of 60 blocks each. At each session boundary, the active committee is re-evaluated from on-chain stake.
- **Fault Tolerance**: The network remains live and correct as long as fewer than 1/3 of validators are Byzantine.

### 3.3 Slashing Conditions

To ensure network security, malicious or offline validators are penalized:
- **Downtime (Soft-Slash)**: If a validator misses more than 30% of their designated slots in an epoch, 5% of their stake is slashed and burned.
- **Equivocation (Hard-Slash)**: Double-signing results in a 50% stake slash, with 10% rewarded to the whistleblower and the remainder burned. The validator is subjected to a 180-epoch cooldown before being allowed to re-register.

---

## 4. Economic Model (V3 Tokenomics)

The V3 tokenomics are designed specifically for a validator-secured, AI-agent economy. The system uses a **dual-stream epoch pool** to ensure validators are rewarded for both uptime and network utility.

### 4.1 Block Reward & Epoch Pool Distribution

Every block produced by the network feeds two streams into the epoch pool:

1. **Block Reward** (0.5 QUA/block): 8% is routed to the on-chain Treasury; the remaining 92% is sent to the EPOCH_POOL_ADDRESS.
2. **Transaction Fees**: 50% is permanently burned; 15% goes to the Treasury; the remaining **35% is also sent to the EPOCH_POOL_ADDRESS**.

At the end of every epoch (every 1,000 blocks), the entire accumulated epoch pool is distributed proportionally to all active validators based on their block proposal count (uptime) during that epoch. Validators with higher uptime earn a proportionally larger share.

This dual-stream design means **validator income scales automatically with AI agent activity**. As Escrow, Stream, and Agent-Bid contracts generate fee volume, validator income grows without any protocol change.

**Emission Curve:**
- **Block Reward**: 0.5 QUA/block.
- **Decay**: Smooth 15% annual reduction (no sudden halving shocks).
- **Floor**: Minimum reward of 0.1 QUA/block in perpetuity (ensures validators always receive base income).

### 4.2 Fee Structure

Transaction fees are paid in microunits (1 QUA = 1,000,000 microunits) with a minimum base fee of 0.001 QUA.

| Destination | Share | Purpose |
|---|---|---|
| **Burned** | 50% | Permanent deflation — removes QUA from circulation forever |
| **Epoch Pool (Validators)** | 35% | Pooled and distributed to all validators at each epoch boundary by uptime |
| **Treasury (QEF)** | 15% | Funds ecosystem development, AI SDK grants, security audits |

### 4.3 Treasury

The Quanta Ecosystem Fund (QEF) uses a 3-of-5 Falcon-512 Multisig (ms69216b1d10425689704d5ae3b2a4aa17049f59b1). Any 3 of the 5 keyholders must sign to authorize a spend. The treasury automatically accumulates:
- **8% of every block reward** (protocol issuance).
- **15% of every transaction fee** (network usage fees).

---

## 5. Native AI Agent Contract Layer

### 5.1 Design Philosophy

QUANTA deliberately rejects Turing-complete smart contracts (EVM/WASM). Instead, it provides five **hardcoded, auditable native contract templates** written directly into the consensus engine in Rust. Each template has a fixed, predictable execution path — no bytecode interpreter, no re-entrancy, no gas estimation errors.

This eliminates the entire class of smart contract exploits (re-entrancy, overflow, logic bugs) while still providing the financial primitives required for a fully autonomous machine-to-machine AI economy.

### 5.2 Native Contract Templates

#### Template 1 — Escrow (HTLC)
Hash Time-Locked Contract. An employer locks QUA for a worker. The worker claims payment by submitting a cryptographic preimage (proof of work completion). If unclaimed before the refund height, the employer reclaims funds atomically.
- **CLI**: quanta-wallet deploy-escrow, claim-escrow, refund-escrow

#### Template 2 — Agent Job
Single-worker AI job contract. An employer assigns a task to a specific worker address with a deadline. The worker submits a result hash to claim payment. The employer can refund after the deadline if the worker fails to deliver.
- **CLI**: quanta-wallet deploy-agent-job, claim-agent-job, refund-agent-job

#### Template 3 — Agent Bid (Auction)
Multi-agent competitive auction. An employer posts a task and reward. Multiple AI agents submit competitive bids with result hashes and prices. The employer selects the winner, who is paid automatically. Any unspent balance is refunded.
- **CLI**: quanta-wallet deploy-agent-bid, submit-bid, select-winner

#### Template 4 — Payment Stream
Continuous pay-per-block subscription. An employer deposits QUA and sets a rate per block. The recipient (an AI service provider) can withdraw accrued funds at any time. The owner can cancel and reclaim the unspent remainder.
- **CLI**: quanta-wallet deploy-stream, withdraw-stream, cancel-stream

#### Template 5 — Agent Registry
On-chain AI service discovery. Agents register themselves with an endpoint hash, service type, and price per call. Other agents query the registry to discover and hire services without any off-chain coordination layer.
- **CLI**: quanta-wallet register-agent, update-agent

---

## 6. Network Architecture

### 6.1 Throughput and Capacity

Falcon-512 transactions are around 1,713 bytes each (larger than ECDSA). QUANTA is architected specifically around this reality:
- **Max Block Transactions**: 2,000 txs.
- **Max Block Size**: 4 MB.
- **TPS (Transactions per Second)**: ~400 TPS (2,000 txs / 6s slot).
- **Compression**: zstd compression reduces wire size by ~4x.
- **Verification**: Rayon parallel signature verification across all CPU cores reduces a 2,000-tx block to ~225ms verification time.
- **Mempool**: Bloom filter for O(1) duplicate detection; 5,000 tx capacity with a per-sender cap of 25 to prevent griefing.

### 6.2 Transaction Types

All base transactions are signed with Falcon-512:

- Transfer: Standard value transfer between addresses.
- TimeLockTransfer: Locks funds until a specified block height (vesting/escrow).
- Stake / Unstake: Validator registration and deregistration.
- Delegate / Undelegate: Delegation of QUA to an active validator for DPoS participation.
- ContractDeploy: Deploys one of the five native AI contract templates on-chain.
- ContractCall: Interacts with a deployed native contract (claim, refund, bid, withdraw, etc.).

---

## 7. System Requirements

To ensure stable block production and AlephBFT consensus, validator nodes must meet the following hardware requirements:

- **CPU:** 4 Cores (modern x86_64 or ARM64)
- **RAM:** 8 GB
- **Storage:** 50 GB SSD (NVMe recommended)
- **Network:** Reliable 100 Mbps connection with a static IP

Operators can run nodes in Archive, Pruned (last 30 days of blocks), or Light configurations.

---

## 8. Conclusion

QUANTA is not a retrofitted patch — it is a blockchain designed from genesis with NIST-standardized post-quantum cryptography. With Falcon-512 signatures, Kyber-1024 encryption, 6-second finality through leaderless AlephBFT, a robust Delegated Proof of Stake mechanism with native delegation, a dual-stream epoch reward pool, and a native AI agent contract layer, QUANTA provides the infrastructure for autonomous AI agents and digital value to thrive securely in the quantum era.

**Contact**: Kishore K, Founder | admin@quantachain.org | quantachain.org