# Technical Specifications

This page documents the consensus protocol, block structure, transaction format, and network architecture of the Quanta blockchain as implemented in the v0.7.x alpha releases.

---

## Cryptography Stack

All cryptographic primitives used in Quanta are post-quantum safe:

| Primitive | Algorithm | NIST Level | Use |
|-----------|-----------|-----------|-----|
| Signatures | Falcon-512 | Level 1 (128-bit classical, 64-bit PQ) | Transaction signing |
| Encryption | Kyber-1024 | Level 5 (256-bit PQ) | Wallet file encryption |
| Hashing | SHA3-256 | Quantum-safe (Grover: 128-bit effective) | Block hashing, address derivation |
| KDF | Argon2id | Memory-hard | Wallet password protection |

### Falcon-512 Parameters

- Public key: **897 bytes** (exact; consensus-enforced)
- Secret key: ~1,281 bytes (never sent to network)
- Signature: up to **666 bytes** (variable-length; signed-message blob 33–698 bytes)
- Signing: ~0.8 ms per signature
- Verification: ~0.1 ms per signature (single-threaded)

### Transaction Size Impact

```
Falcon-512 transaction: ~1,713 bytes
  = 666 bytes (signature) + 897 bytes (pubkey) + ~150 bytes (payload)

ECDSA transaction: ~165 bytes
  = 10.4× larger than ECDSA
```

This is why the max block size is 2 MB with a cap of 1,200 transactions.

---

## Block Structure

```rust
Block {
    index:         u64,      // Block height (monotonically increasing)
    timestamp:     i64,      // Unix timestamp (seconds)
    transactions:  Vec<Tx>,  // Up to 1,200 transactions
    previous_hash: String,   // double-SHA3-256 of prior block
    merkle_root:   String,   // SHA3-256 Merkle root of all tx hashes
    state_root:    String,   // SHA3-256 commitment to global account state
    nonce:         u64,      // Proof-of-work nonce
    difficulty:    u32,      // Leading-zero nibble target
    hash:          String    // double-SHA3-256 of block header fields
}
```

### Genesis Block

| Parameter | Testnet | Mainnet |
|-----------|---------|---------|
| Timestamp | `1775001600` (2026-04-01 00:00:00 UTC) | `1735689600` (2026-01-01) |
| Hash | `00000012d3a2cbb7eb9579330ccdaa4f83ca9e6e016bfe6d2c8a38539cf3733b` | `1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0` |
| Difficulty | 8,304,130 | 16,777,216 |

---

## Transaction Structure

```rust
Transaction {
    sender:     String,    // "0x" + hex(SHA3-256(pubkey)[0:20])
    recipient:  String,    // Recipient address
    amount:     u64,       // Microunits (1 QUA = 1,000,000)
    fee:        u64,       // Minimum 100 microunits
    nonce:      u64,       // Monotonic account nonce (replay prevention)
    lock_time:  u64,       // Fee sniping defense (block height constraint)
    timestamp:  i64,       // Rejected if > 24 hours old
    signature:  Vec<u8>,   // Falcon-512 signed-message blob (33–698 bytes)
    public_key: Vec<u8>,   // Falcon-512 public key (must be exactly 897 bytes)
    sig_scheme: u8,        // 0 = Falcon512 (current)
    tx_type:    TransactionType,
    network_id: u32        // 0 = testnet, 1 = mainnet (replay protection)
}
```

### Transaction Types

| Type | Description |
|------|-------------|
| `Transfer` | Standard value transfer |
| `TimeLockTransfer { unlock_height }` | Funds locked until block height |
| `MultiSigTransfer { signers_required }` | M-of-N Falcon-512 threshold |

---

## Signing Contract

All transactions are signed over a canonical byte sequence:

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
signature     = Falcon-512.Sign(secret_key, signing_hash)
```

The domain prefix `"QUANTA_TX_V1:"` ensures signatures cannot be replayed in any other protocol context.

---

## Consensus: Adaptive Proof-of-Work

### Mining Algorithm

```
Block Hash = SHA3-256(SHA3-256(block_data || nonce))
Valid if:   hash starts with `difficulty` leading zero nibbles
```

Double-SHA3 eliminates length-extension attacks and provides a two-layer pre-image barrier.

### Difficulty Adjustment

- **Interval**: Every **2,016 blocks** (~16.8 hours at 30-second blocks)
- **Formula**: `new_difficulty = current × (expected_time / actual_time)` — integer math only
- **Cap**: ×1.15 maximum increase, ×0.85 maximum decrease per adjustment
- **MTP**: Uses Median-Time-Past (last 11 blocks) to prevent timestamp manipulation
- **Range**: Minimum 4, maximum 2,147,483,647

---

## Account Model

QUANTA uses an account-based model (not UTXO). Each address has:

```
AccountState {
    balance:         u64,                     // Spendable microunits
    nonce:           u64,                     // Monotonic (starts 0, first tx = 1)
    locked_balances: Vec<(amount, unlock_height)>  // Vesting + coinbase maturity
}
```

---

## Transaction Validation Rules

Each transaction is checked in order:

1. `sig_scheme == 0` (Falcon512) — unknown values rejected immediately
2. `signature` and `public_key` must be non-empty
3. `public_key.len() == 897` exactly — checked before crypto
4. `signature.len()` in `[33, 698]` — checked before crypto
5. Sender derives from public key: `"0x" || hex(SHA3-256(pubkey)[0:20])`
6. Falcon-512 signature verification
7. `balance >= amount + fee`
8. `nonce == account_nonce + 1`
9. `timestamp >= now - 86400` (24-hour expiry)
10. `fee >= 100` microunits
11. Transaction hash not already in mempool or recent blocks
12. Serialized size ≤ 100 KB

---

## Block Validation Rules

1. Proof-of-Work: hash starts with `difficulty` leading zero nibbles (double-SHA3-256)
2. Merkle root matches all transaction hashes
3. Previous hash references parent block
4. Timestamp: `prev.timestamp < block.timestamp <= now + 7200`
5. All transactions individually valid (parallel Falcon-512 verification)
6. Exactly one `COINBASE` transaction with correct reward amount
7. Treasury transaction sends to `ms69216b1d10425689704d5ae3b2a4aa17049f59b1`
8. Serialized block size ≤ 2,097,152 bytes (2 MB)
9. Transaction count ≤ 1,200
10. Difficulty matches `calculate_next_difficulty()` exactly
11. Hash matches hardcoded checkpoint at checkpoint heights

---

## Network Architecture (v0.7.x)

### Headers-First Sync (IBD)

Introduced in v0.7.0. A syncing node:
1. Downloads light headers from peers (`GetHeaders` / `Headers` messages)
2. Validates headers (PoW, chain linkage, cumulative work)
3. Finds the fork point
4. Requests full blocks only for the missing range
5. Validates each block (parallel Falcon-512 + state checks)

### Cumulative Work Peer Selection

Nodes exchange `cumulative_work` in the handshake alongside `height`. Sync targets the peer with the **highest cumulative PoW**, not the tallest chain. This prevents chain-length attacks.

### P2P Message Protocol

| Category | Messages |
|----------|---------|
| Handshake | `Version`, `VerAck` |
| Peer Discovery | `GetAddr`, `Addr` |
| Block Sync | `GetBlocks`, `Block`, `GetHeaders`, `Headers`, `GetHeight`, `Height` |
| Transactions | `NewTx`, `GetMempool`, `Mempool` |
| Maintenance | `Ping`, `Pong`, `Disconnect` |

Wire encoding: **bincode** (binary) + **zstd** compression
- 22% smaller than JSON
- 8× faster serialization
- 4× block size reduction on the wire (~500 KB per block compressed)

### Cross-Chain Replay Protection

Each transaction includes `network_id: u32`:
- Testnet: `0`
- Mainnet: `1`

Signatures are cryptographically bound to a specific network. A testnet transaction signature cannot be replayed on mainnet.

---

## Performance Characteristics

| Metric | Value |
|--------|-------|
| Target block time | 30 seconds |
| Max transactions/block | 1,200 |
| Max block size | 2 MB (compressed: ~500 KB) |
| Theoretical TPS | 120+ (parallel verification) |
| Serial block validation | ~1,800 ms |
| Parallel validation (8 cores) | ~225 ms (8× speedup) |
| Cached validation (~80% hit) | ~45 ms |

### Performance Optimizations

| Optimization | Speedup |
|---|---|
| Rayon parallel Falcon-512 verify (physical cores) | 8× |
| LRU signature cache (100k entries, ~80% hit) | 5× |
| Bloom filter mempool dedup (50k cap, 0.01% FP) | O(n) → O(1) |
| Pubkey deserialization cache (DashMap) | High on busy blocks |
| zstd block compression | 4× bandwidth reduction |
| bincode serialization | 8× faster than JSON |

---

## Storage Schema (sled Key-Value)

```
Blockchain:
  "blocks/{height}"         → bincode(Block)
  "blocks/height"           → u64 (current tip)
  "blocks/hash/{hash}"      → u64 (height lookup by hash)
  "cumulative_work"         → u128 (O(1) lookup, updated per block)

Account State:
  "accounts/{addr}/balance"          → u64
  "accounts/{addr}/nonce"            → u64
  "accounts/{addr}/locked_balances"  → bincode(Vec<LockedBalance>)

Snapshots (v0.7.1+):
  "snapshots/{height}"  → bincode(AccountStateSnapshot)
  — saved every 1,000 blocks — reorg replay starts from nearest snapshot

Indexes:
  "transactions/{tx_hash}" → (block_height, tx_index)
```

---

## Technology Stack

| Component | Library | Version |
|-----------|---------|---------|
| Language | Rust | 2021 edition |
| Async runtime | Tokio | 1.35 |
| Database | sled | 0.34 |
| REST API | Axum + Tower | 0.7 |
| Parallel compute | Rayon | 1.8 |
| Compression | zstd | 0.13 |
| Sig cache | lru | 0.12 |
| Concurrency | parking_lot + DashMap | — |
| PQ signatures | pqcrypto-falcon | 0.3.0 (pinned exact) |
| PQ encryption | pqcrypto-kyber | 0.8 |
| Hashing | sha3 | 0.10 |
| KDF | argon2 | 0.5 |
| HD Wallet | bip39 + hmac | 2.0 + 0.12 |

The `pqcrypto-falcon` dependency is pinned to `= 0.3.0` (exact). Build flags enforce `strict-float` IEEE 754 compliance and `codegen-units = 1` for byte-for-byte reproducible consensus binaries across x86_64 and ARM64.
