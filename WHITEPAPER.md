# QUANTA WHITEPAPER

**A Quantum-Resistant Blockchain Built for the Future**

Version 2.0 | March 2026

**Founder**: Kishore K — [admin@quantachain.org](mailto:admin@quantachain.org) — [quantachain.org](https://quantachain.org)  
**Repository**: [github.com/quantachain/quanta](https://github.com/quantachain/quanta)

---

## Executive Summary

QUANTA is the first production-ready blockchain purpose-built with post-quantum cryptography from inception. While current blockchains face existential risk from quantum computers capable of breaking elliptic curve cryptography, QUANTA provides future-proof security through NIST-standardized algorithms that resist both classical and quantum attacks.

**Key Highlights:**
- **Quantum-Resistant Security**: NIST-standardized Falcon-512 signatures and Kyber-1024 encryption — deployed from genesis, not retrofitted
- **Fair Launch Model**: No pre-mine, no ICO, 100% community distribution through mining
- **Sustainable Economics**: Adaptive tokenomics with 70% fee burning, 50% anti-dump vesting, and perpetual mining incentives
- **Production-Ready**: Built in Rust with parallel signature verification, LRU caching, zstd compression, and sled embedded storage
- **Smart Contract Foundation**: Transfer, DeployContract, and CallContract transaction types with crypto-agile signature scheme field
- **Open Development**: Transparent roadmap, fully open-source codebase, and active security audits

This whitepaper presents the technical architecture, cryptographic foundations, consensus mechanism, economic model, and implementation details of the QUANTA blockchain.

**Target Audiences:**
- **Investors**: Understand the market opportunity and long-term value proposition
- **Developers**: Learn the technical architecture and integration possibilities
- **Miners**: Evaluate profitability and operational requirements
- **Researchers**: Review cryptographic choices and security analysis

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Cryptographic Foundations](#2-cryptographic-foundations)
3. [System Requirements](#3-system-requirements)
4. [Consensus Mechanism](#4-consensus-mechanism)
5. [Economic Model](#5-economic-model)
6. [Network Architecture](#6-network-architecture)
7. [Security Analysis](#7-security-analysis)
8. [Implementation Details](#8-implementation-details)
9. [Governance and Upgrades](#9-governance-and-upgrades)
10. [Roadmap](#10-roadmap)
11. [Comparison with Existing Solutions](#11-comparison-with-existing-solutions)
12. [Conclusion](#12-conclusion)
13. [References](#references)
14. [Appendix A: FAQ](#appendix-a-faq)

---

## 1. Introduction

### 1.1 The Quantum Threat

Current blockchain systems rely on elliptic curve cryptography (ECDSA, EdDSA) for transaction signing. These algorithms are vulnerable to Shor's algorithm, which can be efficiently executed on sufficiently powerful quantum computers. Conservative estimates suggest that quantum computers capable of breaking 256-bit ECDSA could exist within 10–15 years. Over $1.7 trillion in crypto assets rely on ECDSA today — none of them are quantum-safe.

### 1.2 Why Now?

- **NIST PQC Standards Finalized (2024)**: The cryptographic primitives are mature, peer-reviewed, and NIST-standardized
- **Migration Window is Closing**: Upgrading existing chains requires hard forks affecting millions of users; building correctly from day one eliminates this risk
- **Future-Proofing**: Infrastructure decisions made in 2026 will persist for decades. "Harvest Now, Decrypt Later" attacks are already feasible — adversaries can record transactions today for decryption on future quantum hardware
- **Institutional Demand**: Global PQC market projected at $29.95B by 2034 (37.72% CAGR) — institutional capital is actively seeking exposure

### 1.3 Design Philosophy

QUANTA is built on three core principles:

1. **Quantum Resistance First**: Security against both classical and quantum adversaries from block #0 — Falcon-512 for signatures, Kyber-1024 for encryption, SHA3-256 for hashing
2. **Economic Sustainability**: Tokenomics that align incentives for long-term network health — deflationary fee burn, anti-dump vesting, perpetual mining floor
3. **Operational Excellence**: Production-ready security, high-throughput performance, comprehensive monitoring, and operational tooling written entirely in Rust

---

## 2. Cryptographic Foundations

### 2.1 Post-Quantum Cryptography (PQC)

QUANTA implements NIST-standardized post-quantum algorithms as consensus-critical primitives:

#### Falcon-512 (Digital Signatures)
- **Type**: Lattice-based signature scheme (NTRU lattices)
- **Security Level**: NIST Level 1 (equivalent to AES-128 classical; 64-bit post-quantum via Grover)
- **Key Sizes**:
  - Public key: **897 bytes** (exact; consensus-enforced)
  - Secret key: ~1,281 bytes (never sent to network)
  - Signature: up to **666 bytes** (variable-length compressed; signed-message blob 33–698 bytes)
- **Performance**: ~0.8 ms signing, ~0.1 ms verification (pre-quantum hardware)
- **Rationale**: Optimal balance of security, size, and speed for high-throughput blockchain use. Compact signatures (vs. Dilithium's 2,420 bytes) minimize block size pressure

#### Kyber-1024 (Key Encapsulation)
- **Type**: Module-LWE-based key encapsulation mechanism (ML-KEM)
- **Security Level**: NIST Level 5 (equivalent to AES-256)
- **Key Sizes**:
  - Public key: 1,568 bytes
  - Secret key: 3,168 bytes
- **Use Case**: Wallet encryption, secure key storage, HD wallet seed protection
- **Rationale**: Maximum security for long-term key protection in wallet files

#### SHA3-256 (Hashing)
- **Type**: Keccak-based cryptographic hash function
- **Security**: 256-bit collision resistance; quantum-safe (Grover's halves to 128-bit effective — still secure)
- **Use**: Block hashing (double-SHA3-256), transaction hashing, address derivation, signing hash

#### Argon2id (Key Derivation)
- **Type**: Memory-hard password hashing
- **Configuration**: Time cost 2, Memory 65,536 KB, Parallelism 4
- **Rationale**: Resistant to GPU/ASIC attacks, quantum-safe (memory hardness is algorithm-independent)

### 2.2 Security Analysis

**Classical Attack Resistance**:
- Signature forgery: Computationally infeasible (2^128 operations for Falcon-512)
- Hash collisions: 2^256 operations for SHA3-256 (double-SHA3 blocks add a second layer)
- Brute force: Protected by Argon2id memory hardness (~4 GB RAM required per attempt)

**Quantum Attack Resistance**:
- Grover's algorithm: Effective key security halved — Falcon-512 retains 64-bit quantum security (safe until cryptanalysis improves significantly)
- Shor's algorithm: **Not applicable** to lattice-based cryptography. Shor's targets discrete log and factoring problems; Falcon is based on NTRU lattices
- Post-quantum cryptanalysis: No known polynomial-time attacks on Falcon or Kyber as of 2026

### 2.3 Canonical Signing Contract

All transaction signatures follow a strict canonical form enforced at the protocol level. This is a **consensus-frozen** specification — any deviation results in an invalid transaction.

```
signing_bytes = sender_utf8 
             || recipient_utf8
             || amount_le64
             || timestamp_le64
             || fee_le64
             || nonce_le64
             || public_key_bytes
             || sig_scheme_u8
             || tx_type_discriminant [|| tx_type_payload]

signing_hash  = SHA3-256("QUANTA_TX_V1:" || signing_bytes)

Signature     = Falcon-512.Sign(secret_key, signing_hash)
Verification  = Falcon-512.Verify(public_key, signature, signing_hash)
```

The domain prefix `"QUANTA_TX_V1:"` is prepended before hashing. This **domain separation** ensures that a signature produced for a QUANTA transaction cannot be replayed in any other protocol or context. The `sig_scheme` byte is included in the signing payload, preventing scheme substitution attacks (an attacker cannot swap the scheme byte after signing).

### 2.4 Crypto Agility

Every transaction encodes its signature scheme as a frozen byte field:

```
SignatureScheme:
  0 = Falcon512   (current, active — all nodes verify)
  1 = Reserved    (rejected by all current nodes; activatable via soft fork)
```

This field is part of the signing payload. Future post-quantum algorithms can be activated via soft fork by assigning new scheme values and updating node verification dispatch — **no structural changes** to the transaction format are required. This ensures the protocol can adapt to cryptographic advances without hard forks.

### 2.5 Falcon-512 Protocol Hardening (5 Measures)

Five specific hardening measures ensure Falcon-512 is deployed safely in a consensus-critical environment:

**1. Separation of Signing and Verification**

Signing (which uses floating-point Gaussian sampling internally) only ever occurs in wallets. Consensus nodes — validating node, mining node — only ever call the deterministic verification path. No keypair material is required or loaded on a validating or mining node.

**2. Domain-Separated Canonical Signing Format**

Transactions are signed over `SHA3-256("QUANTA_TX_V1:" || signing_bytes)`. The domain prefix is a consensus-frozen constant. Its inclusion ensures a QUANTA signature cannot be re-used in any other protocol context.

**3. Strict Pre-Verification Size Checks**

Before any polynomial arithmetic is invoked, the verifier checks:
- `public_key.len() == 897` (exact)
- `signed_msg.len()` ∈ `[33, 698]`
- Sender address must derive from supplied public key: `"0x" || hex(SHA3-256(pubkey)[:20])`
- Signature scheme must be `Falcon512 = 0`

Malformed inputs are rejected immediately, before entering Falcon internals. This ensures protocol invariant violations are caught at constant-time cost.

**4. Crypto Agility via `sig_scheme` Field**

Every transaction includes a `sig_scheme` byte covered by the signature. Enables soft-fork algorithm upgrades.

**5. Build Determinism**

The `pqcrypto-falcon` dependency is pinned to `= 0.3.0` (exact version). The `.cargo/config.toml` sets `target-feature=+strict-float` to enforce IEEE 754 compliant floating-point behavior, preventing compiler-introduced rounding divergence between x86_64 and ARM64 consensus nodes. Release builds set `codegen-units = 1` and `incremental = false` for byte-for-byte reproducible binaries.

### 2.6 Operational Impact of Post-Quantum Cryptography

**Signature Size Implications**:
- Falcon-512 signatures (~666 bytes) are 10.4× larger than ECDSA signatures (~64 bytes)
- Actual Falcon-512 transaction size: ~1,713 bytes (666 sig + 897 pubkey + payload overhead)
- Maximum block transactions corrected to **1,200** (not 2,000): 1,200 × 1,713 B = 2.06 MB ≤ 2 MB limit

**Performance Characteristics**:
- Signature generation: ~0.8 ms
- Signature verification: ~0.1 ms (single-threaded)
- Block validation (1,200 tx, 8 cores, parallel): ~225 ms
- Block validation (1,200 tx, cache hits): ~0 ms (100k-entry LRU cache)

**Mitigation Strategies (Implemented)**:
1. **Parallel Signature Verification** (Rayon): Serial 1,800 ms → 225 ms on 8 cores
2. **LRU Signature Cache**: 100,000 entries, ~80% hit rate in practice (transactions propagate multiple times before block inclusion)
3. **zstd Compression**: Block 2 MB → ~500 KB on wire (4× reduction)
4. **Bincode Serialization**: 22% smaller than JSON, 8× faster

---

## 3. System Requirements

### 3.1 Hardware Requirements

**Full Node (Archival)**:
- **CPU**: 4 cores @ 2.0 GHz (x86-64 or ARM64) — multi-core strongly recommended for parallel sig verification
- **RAM**: 8 GB minimum, 16 GB recommended
- **Storage**: 1 TB SSD (year 1, with zstd compression), plan for 5 TB over 5 years
- **Bandwidth**: 50 Mbps down, 20 Mbps up

**Pruned Node**:
- **CPU**: 2 cores @ 2.0 GHz
- **RAM**: 4 GB
- **Storage**: 400 GB SSD (maintains rolling 6-month window)
- **Bandwidth**: 25 Mbps down, 10 Mbps up

**Light Client** (Planned — Q2 2027):
- **CPU**: 1 core
- **RAM**: 1 GB
- **Storage**: 1 GB (headers + proofs only)
- **Bandwidth**: 5 Mbps

### 3.2 Network Requirements

**Ports**:
- `8333` — P2P TCP (default; configurable via `--network-port`)
- `7777` — REST API (default; configurable via `--port`)
- `7782` — RPC TCP server (default; configurable via `--rpc-port`)

**Connectivity**:
- IPv4 or IPv6 support
- Stable internet connection (residential broadband sufficient)
- Port forwarding recommended for incoming P2P connections

**Bootstrap Nodes (Testnet Q2 2026)**:
- `testnet-us-east.quanta.network:8333`
- `testnet-us-west.quanta.network:8333`
- `testnet-eu-west.quanta.network:8333`
- `testnet-eu-central.quanta.network:8333`
- `testnet-ap-southeast.quanta.network:8333`
- `testnet-ap-northeast.quanta.network:8333`

**DNS Seeds**:
- `seed.testnet.quanta.network`
- `nodes.testnet.quanta.network`
- `peers.testnet.quanta.network`

### 3.3 Software Requirements

**Operating Systems**:
- Linux: Ubuntu 20.04+, Debian 11+, CentOS 8+, Arch Linux
- macOS: 10.15 (Catalina) or later
- Windows: Windows 10 (build 1809+), Windows Server 2019+

**Dependencies for Node Operators**: None (single statically-linked binary)

**Dependencies for Compilation**:
- Rust 1.70+ (stable toolchain)
- LLVM 14+ (for cryptographic performance)

### 3.4 Storage Growth Projections (with zstd Compression)

```
Archive Nodes (full history):
  Year 1:  ~1.95 TB   (uncompressed: ~8.3 TB)
  Year 5:  ~9.75 TB
  Cost:    $30–$150/year (NVMe SSD)

Pruned Nodes (rolling 6 months):
  Year 1+: ~400 GB    (constant with rolling pruning)
  Cost:    $10–$60/year

Light Clients (headers only, planned):
  Year 1:  ~1 GB
  Year 5:  ~5 GB
```

---

## 4. Consensus Mechanism

### 4.1 Adaptive Proof-of-Work

QUANTA uses a modified proof-of-work consensus with dynamic difficulty adjustment. Mining is CPU-friendly (no ASIC-optimized hash function) using SHA3 (Keccak), which has no known hardware optimization advantage.

#### Mining Algorithm
```
Block Hash = SHA3-256(SHA3-256(block_data || nonce))
Valid Block: Hash starts with `difficulty` leading zero nibbles
```

Double-SHA3 provides a two-layer pre-image resistance barrier and eliminates length-extension attacks present in SHA-2 based double-hash constructions.

#### Difficulty Adjustment
- **Interval**: Every **2,016 blocks** (~5.6 hours at 10-second average)
  - *Why 2,016?* Security fix from original design (was 10 blocks). 2,016 prevents rapid oscillation — matches Bitcoin's proven stability window.
- **Target Block Time**: 10 seconds
- **Formula (pure integer math — no floats)**:
  ```
  scaled = round(current_difficulty × expected_time / actual_time)
  
  Where:
    expected_time = 2,016 blocks × 10 seconds = 20,160 seconds
    actual_time   = median_time_past(latest_block) - median_time_past(2016_blocks_ago)
    # Median-Time-Past (MTP) prevents timestamp manipulation attacks
  ```
- **Bounds** (integer percentages):
  - Maximum increase: ×1.15 per adjustment (15% cap — prevents rapid difficulty spikes)
  - Maximum decrease: ×0.85 per adjustment (15% floor — prevents hash-rate collapse death spiral)
  - Minimum difficulty: 4
  - Maximum difficulty: 2,147,483,647 (2^31−1, supports massive hashrate growth for decades)

### 4.2 Block Structure

```rust
Block {
    index:         u64,          // Block height (monotonically increasing)
    timestamp:     i64,          // Unix timestamp (seconds)
    transactions:  Vec<Tx>,      // Up to 1,200 transactions (2 MB limit with Falcon-512)
    previous_hash: String,       // SHA3-256 hash of prior block (chain linking)
    merkle_root:   String,       // SHA3-256 Merkle root of all transaction hashes
    nonce:         u64,          // Proof-of-work nonce
    difficulty:    u32,          // Leading-zero difficulty target
    hash:          String        // double-SHA3-256 of block header fields
}
```

**Genesis Block Parameters (Mainnet — Consensus-Critical)**:
- Timestamp: `1735689600` (2026-01-01 00:00:00 UTC)
- Difficulty: `6`
- Hash: `527a8a6ad3292c9b42c40f3d71fd3b89cdd79415106ce0b8d9f7f6690a96433d`

The genesis hash is hardcoded in `blockchain.rs`. Any mismatch immediately panics the node, preventing accidental cross-network contamination.

### 4.3 Transaction Types

QUANTA supports three transaction types, all signed with Falcon-512:

```rust
TransactionType {
    Transfer,
    // Standard value transfer between two accounts

    DeployContract { code: Vec<u8> },
    // Deploy on-chain program bytecode
    // Minimum fee: 10,000 microunits (0.01 QUA)

    CallContract { contract: String, function: String, args: Vec<u8> },
    // Invoke deployed contract function with arguments
    // Minimum fee: 5,000 microunits (0.005 QUA)
}
```

All transaction types share the identical signing pipeline (same domain prefix, same Falcon-512 verification). The `tx_type` discriminant byte and any type-specific payload are included in `signing_bytes` and thus covered by the signature.

### 4.4 Transaction Structure

```rust
Transaction {
    sender:     String,       // Address: "0x" + hex(SHA3-256(pubkey)[:20])
    recipient:  String,       // Recipient address (empty for DeployContract)
    amount:     u64,          // Amount in microunits (1 QUA = 1,000,000 microunits)
    fee:        u64,          // Transaction fee in microunits (min: 100)
    nonce:      u64,          // Monotonic account nonce (replay prevention)
    timestamp:  i64,          // Creation time; rejected if > 24 hours old
    signature:  Vec<u8>,      // Falcon-512 signed-message blob (33–698 bytes)
    public_key: Vec<u8>,      // Falcon-512 public key (must be exactly 897 bytes)
    sig_scheme: u8,           // Signature scheme: 0=Falcon512, 1=Reserved
    tx_type:    TransactionType
}
```

**Account Model**: QUANTA uses an **account-based model** (not UTXO). Each address has:
- `balance`: spendable microunits
- `nonce`: monotonically increasing counter (starts at 0, first tx uses nonce 1)
- `locked_balances`: list of `(amount, unlock_height)` pairs — coinbase maturity + mining vesting

### 4.5 Transaction Validation (Ordered Rules)

Each transaction must satisfy all checks in order:

1. **Signature scheme known**: `sig_scheme == 0` (Falcon512). Unknown values rejected.
2. **Non-empty fields**: Both `signature` and `public_key` must be non-empty.
3. **Public key length**: Exactly 897 bytes — checked before any crypto operations.
4. **Signed message length**: In range `[33, 698]` — checked before any crypto operations.
5. **Sender derives from public key**: `"0x" + hex(SHA3-256(pubkey)[:20])` must equal `sender`.
6. **Cryptographic verification**: `Falcon-512.Verify(pubkey, signature, SHA3-256("QUANTA_TX_V1:" || signing_bytes))` must succeed.
7. **Balance check**: `spendable_balance >= amount + fee`.
8. **Nonce ordering**: `tx.nonce == account_nonce + 1` (atomic — DashMap prevents race conditions).
9. **Timestamp validity**: `tx.timestamp >= now - 86400` (24-hour expiry window).
10. **Fee minimum**: `fee >= 100` microunits (0.0001 QUA).
11. **No duplicates**: `tx.hash()` not already in mempool or recent blocks.
12. **Size limit**: Serialized tx ≤ 100 KB (100,000 bytes; DoS protection).

### 4.6 Block Validation

Each block must satisfy:
1. **Proof-of-Work**: `hash.starts_with("0" × difficulty)` using double-SHA3-256
2. **Merkle Root**: `merkle_root == SHA3-256_merkle_tree(transaction_hashes)`
3. **Previous Hash**: Correctly references parent block hash
4. **Timestamp**: `prev.timestamp < block.timestamp <= now + 7200` (within 2 hours of current time)
5. **Transaction Validity**: All transactions individually valid (parallel Falcon-512 verification + nonce/balance state validation)
6. **Coinbase Correctness**: Exactly one `COINBASE` sender tx; amount must equal `immediate_reward + fee_to_miner`
7. **Treasury Correctness**: Treasury tx must exist when `treasury_amount > 0`; must send to `0x0000000000000000000000000000000000000001`
8. **Block Size**: Serialized ≤ 2,097,152 bytes (2 MB)
9. **Transaction Count**: ≤ 1,200 transactions
10. **Difficulty**: Must equal `calculate_next_difficulty()` exactly
11. **Checkpoint**: Must match hardcoded checkpoint hash at checkpoint heights

### 4.7 Performance Optimizations

**Parallel Signature Verification (Rayon)**:
```
Serial:    1,200 tx × 1.5 ms = 1,800 ms
Parallel:  1,200 tx × 1.5 ms ÷ 8 cores = 225 ms   ← 8× speedup
```

**Signature Verification Cache (LRU)**:
```
Cache size:  100,000 entries
Hit rate:    ~80% (transactions seen multiple times before block inclusion)
Cache hit:   0 ms  ← instant verification
Effective block validation time: ~45 ms at 80% cache
```

**Block Compression (zstd)**:
```
Uncompressed block:  ~2 MB
Compressed block:    ~500 KB  ← 4× reduction
Network daily data:  ~13 GB → ~3.25 GB
```

**Throughput**:
- **120 TPS** (1,200 tx ÷ 10 seconds)
- 17× higher than Bitcoin (~7 TPS)
- 8× higher than Ethereum (~15 TPS)
- Achieved despite 10.4× larger signatures than ECDSA

---

## 5. Economic Model

See [TOKENOMICS.md](./TOKENOMICS.md) for the complete economic specification.

### 5.1 Supply Overview

- **Initial Supply**: 0 QUA (fair launch — no pre-mine, no ICO, no team allocation)
- **Emission Schedule**: Exponential decay (15% annual reduction) with a 5 QUA perpetual floor
- **Soft Maximum**: ~1.5 billion QUA by year 15–20
- **Distribution**: 100% through proof-of-work mining

### 5.2 Block Reward Formula (Integer Math — Consensus Critical)

```
year = block_height / 3,153,600    (integer division)
base  = 100 QUA × (85/100)^year   (applied iteratively using integer ops)
reward = max(base, 5 QUA)

No floating point — integer division only, to prevent consensus forks
across x86_64 and ARM64 architectures.
```

### 5.3 Reward Distribution Per Block

```
Block Reward = R  (e.g., 100 QUA in Year 1)

Treasury allocation:  R × 5%  → Treasury address (0x0000...0001)
Miner reward:         R × 95% → Miner address

  Of miner reward:
    Immediate:  (R × 95%) × 50% = 47.5% of R  (spendable now)
    Locked:     (R × 95%) × 50% = 47.5% of R  (locked for 157,680 blocks / ~6 months)
```

### 5.4 Fee Distribution Per Block

```
Total fees = sum of all tx.fee in block

Fee burn:      fees × 70%  → Destroyed (deflationary)
Fee treasury:  fees × 20%  → Treasury address
Fee miner:     fees × 10%  → Miner address (added to coinbase)
```

---

## 6. Network Architecture

### 6.1 Peer-to-Peer Protocol

**Network Magic Bytes**:
- Testnet: `QUAX` (0x51554158)
- Mainnet: `QUAM` (0x5155414D)

**P2P Message Types**:
| Category | Messages |
|---|---|
| Handshake | `Version`, `VerAck` |
| Peer Discovery | `GetAddr`, `Addr` |
| Block Sync | `GetBlocks`, `Block`, `GetHeaders`, `Headers`, `GetHeight`, `Height` |
| Transactions | `NewTx`, `GetMempool`, `Mempool` |
| Maintenance | `Ping`, `Pong`, `Disconnect` |

**Security Parameters**:
- Maximum message size: 2 MB (DoS protection)
- Maximum peers: 125 connections
- Ping interval: 60 seconds
- Peer timeout: 180 seconds (3 pings missed)
- Wire encoding: bincode (binary, not JSON — 8× faster, 22% smaller than text)

### 6.2 DNS Seed Discovery

Nodes bootstrap peer discovery through DNS seeds:
```
seed.quanta.network
nodes.quanta.network
peers.quanta.network
```

Falls back to hardcoded bootstrap addresses if DNS is unavailable. Supports both IPv4 and IPv6 DNS resolution.

### 6.3 Block Propagation

1. Miner mines valid block
2. Broadcasts to all connected peers (bincode-encoded, zstd-compressed)
3. Peers validate (parallel signature verification + LRU cache)
4. Peers rebroadcast to their connections
5. Full network propagation target: **< 5 seconds**

### 6.4 Blockchain Sync Protocol

On first start, a new node:
1. Connects to bootstrap peers
2. Requests chain height
3. Downloads missing blocks sequentially (with parallel validation)
4. Verifies every block including Merkle root, signatures, reward amounts
5. Begins participating in block propagation and mempool relay

Initial sync speed: limited by bandwidth (~500 KB/block compressed) and CPU (225 ms/block validation).

### 6.5 Mempool Management

- **Maximum size**: 5,000 pending transactions
- **Eviction policy**: Lowest-fee transactions evicted when full
- **Priority ordering**: Highest fee-per-transaction selected first for block template
- **Expiry**: Transactions older than 24 hours automatically evicted
- **Nonce tracking**: Per-sender `DashMap<String, u64>` for atomic concurrent nonce validation (eliminates race conditions)
- **Duplicate detection**: Transaction hash deduplication before mempool insertion

---

## 7. Security Analysis

### 7.1 Threat Model

**Assumptions**:
- Adversary has bounded classical computational power
- Adversary does not control > 50% of mining power
- Adversary may operate sybil network nodes
- Adversary may possess quantum computers with up to 10^6 qubits in the future

**Explicitly NOT Protected Against**:
- 51% attacks (fundamental PoW property)
- Eclipse attacks on network-isolated nodes (mitigated by diverse bootstrapping)
- Physical key extraction from compromised devices

### 7.2 Attack Resistance

#### Double-Spend Attack
**Mitigation**:
- Recommended confirmation depth: 6 blocks for high-value transactions
- Probabilistic finality: 99.9% certainty after 6 blocks even at 40% attacker hashpower
- Checkpoint system: prevents rewriting finalized checkpoints

#### Transaction Replay Attack
**Mitigation**:
- Monotonic nonce per account (stored in persistent account state)
- 24-hour transaction expiry window
- Unique transaction hash (covers all fields including nonce and timestamp)

#### Timestamp Manipulation
**Mitigation**:
- Block timestamp must be strictly > previous block timestamp
- Block timestamp must be ≤ now + 7,200 seconds (±2 hours tolerance)
- Difficulty adjustment uses **Median-Time-Past** across last 11 blocks, not raw timestamps

#### Memory Exhaustion / DoS
**Mitigation**:
- Orphan block limit: 100 blocks maximum
- Mempool cap: 5,000 transactions
- Maximum message size: 2 MB
- Maximum transaction size: 100 KB
- Strict pre-check rejection of malformed crypto inputs

#### Sybil Attack
**Mitigation**:
- Proof-of-work requirement for block production (CPU investment required)
- Maximum 125 peer connections (bounds sybil influence)
- Peer diversity through multiple DNS seed domains

### 7.3 Post-Quantum Security Considerations

**Harvest Now, Decrypt Later (HNDL)**:
- **Threat**: Adversary records wallet-encrypted data today; decrypts on future quantum hardware
- **QUANTA Protection**: Kyber-1024 encryption provides 256-bit post-quantum security — safe until at least 2045 under conservative estimates

**Signature Forgery via Quantum Computer**:
- **Threat**: Future quantum computer forges Falcon-512 transaction signatures
- **QUANTA Protection**: Falcon-512 relies on NTRU lattice problems. No known quantum algorithm (Shor's or otherwise) solves lattice problems efficiently. Current best quantum attacks are brute-force (Grover's), reducing security from 128-bit to 64-bit — still computationally infeasible.

**Long-Range Chain Rewrite**:
- **Threat**: Adversary rewrites chain history from genesis
- **QUANTA Protection**: Hardcoded checkpoints in binary prevent rewriting finalized chain segments. Social consensus (exchanges, wallets reject alternative chains at checkpoint heights).

---

## 8. Implementation Details

### 8.1 Technology Stack

| Component | Technology | Rationale |
|---|---|---|
| **Language** | Rust 2021 | Memory-safe, zero-cost abstractions, no GC pauses |
| **Async Runtime** | Tokio 1.35 | Production-grade async I/O, multi-threaded |
| **Database** | sled 0.34 | Embedded, transactional, crash-safe key-value store |
| **P2P Networking** | Tokio TCP + custom protocol | Full control; bincode wire format |
| **REST API** | Axum 0.7 + Tower | High-performance, type-safe HTTP |
| **RPC Server** | Custom TCP JSON-RPC | Low latency, CLI-to-node communication |
| **Serialization** | bincode 1.3 (wire) + serde_json (API) | Binary internal, JSON external |
| **Parallel Compute** | Rayon 1.8 | Parallel iterator-based signature verification |
| **Compression** | zstd 0.13 | Block and P2P message compression |
| **Signature Cache** | lru 0.12 | 100k-entry LRU signature verification cache |
| **Concurrency** | parking_lot + DashMap | Lock-free concurrent nonce tracking |
| **Cryptography** | pqcrypto-falcon 0.3.0 (pinned), pqcrypto-kyber 0.8, sha3 0.10, argon2 0.5 | All post-quantum |

### 8.2 Module Architecture

```
quanta/
├── src/
│   ├── consensus/
│   │   ├── blockchain.rs   ← Chain state, block add/validate, difficulty, rewards
│   │   ├── mempool.rs      ← Mempool management, fee ordering
│   │   ├── performance.rs  ← Metrics, performance tracking
│   │   └── mod.rs
│   ├── core/
│   │   ├── block.rs        ← Block struct, mine(), hash, PoW check
│   │   ├── transaction.rs  ← Tx struct, verify(), signing contract, AccountState
│   │   ├── merkle.rs       ← SHA3-256 Merkle tree
│   │   └── mod.rs
│   ├── crypto/
│   │   ├── signatures.rs   ← FalconKeypair, verify_signature_strict(), sha3, domain sep
│   │   ├── wallet.rs       ← QuantumWallet, Kyber-1024 encryption, Argon2id
│   │   ├── hd_wallet.rs    ← BIP39 mnemonic, BIP32 derivation, Falcon keys
│   │   ├── multisig.rs     ← M-of-N Falcon-512 threshold signatures
│   │   └── mod.rs
│   ├── network/
│   │   ├── p2p.rs          ← TCP peer sessions, magic bytes, message protocol
│   │   ├── sync.rs         ← Blockchain sync, block request/response
│   │   ├── discovery.rs    ← DNS seed resolution, peer address exchange
│   │   └── mod.rs
│   ├── api/
│   │   └── mod.rs          ← Axum REST endpoints (port 7777)
│   ├── rpc/
│   │   └── mod.rs          ← TCP RPC server + client (port 7782)
│   ├── storage/
│   │   └── mod.rs          ← sled DB wrapper, block/account CRUD
│   ├── config/
│   │   └── mod.rs          ← TOML config loader, QuantaConfig struct
│   ├── bin/
│   │   └── wallet_cli.rs   ← Interactive wallet CLI binary
│   └── main.rs             ← CLI entry point (quanta binary, ~20 subcommands)
```

### 8.3 Storage Schema (sled Key-Value)

```
Blockchain:
  "blocks/{height}"        → bincode(Block)      ← full block
  "blocks/height"          → u64                 ← current chain height
  "blocks/hash/{hash}"     → u64                 ← height lookup by hash

Account State:
  "accounts/{addr}/balance"             → u64     ← spendable microunits
  "accounts/{addr}/nonce"               → u64     ← current account nonce
  "accounts/{addr}/locked_balances"    → bincode(Vec<LockedBalance>)

Indexes:
  "transactions/{tx_hash}" → (block_height, tx_index)
```

### 8.4 HD Wallet (BIP39 / BIP32 with Falcon-512)

QUANTA supports hierarchical deterministic wallets compatible with the BIP39 and BIP32 standards, adapted for Falcon-512 keys:

- **Mnemonic**: 24-word BIP39 phrase (256-bit entropy)
- **Master Seed**: PBKDF2-HMAC-SHA512 (2048 rounds) from mnemonic + optional passphrase
- **Account Generation**: HMAC-SHA512-based child key derivation per account index
- **Key Per Account**: Each account generates a fresh Falcon-512 keypair
- **Backup**: The 24-word mnemonic is the sole recovery mechanism

```bash
# Generate HD wallet with 3 accounts
quanta new_hd_wallet --accounts 3 --file hd_wallet.json
```

### 8.5 Multisig Transactions

QUANTA implements M-of-N threshold Falcon-512 multisig for treasury and high-value accounts:

- **Scheme**: M-of-N signers must provide valid Falcon-512 signatures
- **Use Cases**: Treasury (3-of-5), team wallets, smart contract escrow
- **Address**: Multisig address derived from SHA3-256 hash of all N public keys
- **Verification**: Block validator checks M independent Falcon-512 signatures

### 8.6 REST API Endpoints

```
GET  /health                     ← Node health check
GET  /blocks/:height             ← Block by height
GET  /blocks/latest              ← Latest block
GET  /transactions/:hash         ← Transaction by hash
GET  /accounts/:address/balance  ← Account balance
GET  /accounts/:address/nonce    ← Account nonce
GET  /mempool                    ← Pending transactions
POST /transactions               ← Submit new transaction
GET  /peers                      ← Connected peer list
GET  /network/stats              ← Network statistics
```

---

## 9. Governance and Upgrades

### 9.1 Current Governance Model (Off-Chain)

QUANTA v1.x uses off-chain governance:
- Kishore K (Founder) and core team propose upgrades
- Community review on GitHub Discussions and Discord
- Testnet deployment and minimum 2-week testing period
- Mainnet upgrade with clear migration path and 4-week advance notice

### 9.2 Soft Fork Process

Soft forks are backward-compatible upgrades:
1. New rule added with version bit signaling
2. 95% miner activation threshold
3. Old nodes remain valid (receive valid blocks)
4. Example: activating a new `sig_scheme` value

### 9.3 Hard Fork Policy

Hard forks will be:
- Announced with minimum 4-week notice
- Testnet-validated for minimum 2 weeks
- Semantically versioned (MAJOR.MINOR.PATCH)
- Require explicit user upgrade action

### 9.4 Future On-Chain Governance (Planned Year 2+)

- Token-weighted proposal voting
- Time-locked protocol upgrade execution
- Emergency security patches with multisig override
- Transparent on-chain treasury spending proposals

---

## 10. Roadmap

### ✅ Phase 1: Testnet Preparation (Q1 2026) — **COMPLETE**

- ✅ Core blockchain implementation (consensus, crypto, storage)
- ✅ P2P networking with DNS seed discovery
- ✅ REST API and RPC server
- ✅ HD Wallet (BIP39/BIP32) and multisig
- ✅ Docker deployment and monitoring setup
- ✅ Performance optimizations (parallel verify, LRU cache, zstd)
- ✅ Security hardening (strict pre-checks, domain separation, build determinism)
- ✅ Community onboarding materials and guides
- ✅ Block explorer (explorer.html)

### 🔄 Phase 2: Public Testnet Launch (Q2 2026)

- Public testnet launch with geographically distributed bootstrap nodes
- Real-world stress testing (target: 10,000+ transactions, 30+ days stable)
- External security audits and vulnerability assessments
- Bug bounty program launch
- Performance monitoring and optimization
- Developer onboarding and SDK documentation

**Success Criteria**: 30+ active nodes, 10,000+ TXs validated, zero critical CVEs, 30+ days uptime

### Phase 3: Security Hardening (Q3 2026)

- Address all testnet findings
- Comprehensive penetration testing
- Protocol finalization and code freeze
- Documentation completion
- Emergency response procedures

### Phase 4: Mainnet Preparation (Q4 2026)

- Code freeze and final audit
- Genesis block configuration (mainnet parameters)
- Production bootstrap node deployment (6+ regions)
- Desktop wallet (all major platforms)
- Block explorer production deployment
- Exchange integration support

### Phase 5: Mainnet Launch (Q1 2027)

- Mainnet genesis event
- Full-featured block explorer
- Production wallet release
- Continuous network monitoring
- Exchange listing coordination

**Success Criteria**: 25+ independent nodes, 95%+ uptime, consistent block times 10–15 seconds

### Phase 6: Expansion (Q2–Q4 2027)

- Light client (SPV) protocol specification
- Signature aggregation research (Falcon batch verification)
- Mobile wallet prototype
- Hardware wallet integration
- Smart contract execution layer
- Layer 2 scaling research

---

## 11. Comparison with Existing Solutions

### 11.1 vs Bitcoin

| Feature | Bitcoin | QUANTA |
|---|---|---|
| Signature Scheme | ECDSA (quantum-vulnerable) | Falcon-512 (NIST PQ standard) |
| Hash Function | SHA-256 | SHA3-256 (quantum-safe) |
| Initial Reward | 50 BTC | 100 QUA |
| Reduction Method | 50% halving every 4 years | Smooth 15% annual decay |
| Final Supply | 21M (hard cap) | ~1.5B (soft cap, 5 QUA floor) |
| Security Budget | Ends ~2140 | Perpetual (5 QUA/block minimum) |
| Fee Burning | None | 70% of fees |
| Block TPS | ~7 TPS | 120 TPS |
| Smart Contracts | No (only UTXO scripts) | Transfer + DeployContract + CallContract |
| HD Wallet | BIP32/39 (ECDSA) | BIP32/39 (Falcon-512) |
| Multisig | ECDSA multisig | Falcon-512 M-of-N |
| Quantum Risk | **HIGH** — ECDSA breakable | **NONE** — native PQ from genesis |

### 11.2 vs Ethereum

| Feature | Ethereum (PoS) | QUANTA |
|---|---|---|
| Consensus | Proof-of-Stake | Proof-of-Work (ASIC-resistant) |
| Signature | ECDSA (quantum-vulnerable) | Falcon-512 (NIST PQ standard) |
| Quantum Risk | **HIGH** — all addresses vulnerable | **NONE** |
| Issuance | ~0.5% annual | 15% → 0.8% over 20 years |
| Fee Burning | EIP-1559 (variable burn) | 70% fixed burn |
| Initial Distribution | ICO + pre-mine (~72M ETH founders) | Fair launch — zero pre-mine |
| Smart Contracts | Solidity EVM | Native TX types + future QUANTA VM |

### 11.3 vs QRL (Quantum Resistant Ledger)

| Feature | QRL | QUANTA |
|---|---|---|
| PQ Signatures | XMSS (hash-based, stateful) | Falcon-512 (stateless, NIST standard) |
| Signature Size | ~2,500 bytes | ~666 bytes (3.75× smaller) |
| Key State | Stateful (limited uses) | Stateless (unlimited reuse) |
| TPS | ~10 | 120 |
| Smart Contracts | Limited | Transfer + Deploy + Call |
| Language | Python + Go | Rust (memory-safe, high-performance) |

### 11.4 vs QANplatform

| Feature | QAN | QUANTA |
|---|---|---|
| PQ Approach | Multi-sig lattice layer | Native Falcon-512 from genesis |
| Consensus | PoS | PoW (Sybil-resistant, no stake hoarding) |
| Fair Launch | No (ICO) | Yes (100% mining, no pre-mine) |
| Open Source | Partial | Fully open source (GitHub) |

**QUANTA's unique position**: Purpose-built PQ blockchain, fully open source, fair launch, production-ready Rust implementation, 120 TPS, smart contract foundations, and the only PQ chain using NIST-standardized Falcon-512 as its native and only signature scheme.

---

## 12. Conclusion

Quantum computers represent an existential threat to every blockchain secured by elliptic curve cryptography. QUANTA is not a retrofitted patch — it is a blockchain designed from block #0 with NIST-standardized post-quantum cryptography at its core.

With Falcon-512 signatures, Kyber-1024 wallet encryption, SHA3-256 hashing, a fair launch model, sustainable tokenomics, and a production-grade Rust implementation, QUANTA provides the infrastructure for digital value to survive the quantum era.

The window to build quantum-resistant infrastructure before quantum computers arrive is narrowing. QUANTA is already here.

**Contact**: Kishore K, Founder | admin@quantachain.org | quantachain.org

---

## References

1. Ducas, L., et al. (2019). *FALCON: Fast-Fourier Lattice-based Compact Signatures over NTRU*. NIST PQC submission.
2. Avanzi, R., et al. (2022). *CRYSTALS-Kyber: Algorithm Specifications and Supporting Documentation*. NIST FIPS 203.
3. NIST (2024). *Post-Quantum Cryptography Standards: FIPS 204 (ML-DSA), FIPS 205 (SLH-DSA), FIPS 203 (ML-KEM)*.
4. Shor, P. (1994). *Algorithms for Quantum Computation: Discrete Logarithms and Factoring*. FOCS 1994.
5. Nakamoto, S. (2008). *Bitcoin: A Peer-to-Peer Electronic Cash System*.
6. NIST. (2023). *Falcon (FIPS 206 draft)*. Post-Quantum Cryptography Standardization.
7. Precedence Research (2025). *Post-Quantum Cryptography Market Size, Share & Trends Report*.

---

## Appendix A: FAQ

**Q: What makes QUANTA different from simply adding PQ signatures to Bitcoin?**

A: Retrofitting Bitcoin would require every address holder to migrate, a coordinated hard fork of thousands of nodes, and changes to UTXO structures. QUANTA builds PQ cryptography at the protocol foundation: signing format, address derivation, wallet encryption, and node verification are all PQ-native. There is no "legacy mode" that weakens security.

**Q: Isn't Falcon-512 only NIST Level 1? Should we use Level 3 or higher?**

A: Level 1 provides 128-bit classical / 64-bit quantum security. A quantum computer capable of attacking 64-bit Falcon security would require millions of error-corrected qubits — a capability estimated to be 30+ years away under the most optimistic projections. Falcon-512 offers the best throughput for a blockchain (smaller signatures, faster verification). We can upgrade via the `sig_scheme` soft-fork mechanism if needed.

**Q: What is the `tx_type` field and why does it matter for investors?**

A: QUANTA supports three transaction types: `Transfer`, `DeployContract`, and `CallContract`. This means QUANTA is not just a value-transfer layer — it is a foundation for quantum-resistant decentralized applications. Smart contracts deployed on QUANTA inherit full Falcon-512 security, unlike EVM contracts on quantum-vulnerable Ethereum.

**Q: What is the HD wallet and why does it matter?**

A: The HD wallet uses BIP39 (24-word mnemonic) and BIP32 (hierarchical deterministic derivation) with Falcon-512 keys. A single 24-word seed backs up all accounts. This provides the same user experience as any modern crypto wallet, with full post-quantum security.

**Q: Why Proof-of-Work instead of Proof-of-Stake?**

A: PoW provides Sybil resistance through physical resource expenditure, requires no initial token distribution (enabling a fair launch), and has a 15-year proven security track record. PoS systems require a pre-existing token distribution — which creates centralization. QUANTA's fair launch through PoW enables genuine decentralization from genesis.

**Q: When is mainnet?**

A: Target Q1 2027. The current phase is testnet development (Q2-Q3 2026) followed by mainnet preparation and code freeze (Q4 2026).

---

**Document Version**: 2.0  
**Last Updated**: March 2026  
**Founder**: Kishore K (admin@quantachain.org)  
**License**: CC BY 4.0  
