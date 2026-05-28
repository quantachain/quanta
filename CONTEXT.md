# QUANTA — AI Handoff Context File
> **Purpose:** Drop this file into any AI assistant to get it fully up to speed on the Quanta blockchain project in 2 minutes. Last updated: 2026-05-28.

---

## 1. What Is Quanta?

Quanta (`e:\temp\quanta`) is a **post-quantum blockchain written in Rust**, purpose-built as the financial settlement layer for autonomous AI agents. It is currently on testnet (QUA7), version `0.7.5`.

**The core vision:** AI agents on the internet need to hire each other, pay for compute, buy datasets, etc. — all without humans in the loop. Every existing chain is too slow, too expensive, or uses classical (non-quantum-safe) cryptography. Quanta is purpose-built to fix all three.

**The $1B path:** Own the AI-agent payment layer before anyone else does. The AI agent economy is projected to be a multi-trillion dollar market. Quanta provides trustless, quantum-safe, sub-cent micropayments natively on-chain.

---

## 2. Tech Stack

| Layer | Choice | Why |
|---|---|---|
| Language | Rust | Performance + safety, no GC pauses |
| Signatures | Falcon-512 (NIST PQC) | Quantum-resistant, compact |
| Consensus | **BFT (Tendermint-style) + DPoS** | Fast finality (NOT PoW — PoW was only for benchmarking) |
| Hashing | SHA3-256 | Quantum-resistant |
| Storage | Sled (embedded RocksDB-like) | No external DB dependency |
| Networking | Tokio async + Axum HTTP/JSON-RPC | Full async P2P |
| Wallet Encryption | Kyber KEM + ChaCha20-Poly1305 | Post-quantum wallet files |

**Key dependency:** `cargo` runs ONLY inside WSL (`wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo ..."`). Do NOT use Windows-native cargo.

---

## 3. Consensus Architecture (IMPORTANT — Read This Carefully)

This is **NOT Proof of Work**. The chain uses:

### DPoS (Delegated Proof of Stake)
- Validators register by sending a `Stake` transaction containing their Falcon-512 pubkey + staked QUA.
- Each epoch = 1000 blocks. Top 7 stakers form the **committee** (see `src/consensus/authorities.rs`).
- Committee rotates deterministically via `get_proposer(epoch, height, committee)`.

### BFT (Tendermint-style finality)
- Protocol: **PROPOSE → PREVOTE → PRECOMMIT → COMMIT**
- Finality requires **>2/3 committee signatures** on a block.
- Each block carries `bft_signatures: Vec<Vec<u8>>` and `bft_signers: Vec<String>`.
- Certificate verification: `src/consensus/bft.rs` → `verify_bft_certificate()`.
- Proposer logic: `src/consensus/bft_proposer.rs`.

### Why Not PoW?
PoW exists only in benchmarks (`--full-pow` flag). The live chain uses BFT+DPoS for:
- **Instant finality** (no 6-confirmation wait) — AI agents can't wait.
- **Energy efficiency** — no wasted hashing.
- **Predictable block times** — 30s target via LWMA difficulty.

---

## 4. Key Source Files

```
src/
├── core/
│   ├── transaction.rs      ← Transaction struct, AccountState, ContractState
│   ├── block.rs            ← Block struct, genesis(), calculate_hash()
│   ├── contracts.rs        ← [NEW] NativeContracts dispatcher + Escrow template
│   └── mod.rs
├── consensus/
│   ├── blockchain.rs       ← Main Blockchain struct, add_network_block(), state machine
│   ├── bft.rs              ← BFT vote types, BftVoteCollector, verify_bft_certificate()
│   ├── bft_proposer.rs     ← Proposer logic, VoteMsg, BftProtocolMsg
│   ├── authorities.rs      ← Committee computation, epoch helpers, key resolution
│   └── mempool.rs          ← Mempool with bloom filter + LRU sig cache
├── crypto/
│   └── signatures.rs       ← FalconKeypair, verify_signature_strict(), canonical_signing_hash()
├── storage/
│   └── db.rs               ← BlockchainStorage (sled-backed)
├── network/
│   ├── network.rs          ← P2P layer, bft_vote_buffer, bft_proposal_buffer
│   └── protocol.rs         ← P2P wire messages
├── api/
│   └── handlers.rs         ← REST API handlers
├── rpc/
│   └── server.rs           ← JSON-RPC server
└── main.rs                 ← Node binary entrypoint + CLI
```

---

## 5. Data Structures You Must Know

### Transaction
```rust
pub struct Transaction {
    pub sender: String,       // Falcon-512 address (hex of pubkey hash)
    pub recipient: String,
    pub amount: u64,          // microunits (1 QUA = 1_000_000)
    pub fee: u64,
    pub nonce: u64,
    pub tx_type: TransactionType,
    pub sig_scheme: SignatureScheme,  // Always Falcon512 = 0
    pub network_id: u32,      // 0 = Testnet, 1 = Mainnet
    pub payload: Vec<u8>,     // [NEW] AI metadata / data provenance blob
    // + signature, public_key, timestamp, lock_time
}
```

### TransactionType (frozen values — append only)
```
0 = Transfer
1 = TimeLockTransfer
2 = MultiSigTransfer
3 = Stake          — register BFT validator (includes falcon_pubkey)
4 = Unstake        — begin unbonding
5 = ContractDeploy — deploy native template
6 = ContractCall   — invoke deployed contract
```

### AccountState
- Manages: `accounts: HashMap<String, AccountBalance>`, `validators: HashMap<String, ValidatorInfo>`, `contracts: HashMap<String, ContractState>`
- Key methods: `credit_account()`, `debit_account()`, `credit_account_direct()`, `get_balance()`, `get_validators()`, `deploy_contract()`, `get_contract_mut()`
- `credit_account()` now **hooks into NativeContracts::execute()** for contract transactions.

---

## 6. What Has Been Built (Completed Work)

### ✅ Phase 1 — AI Payload Support
- Added `payload: Vec<u8>` to `Transaction` struct and included in signing bytes.
- All existing transaction creation sites updated (genesis, coinbase, faucets, main.rs, quanta-wallet.rs).
- Unit test: `test_ai_payload_signature` — tampering with payload invalidates signature.

### ✅ Phase 2 — Native Smart Templates (Escrow)
- **`src/core/contracts.rs`** — New file. Contains:
  - `NativeContracts` — dispatcher for `ContractDeploy` and `ContractCall` transactions.
  - `Escrow` template (`TEMPLATE_ESCROW = 1`) — trustless AI-agent hiring contract.
- **Escrow flow:**
  1. Employer deploys Escrow contract with `EscrowInitArgs { beneficiary, secret_hash }` and sends funds to the contract address.
  2. Worker AI performs task off-chain, then calls `claim` with `EscrowClaimArgs { preimage }`.
  3. Contract hashes `preimage` with SHA3-256. If it matches `secret_hash`, funds are atomically transferred to `beneficiary`.
- Contract addresses are deterministic: `format!("0xc_{}", &tx_hash[0..36])`.
- Unit test: `test_native_escrow_template` — tests full deploy + claim lifecycle.

---

## 7. Current Bugs / Compile Errors to Fix

These are known compile failures in the **integration tests** (not in the library itself):

### `tests/network_integration.rs:67`
```rust
// ERROR: Block::genesis() takes 0 args, not 1
let mut bad_genesis = Block::genesis(ChainNetwork::Mainnet);
// FIX:
let mut bad_genesis = Block::genesis();
```

### `tests/consensus_integration.rs:28,34,37`
```rust
// ERROR: mine_pending_transactions() does not exist
node_a.mine_pending_transactions(miner_a.to_string()).unwrap();
// FIX: Find the correct method in blockchain.rs — likely create_block_template() or similar
// Run: grep -n "pub fn.*mine\|pub fn.*block\|pub fn.*create" src/consensus/blockchain.rs
```

### Minor warnings (non-blocking)
- `src/consensus/blockchain.rs:519` — `let Ok(b)` should be `let Ok(_b)`.
- `src/consensus/blockchain.rs:338` — `mut account_state` should be `account_state`.

---

## 8. What Needs To Be Done Next (Phase 3)

### 3A — Fix Compile Errors
Fix the integration tests listed above so `cargo test` passes cleanly.

### 3B — BFT Finality Review
The BFT consensus engine exists (`src/consensus/bft.rs`, `src/consensus/bft_proposer.rs`) but needs to be verified end-to-end:
1. Verify `validate_bft_certificate()` in `blockchain.rs` is called on every `add_network_block()`.
2. Verify the proposer rotation (`get_proposer()` in `authorities.rs`) is correctly wired into the block production loop in `main.rs`.
3. Verify that blocks with 0 BFT signatures (solo testnet mode) are still accepted gracefully.
4. Write an integration test: deploy 3 validators → propose a block → collect >2/3 votes → verify finality.

### 3C — DPoS Validator Lifecycle Test
Write a test confirming:
- `Stake` tx registers validator with correct Falcon pubkey.
- `compute_committee()` returns the staker in the next epoch.
- `Unstake` tx starts unbonding; funds locked for `UNBONDING_EPOCHS`.

### 3D — AI Agent Demo Script
Create a demo in `src/bin/ai_agent_demo.rs` that:
1. Creates 2 keypairs (employer AI + worker AI).
2. Employer deploys an Escrow contract, locks 1 QUA.
3. Worker calls claim with the preimage.
4. Prints final balances to confirm atomic settlement.

---

## 9. How to Run & Test

```bash
# IMPORTANT: cargo must run in WSL
wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo build"
wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo test"
wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo test test_native_escrow_template -- --nocapture"
wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo check"  # Faster — just type-checks

# Run the node
wsl -- bash -lc "cd /mnt/e/temp/quanta && cargo run --release"
```

---

## 10. Strategic Context (Why This Matters)

- **AI Agent Market:** $50B+ today, projected $1T+ by 2030. Agents need to transact.
- **Competitors:** Fetch.ai (FET), Ocean Protocol, Akash — all use EVM or custom VMs with high overhead.
- **Quanta's edge:** Native Rust templates instead of a VM = nanosecond contract execution, 4GB RAM node, sub-cent fees.
- **Monetization:** Transaction fees, validator staking yields, enterprise licensing of the AI settlement layer.
- **Moat:** Quantum-safe cryptography (Falcon-512) means Quanta won't be broken by quantum computers — competitors will.
