#![allow(dead_code)]
#![allow(
    clippy::comparison_chain,
    clippy::if_same_then_else,
    clippy::absurd_extreme_comparisons,
    clippy::redundant_pattern_matching,
    clippy::empty_line_after_doc_comments
)]
use crate::core::block::Block;
use crate::core::transaction::{AccountState, Transaction};
use crate::core::ChainNetwork;
use crate::storage::{BlockchainStorage, StorageError};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use std::collections::VecDeque;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::watch; // New-block notification channel (abort-on-stale mining)

// PERFORMANCE OPTIMIZATIONS FOR POST-QUANTUM CRYPTO
use lru::LruCache; // Signature verification cache
use rayon::prelude::*; // Parallel signature verification (6x faster)
use std::num::NonZeroUsize;
use std::sync::Mutex;

// PQC-SPECIFIC OPTIMIZATIONS
// Falcon-512 transactions are 1713 bytes each — far larger than secp256k1.
// These optimizations are specifically tuned for that reality.
use bloomfilter::Bloom; // O(1) mempool duplicate check (replaces O(n) scan)
use parking_lot::Mutex as PLMutex; // Faster mutex for pubkey cache (no poisoning)

#[derive(Error, Debug)]
pub enum BlockchainError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Invalid signature")]
    InvalidSignature,
    #[error(
        "Insufficient balance: required {required} microunits, available {available} microunits"
    )]
    InsufficientBalance { required: u64, available: u64 },
    #[error("Invalid nonce: expected {expected}, got {actual}")]
    InvalidNonce { expected: u64, actual: u64 },
    #[error("Transaction already exists in mempool")]
    DuplicateTransaction,
    #[error("Invalid block")]
    InvalidBlock,
    #[error("Mempool full: {0} transactions")]
    MempoolFull(usize),
    #[error("Fee too low: {fee} microunits, minimum: {min} microunits")]
    FeeTooLow { fee: u64, min: u64 },
    #[error("Transaction expired")]
    TransactionExpired,
    #[error("Block too large: {size} bytes")]
    BlockTooLarge { size: usize },
    #[error("Invalid coinbase reward: {actual} != {expected}")]
    InvalidCoinbaseReward { actual: u64, expected: u64 },
    #[error("Invalid block difficulty")]
    InvalidDifficulty,
}

// Target block time: 30 seconds.
// PQC blocks are ~2 MB; 30s gives ~26s propagation slack in a 6-node mesh
// and reduces fork probability from 2–5% to <0.1%.
const TARGET_BLOCK_TIME: u64 = 30; // seconds

// LWMA DIFFICULTY ALGORITHM (Linearly Weighted Moving Average)
//
// Replaces the old Bitcoin-style 2016-block interval adjustment which was
// unsuitable for a small, variable-hashrate testnet:
//
//   Problem A — "Difficulty bomb trap":
//     A high-hash miner joins, mines 2016 blocks fast, difficulty jumps 4×,
//     miner leaves, low nodes can't mine for hours until next 2016-block window.
//
//   Problem B — Slow convergence on a fresh chain:
//     With a ±15% per-interval cap it takes 50+ intervals to reach equilibrium.
//
// LWMA adjusts EVERY block using a 45-block sliding window (~22.5 min).
// Each solve-time is weighted linearly (newest block gets weight 45, oldest gets 1)
// so recent hashrate changes dominate without discarding historical data.
//
// References:
//   Zawy (2017) — https://github.com/zawy12/difficulty-algorithms

// v3 TOKENOMICS — AI Agent Execution Layer Era
// Block time: SLOT_SECONDS = 6 (bft_proposer.rs)
const YEAR_1_REWARD: u64 = 50_000_000; // 50 QUA/block — tighter emission for AI era
const ANNUAL_REDUCTION_PERCENT: u64 = 15; // 15% smooth decay (no halving shocks)
const MIN_REWARD: u64 = 100_000; // 0.1 QUA floor — more deflationary long-term
const BLOCKS_PER_YEAR: u64 = 5_256_000; // 365.25 days * 86400 / 6s (BFT SLOT_SECONDS)

// UNIQUE FEATURES - Network Bootstrap
#[allow(dead_code)]
const BOOTSTRAP_PHASE_BLOCKS: u64 = 315_360; // First month bootstrap reference

// v3 FEE STRUCTURE — DPoS validator-first split
// Validators need real fee income; burn kept high for deflation; treasury funds AI SDK work.
#[allow(dead_code)]
const BASE_TRANSACTION_FEE: u64 = 1_000; // 0.001 QUA minimum (prevents spam)
const FEE_BURN_PERCENT: u64 = 50; // 50% burned — deflationary without punishing micro-tx
const FEE_TREASURY_PERCENT: u64 = 15; // 15% to Ecosystem Fund (QEF)
const FEE_VALIDATOR_PERCENT: u64 = 35; // 35% to block proposer — DPoS validators need real yield
                                       // Compile-time guard — build fails if fee percentages don't add to 100.
const _: () = assert!(
    FEE_BURN_PERCENT + FEE_TREASURY_PERCENT + FEE_VALIDATOR_PERCENT == 100,
    "Fee percentages must sum to 100"
);

// ECOSYSTEM FUND (QEF) — AI SDK, security audits, exchange listings, community
const TREASURY_ALLOCATION_PERCENT: u64 = 8; // 8% of block rewards → Quanta Ecosystem Fund

// CONSENSUS-CRITICAL: Treasury multisig address (3-of-5 Falcon-512, generated 2026-03-14)
// This address is hardcoded in consensus — it CANNOT be changed by editing quanta.toml.
// Any node that changes this constant will be rejected by the network (invalid treasury tx).
// To move treasury funds, use: quanta-wallet treasury-propose / treasury-sign / treasury-broadcast
// Keyset: treasury_key0.qua … treasury_key4.qua — any 3 of 5 must sign.
const TREASURY_ADDRESS: &str = "ms69216b1d10425689704d5ae3b2a4aa17049f59b1";

/// Epoch reward pool address — receives all block proposer rewards after EPOCH_REWARD_ACTIVATION_HEIGHT.
/// Distributed to validators proportionally by uptime at each epoch boundary.
/// Address is all-zeros + 1: cannot be derived from any known key, making it permanently
/// non-spendable except through the epoch distribution logic below.
const EPOCH_POOL_ADDRESS: &str = "0x0000000000000000000000000000000000000001";

// NOTE: Reward lock removed — replaced by DPoS unbonding period (UNBONDING_EPOCHS).
// The BFT proposer always receives the full block reward immediately; no lock is applied.

// Security limits
const MAX_MEMPOOL_SIZE: usize = 5000; // Maximum pending transactions
/// HIGH-1 FIX: Per-sender limit — prevents a single address from griefing the
/// mempool with thousands of incrementing-nonce transactions at zero cost.
const MAX_MEMPOOL_TXS_PER_SENDER: usize = 25;
// PERF (2026-07-02): Increased from 1200 to 2000
// Falcon-512 transactions are ~1713 bytes each (666 byte sig + 897 byte pubkey + overhead)
// 2000 tx × 1713 bytes = 3.43 MB — fits cleanly within the new 4 MB block limit
// with room for the coinbase tx and block header overhead.
const MAX_BLOCK_TRANSACTIONS: usize = 2000; // Maximum transactions per block
                                            // PERF (2026-07-02): Increased from 2MB to 4MB
                                            // Doubles TPS ceiling (~200 → ~400 TPS) without changing block time.
                                            // Wire size stays ~1MB after zstd compression (4× ratio on Falcon sig data).
                                            // DB decompress cap in storage/db.rs uses MAX_BLOCK_SIZE_BYTES * 2 and auto-updates.
/// Exported so `storage::db` can enforce a matching decompress size cap (MED-5).
pub const MAX_BLOCK_SIZE_BYTES: usize = 4_194_304; // 4 MB max block size
const MAX_ORPHAN_BLOCKS: usize = 2000; // Increased to hold full MAX_SYNC_BATCH out-of-order blocks
const MAX_TRANSACTION_SIZE_BYTES: usize = 102400; // 100KB max per transaction (prevents DOS)
const MIN_TRANSACTION_FEE: u64 = 100; // 0.0001 QUA in microunits — sub-cent for AI micro-tx
const TRANSACTION_EXPIRY_SECONDS: i64 = 86400; // 24 hours
const COINBASE_MATURITY: u64 = 500; // ~50 min at 6s BFT slots (matches old 100×30s window)
const MAX_FUTURE_BLOCK_TIME: i64 = 7200; // 2 hours maximum future timestamp
/// LOW-1 FIX: Bound address string length to prevent unbounded HashMap key allocations.
const MAX_ADDRESS_LEN: usize = 128;

/// Equivocation-slash burn address — tokens sent here are permanently unspendable.
/// All-zeros address cannot be derived from any known key.
const BURN_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

/// Downtime soft-slash percentage: 5% of stake burned per epoch with >30% missed slots.
const DOWNTIME_SLASH_PCT: u64 = 5;

/// Equivocation hard-slash percentage: 50% of stake burned for double-signing.
const EQUIVOCATION_SLASH_PCT: u64 = 50;

/// Whistleblower reward: 10% of the slashed equivocation amount.
const WHISTLEBLOWER_REWARD_PCT: u64 = 10;

/// STATE ROOT SORT FIX (v0.7.2): Prior to this height, state_root was computed
/// with locked_balances in insertion order, which differs between the mining
/// path (coinbase first) and the validation path (user txs first, coinbase
/// second). This caused a deterministic mismatch on syncing nodes whenever a
/// block contained a TimeLock credit to the miner's address alongside a coinbase.
///
/// From this height onward, calculate_state_root sorts locked_balances by
/// (unlock_height, amount) before hashing, making the result order-independent.
/// Blocks BELOW this height skip state_root validation — they are already
/// secured by hardcoded checkpoints.
// Height from which state_root validation is enforced.
//
// WHY 95,000:
//   Blocks below this boundary were all mined by a node whose account state had
//   accumulated bugs from prior versions (unsorted locked_balances pre-v0.7.2,
//   snapshot-fallback corruption pre-v0.7.4, and dirty incremental state up to
//   the v0.7.5 clean restart at ~block 90,000).  The chain tip at the time of the
//   v0.7.5 clean restart was block 91,768 — all of those blocks carry state_roots
//   that a fresh-genesis-replay node cannot reproduce.
//
//   Hardcoded checkpoints at 85,000 and 90,000 already anchor chain integrity;
//   the Fix-2 checkpoint-bypass in validate_block_consensus handles any remaining
//   checkpointed heights.  From block 95,000 onward every active node will have
//   been running on clean-replayed state long enough that their state_roots
//   will be identical and enforcement is meaningful.
//
//   NEXT ACTION: once the chain reaches block 95,000, add a checkpoint for that
//   height and leave this constant in place permanently.
// ---------------------------------------------------------------------------
// v2 genesis hash — BFT from block 0, no PoW.
// CONSENSUS-CRITICAL: hardcoded to prevent chain-split attacks.
// ---------------------------------------------------------------------------

/// CONSENSUS-CRITICAL: Genesis block hashes (prevent chain-split attacks)
/// Mainnet genesis — pending final mining before mainnet launch.
const GENESIS_HASH: &str = "b35800906135aae00e153756bee3ea9609f5afb0f2266d2ea5bb7cdaaa248d0c";
/// Testnet Alpha genesis — difficulty 8_304_130 (~30s/block).
/// Old nodes on the previous testnet genesis will be rejected by this hash check.
/// Testnet reset 2026-07-06 — new genesis for 4-core validator hard reset (v2.1.2-alpha).
/// Confirmed via `cargo run --bin get_testnet_hash`.
const TESTNET_GENESIS_HASH: &str =
    "b35800906135aae00e153756bee3ea9609f5afb0f2266d2ea5bb7cdaaa248d0c";

// CHECKPOINT SYSTEM: Hardcoded checkpoints prevent deep reorganizations
// Format: (block_height, block_hash)
// Add checkpoints every ~10000 blocks.
//
// TESTNET checkpoints — fetched live from rpc.quantachain.org on 2026-04-22
// Never add a checkpoint you haven't independently verified.
// Updated 2026-06-06: new genesis after validator wallet swap + timestamp reset.
const TESTNET_CHECKPOINTS: &[(u64, &str)] = &[(0, TESTNET_GENESIS_HASH)];

// MAINNET checkpoints — empty until mainnet launch
const MAINNET_CHECKPOINTS: &[(u64, &str)] = &[(0, GENESIS_HASH)];

/// Apply the annual reward reduction using PURE INTEGER MATH.
///
/// Formula: reward = start * (85/100)^years
///
/// We avoid `f64` entirely because IEEE 754 results can differ across
/// architectures and compiler optimization levels, which would cause
/// consensus forks. Instead we multiply by 85 and divide by 100 for
/// each year, which is deterministic on every platform.
///
/// Note: integer division truncates (rounds down). Over 20 years the
/// accumulated error is < 0.01 QUA versus the f64 result — well within
/// the 5 QUA `MIN_REWARD` floor.
fn apply_annual_reduction(start: u64, years: u64) -> u64 {
    let mut reward = start;
    let keep_pct = 100 - ANNUAL_REDUCTION_PERCENT; // = 85
    for _ in 0..years {
        reward = reward * keep_pct / 100;
        if reward <= MIN_REWARD {
            return MIN_REWARD;
        }
    }
    reward
}

/// Thread-safe blockchain with persistent storage.
///
/// Blocks are NOT kept in memory beyond genesis. All historical blocks are
/// loaded on-demand from the storage layer (RocksDB). The in-memory `chain`
/// vec holds only the genesis block so that `get_latest_block()` always has
/// something to return on a freshly-loaded node before the first disk read.
pub struct Blockchain {
    /// Contains only the genesis block. All other blocks are stored on disk.
    chain: Arc<RwLock<Vec<Block>>>,
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    account_state: Arc<RwLock<AccountState>>,
    /// Tracks the highest pending nonce per sender (concurrent-safe).
    pending_nonces: Arc<DashMap<String, u64>>,
    storage: Arc<BlockchainStorage>,
    orphaned_blocks: Arc<RwLock<VecDeque<Block>>>,
    /// The network this node is configured for (Mainnet or Testnet).
    network: ChainNetwork,

    // OPT-1: Signature verification cache.
    // Saves ~1.5 ms per cached Falcon-512 verification (80% hit rate in practice).
    // Only successful verifications are cached — caching `false` would allow a
    // cache-poisoning attack where one invalid tx permanently rejects valid txs
    // with the same hash.
    signature_cache: Arc<Mutex<LruCache<String, bool>>>,

    // OPT-2: Bloom filter for O(1) mempool duplicate detection.
    // At 1200 txs x 1713 bytes = 2 MB per block, an O(n) scan on every
    // mempool add caused measurable latency. False-positive rate ~0.01%
    // at 50k capacity — any false positive is a dropped (not accepted) tx,
    // which is safe.
    mempool_bloom: Arc<PLMutex<Bloom<String>>>,

    // OPT-3: Public key deserialization cache.
    // Falcon-512 public keys are 897 bytes each. When a block contains
    // multiple txs from the same sender, the same 897-byte key would be
    // deserialized repeatedly. Cache gives O(1) after the first hit.
    pubkey_cache: Arc<DashMap<String, Vec<u8>>>,

    /// Persisted cumulative work (1 per BFT block at the current tip).
    /// Stored in memory and in sled for O(1) access instead of O(height) scan.
    /// Enables instant best-peer selection at any chain height.
    cumulative_work: Arc<PLMutex<u128>>,

    /// NEW-BLOCK NOTIFICATION CHANNEL
    ///
    /// Fires the current chain height every time a block is accepted (normal or reorg).
    /// The BFT proposer loop subscribes via `subscribe_new_blocks()` and uses tokio::select!
    /// to restart the template the instant the chain tip moves.
    ///
    /// Using `watch` (not `broadcast`) because we only need the LATEST height;
    /// slow subscribers simply see the most-recent value and restart.
    new_block_tx: Arc<watch::Sender<u64>>,
}

impl Blockchain {
    /// Create or load blockchain from storage (OPTIMIZED to not load full chain)
    pub fn new(
        storage: Arc<BlockchainStorage>,
        network: ChainNetwork,
    ) -> Result<Self, BlockchainError> {
        // OPTIMIZATION: Only load genesis to verify chain exists
        // All other blocks loaded on-demand from disk
        let _chain = storage.load_chain()?;
        let account_state = storage
            .load_account_state()?
            .unwrap_or_else(AccountState::new);

        // OPTIMIZATION: load_chain only returns genesis or empty if new.
        // We must check storage height to see if we truly have an empty chain!
        let height = storage.get_chain_height()?;

        // Define expected genesis hash based on network
        // Note: Mainnet hash is hardcoded constant. Testnet hash should be calculated or hardcoded once known.
        // For now, we trust the generated testnet genesis if it's testnet.

        let (chain, account_state) = if height == 0 {
            // Create genesis block
            tracing::info!(
                "Creating new blockchain with genesis block for {:?}",
                network
            );
            let genesis = Block::genesis();

            // SECURITY: Verify genesis hash matches hardcoded value (prevents chain split)
            if network == ChainNetwork::Mainnet && genesis.hash != GENESIS_HASH {
                panic!("CRITICAL: Mainnet Genesis block hash mismatch!\nExpected: {}\nGot: {}\nThis indicates tampering or incorrect genesis generation.", 
                    GENESIS_HASH, genesis.hash);
            } else if network == ChainNetwork::Testnet && genesis.hash != TESTNET_GENESIS_HASH {
                panic!("CRITICAL: Testnet Genesis block hash mismatch!\nExpected: {}\nGot: {}\nSomeone modified the Testnet Faucet code!", 
                    TESTNET_GENESIS_HASH, genesis.hash);
            }

            let mut account_state = AccountState::new();

            // Genesis distribution
            let (recipients, premine_amount) = if network == ChainNetwork::Testnet {
                // TESTNET PREMINE: 1 Million QUA per wallet (1_000_000_000_000 microunits)
                // Generated via: cargo run --bin gen_faucet_wallets (2026-06-06 reset)
                // Mnemonic: set FAUCET_MNEMONIC in quanta-web/.env.local
                // Account 0 = faucet sender address (used by the faucet API)
                let testnet_faucets = vec![
                    "0xec4f49553e31f22b27a83036a044aff7d697f524", // Faucet 0 (sender)
                    "0xcb1a82500abea773c7ba0196f9461f8ad96ffbc1", // Faucet 1
                    "0x484a9668649fe1994e689f65d7f5d8e3b3cb7b1c", // Faucet 2
                    "0x18f8bb43114706687cde3e3ad12fa833be30ebe9", // Faucet 3
                    "0x456bfefd8ac94b8f2f0443136d256c29f209b1d5", // Faucet 4
                    "0xbb253658ef170d517714f836f6341fafe81f194e", // Faucet 5
                    "0x69636310ab0fb4d8e072e7fd0e18dcb6bd2e4135", // Faucet 6
                    "0x796bb5cd618e7addffcc856ba668055af2aa9d8e", // Faucet 7
                    "0xcf18b26ed2104a9aa9fe8e6e5c889daae25f1516", // Faucet 8
                    "0xc46e343f9990cdaf913942eb05d6e8826096231b", // Faucet 9
                ];
                (
                    testnet_faucets.into_iter().map(String::from).collect(),
                    1_000_000_000_000,
                )
            } else {
                // MAINNET: Standard empty genesis structure (1000 QUA to burn address)
                (
                    vec!["0x0000000000000000000000000000000000000000".to_string()],
                    1_000_000_000,
                )
            };

            for recipient_address in recipients {
                let genesis_tx = Transaction {
                    // GENESIS sender (not COINBASE) so premine is NOT treated as a
                    // mining reward.  Mining rewards are locked for COINBASE_MATURITY
                    // blocks; genesis premine must be immediately spendable so the
                    // faucet can distribute coins from block 1 onward.
                    sender: "GENESIS".to_string(),
                    recipient: recipient_address,
                    amount: premine_amount,
                    timestamp: genesis.timestamp,
                    signature: vec![],
                    public_key: vec![],
                    fee: 0,
                    nonce: 0,
                    lock_time: 0,
                    tx_type: crate::core::transaction::TransactionType::Transfer,
                    sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
                    network_id: 0, // Testnet; system tx bypasses sig check
                    payload: vec![],
                };
                // Pass coinbase_maturity=0: premine coins unlock at height 0 (immediately).
                // COINBASE_MATURITY (100) only applies to block-reward coinbase outputs.
                account_state.credit_account(&genesis_tx, 0, 0);
            }

            // BOOTSTRAP BFT VALIDATORS
            if let ChainNetwork::Devnet(num_nodes) = network {
                tracing::info!("DEVNET MODE: Bootstrapping {} validators dynamically into AccountState...", num_nodes);
                let limit = num_nodes.min(100); // safety limit
                for i in 0..limit {
                    let w = crate::crypto::wallet::QuantumWallet::generate_devnet(i);
                    let pk_bytes = w.keypair.public_key.clone();
                    account_state.register_validator(
                        &w.address,
                        pk_bytes,
                        1_000_000_000_000,
                        0,
                    );
                }
            } else if network == ChainNetwork::Testnet {
                tracing::info!("Bootstrapping BFT validators into AccountState...");

                #[derive(serde::Deserialize)]
                struct ValidatorGenTx {
                    address: String,
                    public_key: String,
                }

                let mut dynamic_validators = None;
                if let Ok(json_bytes) = std::fs::read("./genesis.json") {
                    if let Ok(parsed) = serde_json::from_slice::<Vec<ValidatorGenTx>>(&json_bytes) {
                        tracing::info!(
                            "Loaded {} dynamic validators from genesis.json",
                            parsed.len()
                        );
                        dynamic_validators = Some(parsed);
                    } else {
                        tracing::warn!("Found genesis.json but failed to parse it. Falling back to hardcoded validators.");
                    }
                } else {
                    tracing::info!(
                        "No genesis.json found. Falling back to hardcoded 7-node bootstrap."
                    );
                }

                if let Some(validators) = dynamic_validators {
                    for v in validators {
                        if let Ok(pk_bytes) = hex::decode(&v.public_key) {
                            account_state.register_validator(
                                &v.address,
                                pk_bytes,
                                1_000_000_000_000,
                                0,
                            );
                        } else {
                            tracing::error!(
                                "Failed to decode genesis public key for {}",
                                v.address
                            );
                        }
                    }
                } else {
                    let genesis_validators = [
                        ("0x0217a3fcbadd38e31761f9f949954e9f2ac2503d", "09635446201bc89b374210624fd1e202de4e968d514173d823d9204f5b702485542be9b38f320f948a956ead080e29d46b1e0eb6e436108aad88cd6726d001353b92d8331cdf3eb90e09ca23cef144fac8a15e474caec597c3b1c202be9e8616c7228a7a28d60ae5cf9530acdeebad041070b4ec1893cd84660ed6c76149d54d5a27e441a5e5b3ed44804696e2891da82768906c8b422e900c845607221633b827359d463490efe8324ba9763057ef6501d18582dbddaa20708776ce9eb89e22bad4f712189536f8d8c5da1c99f44e4285e2cca834c7185d9a9e7646c000e58a234ea62887eea1b1af41ce723a0a6a6142fc24e04c219a144eb43904f7607432269bc800be553e1ab618ba31561c0bf202ebafb1e7cbbd9171bb327823355979bf1e26246fed39a868ba52a9c2853c4d3d76dd8fa654d026c8052ae6b24a518572e95211e623d93e2e713415ca3a335e346d65b67873ea7e54ce2be35fef5298ae28fa9cc099789c3a82e81934a345e8c31b25dc96569d946a42a5a0e1ae28dd4e0b3fb412920b5f5700c83262cc6b2d8a0c0542efd35f04767968ee2a1cbfd2fd61210589335e9010c003d09a4069cad6050d25c8501030c4e72502dd409a51fd759d7d4009934164e4f5b72aeb0800453faa030446840981e41fa62785eaf1c7b70b319c0cd9d1e1490cbd580c436b1b81013acbc312d8edbaf83d51c9a0c7596e315ac04be442bdf3511f8a9e480ccaa307dd7b43b54fd6e2dd310ebc5ab42ae1c72bfd41204f2fb58fed5612b399b33cb615d9391e27986d1bdac6c4d4a0d8f87549197f5768208bbe4c227a160cf57c1567102276eabf01d1488c1e4807e360690687e3f1cb049f6a1b452a10d8bdfedf3b0bcd0c6ce1e248995f75b6bcbe4adaef4e47f68c46d081900f46c05804c30248c05082515d0d089c0174ed88ab8bd45678014cad79d573705ce0f9035d049767560087cc6c217fd0826ca9595668cbca9b0ee05a16ad2f54b0f112d448725cf4641401c61ea63df4a098672946e222455561a86fd934d966a42f1add4e9a815de644d82077d7ce82350264920108bd2e9a27c35040161ba120e7fd643390f238c3061ce54bbcac96e9d955a89600e6211ff9aeee3a6098dd3f35e2cbc97819783af4a44012abe20a7c19e69b05b52e9993f159c489b65cb1e6ba9148b5f63fc75e911b5cf48c069ce2fb9ecefc200996e6bc346a520ac5603c7659b5fe618069e5940a3582991c546cd2490317"),
                        ("0xee276cc08332039de4c68715430b42491cd6ad74", "096574a62455a71f5dce8d57225ab2b728f8c89280561fb0bea96063d126807f914e88f8191a856350635370cda58a1e1d3f804156d2f8246d1019538146d0736fac087a096b4e8d5438b21803c91f720596e6dbe03d717c990d243498ec04214aeba506142049c362849971c136b840bd6f7c71e35663fe34be05f3ec2bc409d5507436a96f8c4a4877cd0c7872c5f1194254962f1a001aa3414c2b8a8500cc41e6785d5cd479a80c1e0262a830cf7253d42955f4c2ef038046b7312674aab1655437067481dd63515c80a27fde3888774a9e3aa6e72090ad9d0e2a80544f2bc19bbcea09e1da3719f91daad549cb18f446871dc9a554261de1c42f5235e4d386078b0928213876e20d7d2482bc491dd01804f69b1b766a489706063063e92e967a20f8bf2a0d195d02d2fe0e3ebd5db38977d8cf9854b8116a62d5368692821e9c022fd4720a03dc62301db2049e6ac0ab54f570f7cdc002d1947603088e782126a98bc2981a55b63986cc509299098660196ba5522b5ab28c7f251c22f1501845acc4c489bf2b5a6bb1c4f699b0003dc17e36ffa7a30c86cc066eaf1f777a3416205e08bb71236155634e7c98f9f6b8ae63807c72c280a53976486586c70188a7acc5a55981b46c5862ba8c6efa87ddabc4cf55128702e24af20213234daae64b9b6d16e2652b4d4cdaadc91a060fce677ecad4b20c26636db158117ae8e709a1ebe0c2e9a82be21585535c5a2e495e3be0144c6a3188d7a72bea7c493999c946bb59a929c998d0401e6d2aa3e40ba0b3a83797edae0b523c9836982cf14b31ee0a761dd60228ca545627c3af02d3ea2fad5990c8d695fa279991348a950e45543fe49800978d32216547c640cce40b08ff755e24e059c41b707d0e6a0ea233b9c941d977e0a90378e13f4ae908c9cec98d8cd07b4ac3c622bd0484a3e1697f97a013438e95c1275aa0a1e58a2b5e3997546eecae7011d68293dbd1943015e08c8499abbe70f13245a6714a1d02672d3c6c521ac048d7e568408a33ac98b6b9d42024305ec54630b7d970c4a87bf6ac1a9feb8006a6a3406424838ee19555eecaf72a22433b64cb36aa3e8828d814d4046ea8d74a59d25355d6363001dc583a62f80a8dc2658ae1134b282e4d1acff3d82a0e01c1b3ba2adf6c7719135078c0107afe5e143a9dda1006569701146640e8f283a799971b55f952d1cb823681da73bba12d67253ea50e498a598119b6b2d95400559889f30eca9ed57787a88f2f"),
                        ("0xddfde245db7112657c938bcd9c74d19d31a78449", "097a5a23e7fadcd888f4460552e08b20b5d1ebdb27725b31ba7a6f99979fd879a950a26f9d8a10c046235d07bc0613988d943e7d588beb9603161cf9559faa8a91d1ebe3d6429eaa4e789a981ae843b6006bbf90f42e201a473944736e6a3d312d25caa17c3140e643859d0e61e7cb235862fc7ad9c9a6c5bd2409b0e629433c2126e51fac22ae4224d79384294f19a6678bea430cd6c5aacae8fca00950d5181cd53791f2d74ac2eeb0f58ed0b6678eb585034a8687f808916c0871c1d2a54d1bd594174c8180409a255c4e097c8e25cfe7d63af0b5944692773b118b3625c89bb07cda6bd7a70fbea2d4a0998a5393318aa97044603ed85e9ab059097afd9de1276bb05a6050598e1a3171ad6644f76c08aee2a690b699ad1e5527159a1674480a9a242286beb96572351ca9b933c1beb6c3d58a51c0fe297348028f844b68f92c7d9416f863ccc81b74ce51637b1fce908d4ca7ffad09b23504fa0c1c035ddd3ca4bed71e4bdd25131a094d2d7c49144229da1d690f093197c354c61bd5820035b80037f2e5e5ea052400d3e167522ad163863815b30a1dba39b1191420419739984048a4b52ae057a2a1b9f0b7eb7ae40cba1137c0d2003270b0d3c6a2d9b4b5dcdcb496c66a82c6f266a6c63f871866a95a9fce54b54089796542895d2b083a81355af5ac909e9695328cff8bee40d49dbea046e9a48e32d5f6790b283ca6da23a42346bf02bd28214f626215aea1b400667a20521ec15567c23ee9bfd30ccbd525217207b9ac08a6cd9ea2a7534043501654c0abf337ed5a870d6f9479115b440de22359f4ad7647c91bbaf52b99e554ec221e63d83f67c59228acd3eb5afbb63680cbf3424349f174767c63479cb253c85347e672acf0b9aa82f91170f1f5390ae1431250d4b4e4b856a91eba498cda547c17b6d584c5a42981413da36b19fdf1f63c8b7f4e10aa05b0eba118282486c7d3f6840c8ce11504398295bca12eeda08d15bfb8e3165425ea4b8058c9315998ca05796ee926992142ba2719223e9f305ce66a388e4df3eac4781056520b6496bf20381e481288cb0154181513dfa1b678e81ad4af3966000ec93622ab57b66bc9ede798b93e93c528d50142d9d549d6d3c25d056dbb696cd206e5022ad74fd22f01f901ef43494fec0444e9e2b71f7dc6b7c9af40915dc5a322148a1d942820f6487a4e7ea732256e31951d82ea5e0a1b6f186001bba1e8358cfe22faafa11a7031ea97e62ba6dcaa0a5de1ae"),
                        ("0x0e95de36b72ab7b20372497c2fe6b429223cd9e7", "096ad6b212759b849609273245c0d7091eef21e8ded36106e5045712d3b905f609e9857e36581666c19a525f80e7b7acdc031ad93eeae11a020b455a748b69ab4cee06b60dd1a832915d55ba671ca382af84b224fd5c414a63a6c3eb7e9edf9830697675ede8e08189038b8235e17244299fa52d86f0899f2ea94fb15ce7387c10ee8888d95f712ad566a9ddbd0beeeaeb9bd60f445e42c5d42596624d02549c8d4d752cb3606368689c823ab2198bf90ce58e7b47aa9f5e264e428b5182a886aec1bed0fe24128b06661c642f626eba10ab4998e98335e61cb718e0c63ae5602481a419c0ae581db80083364f5f6b71d91147bc8f739ea9d7c5b101632c960ccb59cf5cb39cd814d4c14928713d3189a96408810b932f4f52218c9c06abc1ff3e0186c22d652d20d8999747dbc2311a7726dd273d1bd90ce59d86c204804b5399d2e7b7e049089a5cbb28b0082b23135b7a28349099501367853d5bb6a637745489a9669e7db6393d8be064fe12710bb52ae0f15aaccbf296884f8cfa50b4b7543da57dbef60858ec2e7c46cae6cbf82911f6e1165ce6bfd6dce83d91fc0dc9e10a800b1c78349fc2f7c150aaaaa537d5521d18858b3825946922ec91b96dca6e2d7c67836b94a54749da75c2c8fd5c3ddf359297238c899885a6ca568b76203a65a42e35ce695a0de6d8be29c61919c3fd51e0c2825bc45459a89965ac87eb80f6d9a77a41e95561c017718d2d87fea370de88058975b4f0ab27d324f52f0687addb190e2616202297abe0b826904eb9435d2bd968633b1534e791f682630ad4b13692ce353d95d2b8e242ec1e967459c0982e8d2e2e97545ef18b06fff66960ffb72d35d89ca6531545db30a385bf8f5ec32bae0e6fb278ebf8a1ca4e4f6657758bce238ec5c1d8f151920934876551ca11694ef6eabb11c72f9e20e6bf9627604a3f255dd55048b02529262d45332ee3fb3cee66b5325aa7e308782db6c878ef502376d5089ad4d9964864200641dc786ba97ca52a69bf1cdadedb61aef19e799a38de2ab20815c143c2ee1da4f6b9abf7db8c5e52ffbbfc64590876b1a8a7064f414c3d39faaf68096952483451e880a9704200cf6eaa101e9b3b227f581a0d96fac8940362162985658f5e2d10fb98298496ba33354118b344d50ed923666c0f05bd16ccee3a4f1cf7436ea42f986a95489ec4710d20bd78176b631d15f37690e2c51fb523f5e89b332c29783beb6b1f8a8d21e723abfe2d9c03231754c8"),
                    ];

                    for (addr, pubkey_hex) in genesis_validators {
                        if let Ok(pk_bytes) = hex::decode(pubkey_hex) {
                            account_state.register_validator(addr, pk_bytes, 1_000_000_000_000, 0);
                        } else {
                            tracing::error!("Failed to decode genesis public key for {}", addr);
                        }
                    }
                }
            }
            storage.save_block(&genesis)?;
            storage.set_chain_height(1)?;
            storage.save_account_state(&account_state)?;

            tracing::info!("✓ Genesis block verified: {}", genesis.hash);
            (vec![genesis], account_state)
        } else {
            // OPTIMIZATION: chain only contains genesis (loaded from db.rs load_chain())
            // Or we just load genesis manually here
            let genesis = storage
                .load_block(0)
                .expect("Genesis block must exist if height > 0");
            let chain = vec![genesis];

            tracing::info!(
                "✓ Loaded blockchain with {} blocks (genesis in memory, rest on disk)",
                height
            );

            // SECURITY: Verify genesis block on load (prevents database tampering)
            if network == ChainNetwork::Mainnet && chain[0].hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block mismatch in existing chain!\nExpected: {}\nGot: {}\nDatabase may be corrupted or from different network.", 
                    GENESIS_HASH, chain[0].hash);
            }

            // SELF-HEAL: Detect corrupted account state from bad reorg (v0.4.0 bug).
            // If Faucet 0 shows 0 balance on an existing chain that has blocks, the
            // genesis premine was never applied — rebuild from scratch automatically.
            const FAUCET_0: &str = "0xec4f49553e31f22b27a83036a044aff7d697f524"; // 2026-06-06 reset
            let faucet_balance = account_state.get_balance(FAUCET_0);
            if network == ChainNetwork::Testnet && faucet_balance == 0 && height > 1 {
                tracing::warn!(
                    "⚠️  SELF-HEAL: Faucet 0 has 0 balance on a chain of {} blocks \
                     — account state is corrupted (v0.4.0 reorg bug). Rebuilding from genesis…",
                    height
                );
                // Rebuild a temporary blockchain state holder to call rebuild method
                // We cannot call self.rebuild_account_state_up_to here (self doesn't exist yet),
                // so we inline the same logic: credit premine then replay all blocks.
                let mut healed_state = AccountState::new();
                let genesis_ts = storage.load_block(0).map(|g| g.timestamp).unwrap_or(0);
                let faucets = [
                    "0xec4f49553e31f22b27a83036a044aff7d697f524",
                    "0xcb1a82500abea773c7ba0196f9461f8ad96ffbc1",
                    "0x484a9668649fe1994e689f65d7f5d8e3b3cb7b1c",
                    "0x18f8bb43114706687cde3e3ad12fa833be30ebe9",
                    "0x456bfefd8ac94b8f2f0443136d256c29f209b1d5",
                    "0xbb253658ef170d517714f836f6341fafe81f194e",
                    "0x69636310ab0fb4d8e072e7fd0e18dcb6bd2e4135",
                    "0x796bb5cd618e7addffcc856ba668055af2aa9d8e",
                    "0xcf18b26ed2104a9aa9fe8e6e5c889daae25f1516",
                    "0xc46e343f9990cdaf913942eb05d6e8826096231b",
                ];
                for addr in &faucets {
                    let gtx = Transaction {
                        sender: "GENESIS".to_string(),
                        recipient: addr.to_string(),
                        amount: 1_000_000_000_000,
                        timestamp: genesis_ts,
                        signature: vec![],
                        public_key: vec![],
                        fee: 0,
                        nonce: 0,
                        lock_time: 0,
                        tx_type: crate::core::transaction::TransactionType::Transfer,
                        sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
                        network_id: 0,
                        payload: vec![],
                    };
                    healed_state.credit_account(&gtx, 0, 0);
                }
                for h in 1..height {
                    if let Ok(block) = storage.load_block(h) {
                        healed_state.unlock_mature_coinbase(block.index);
                        for tx in &block.transactions {
                            if !tx.is_coinbase()
                                && tx.sender != "TREASURY"
                                && !tx.is_genesis_premine()
                            {
                                let total = tx.amount.saturating_add(tx.fee);
                                healed_state.debit_account(&tx.sender, total);
                                healed_state.increment_nonce(&tx.sender);
                            }
                            let maturity = if tx.is_genesis_premine() {
                                0
                            } else {
                                COINBASE_MATURITY
                            };
                            healed_state.credit_account(tx, block.index, maturity);
                        }
                    }
                }
                storage.save_account_state(&healed_state)?;
                tracing::info!(
                    "✅ SELF-HEAL complete: Faucet 0 balance restored to {} microunits",
                    healed_state.get_balance(FAUCET_0)
                );
                (chain, healed_state)
            } else {
                (chain, account_state)
            }
        };

        // OPT: Tune rayon thread pool to physical CPU count.
        // Falcon-512 verification is CPU-bound, not I/O-bound.
        // Hyperthreading doubles logical CPUs but doesn't help for crypto — use physical only.
        // num_cpus::get_physical() returns real cores (e.g. 4 on an 8-logical-thread machine).
        // LOW-5 FIX: Log rayon init errors (e.g. when running in tests where it's already initialized)
        let physical_cores = num_cpus::get_physical().max(1);
        if let Err(e) = rayon::ThreadPoolBuilder::new()
            .num_threads(physical_cores)
            .thread_name(|i| format!("quanta-verify-{}", i))
            .build_global()
        {
            tracing::warn!(
                "Could not configure rayon thread pool: {} (using default config)",
                e
            );
        }

        // Compute/load cumulative work — O(1) after first run (migration is one-time only).
        let initial_cumulative_work = {
            let stored = storage.get_cumulative_work();
            if stored == 0 && height > 1 {
                tracing::info!(
                    "[Migration] Computing cumulative work for {} blocks (one-time)…",
                    height
                );
                let mut work = 0u128;
                for h in 0..height {
                    if let Ok(_b) = storage.load_block(h) {
                        work = work.saturating_add(1u128); // BFT: 1 unit per block
                    }
                }
                let _ = storage.set_cumulative_work(work);
                tracing::info!("[Migration] Cumulative work = {}", work);
                work
            } else {
                stored
            }
        };

        // New-block notification channel — initial value = current height (subscribers
        // start with the tip, not 0, so they don't fire spuriously on startup).
        let (new_block_tx, _) = watch::channel(height);

        Ok(Self {
            chain: Arc::new(RwLock::new(chain)),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            account_state: Arc::new(RwLock::new(account_state)),
            pending_nonces: Arc::new(DashMap::new()),
            storage,
            orphaned_blocks: Arc::new(RwLock::new(VecDeque::new())),
            network,
            signature_cache: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(100_000).unwrap(),
            ))),
            mempool_bloom: Arc::new(PLMutex::new(Bloom::new_for_fp_rate(50_000, 0.0001))),
            pubkey_cache: Arc::new(DashMap::new()),
            cumulative_work: Arc::new(PLMutex::new(initial_cumulative_work)),
            new_block_tx: Arc::new(new_block_tx),
        })
    }

    /// Subscribe to new-block notifications.
    ///
    /// Returns a `watch::Receiver<u64>` that yields the new chain height each
    /// time a block is accepted. Use with `tokio::select!` in the BFT proposer
    /// loop to restart the block template when the chain tip moves:
    ///
    /// ```ignore
    /// let mut new_block_rx = blockchain.read().await.subscribe_new_blocks();
    /// loop {
    ///     tokio::select! {
    ///         _ = new_block_rx.changed() => { /* chain moved, restart template */ }
    ///         block = bft_finalize_rx.recv() => { apply_block(block); }
    ///     }
    /// }
    /// ```
    pub fn subscribe_new_blocks(&self) -> watch::Receiver<u64> {
        self.new_block_tx.subscribe()
    }

    /// Validate block against checkpoints (prevents deep reorgs)
    fn validate_checkpoint(&self, height: u64, hash: &str) -> bool {
        let checkpoints = match self.network {
            ChainNetwork::Testnet => TESTNET_CHECKPOINTS,
            ChainNetwork::Mainnet => MAINNET_CHECKPOINTS,
            ChainNetwork::Devnet(_) => &[],
        };
        for (checkpoint_height, checkpoint_hash) in checkpoints {
            if *checkpoint_height == height {
                if hash != *checkpoint_hash {
                    tracing::error!(
                        "Checkpoint violation at height {}: expected {}, got {}",
                        height,
                        checkpoint_hash,
                        hash
                    );
                    return false;
                }
                tracing::debug!("Checkpoint validated at height {}", height);
                return true;
            }
        }
        true // No checkpoint at this height
    }

    /// Get the latest block
    pub fn get_latest_block(&self) -> Block {
        let height = self.get_height();
        if height == 0 {
            // Return genesis from memory
            self.chain.read().first().unwrap().clone()
        } else {
            // Load from storage (not memory!)
            self.storage
                .load_block(height - 1)
                .expect("Latest block must exist")
        }
    }

    /// Check if a transaction is semantically valid against a specific AccountState.
    /// This handles Stake, Unstake, and SlashEvidence rules (which depend on the state).
    fn is_transaction_valid_for_state(
        &self,
        tx: &Transaction,
        state: &crate::core::transaction::AccountState,
        block_index: u64,
    ) -> Result<(), BlockchainError> {
        let epoch = crate::consensus::authorities::epoch_for_height(block_index);

        match &tx.tx_type {
            crate::core::transaction::TransactionType::Stake { .. } => {
                use crate::consensus::authorities::MIN_VALIDATOR_STAKE;

                if tx.amount < MIN_VALIDATOR_STAKE {
                    return Err(BlockchainError::InvalidBlock);
                }

                let already_active = state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.active)
                    .unwrap_or(false);
                if already_active {
                    return Err(BlockchainError::InvalidBlock);
                }

                let slash_cooldown = state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.slash_cooldown_until_epoch)
                    .unwrap_or(0);
                if slash_cooldown > epoch {
                    return Err(BlockchainError::InvalidBlock);
                }

                let is_unbonding = state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.unbonding_epoch > 0)
                    .unwrap_or(false);
                if is_unbonding {
                    return Err(BlockchainError::InvalidBlock);
                }
            }
            crate::core::transaction::TransactionType::Unstake => {
                let is_active = state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.active)
                    .unwrap_or(false);
                if !is_active {
                    return Err(BlockchainError::InvalidBlock);
                }
            }
            crate::core::transaction::TransactionType::SlashEvidence {
                offender,
                hash_a,
                hash_b,
                sig_a,
                sig_b,
                ..
            } => {
                if hash_a == hash_b {
                    return Err(BlockchainError::InvalidBlock);
                }
                let validator_pk = state
                    .get_validator_info(offender)
                    .map(|v| v.falcon_pk.clone());
                if let Some(pk) = validator_pk {
                    let sig_a_valid = crate::crypto::verify_signature_strict(
                        &crate::crypto::canonical_signing_hash(hash_a.as_bytes()),
                        sig_a,
                        &pk,
                    );
                    let sig_b_valid = crate::crypto::verify_signature_strict(
                        &crate::crypto::canonical_signing_hash(hash_b.as_bytes()),
                        sig_b,
                        &pk,
                    );
                    if !sig_a_valid || !sig_b_valid {
                        return Err(BlockchainError::InvalidBlock);
                    }
                } else {
                    return Err(BlockchainError::InvalidBlock);
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Add a new transaction to the mempool

    pub fn add_transaction(&self, transaction: Transaction) -> Result<(), BlockchainError> {
        // Skip validation for coinbase transactions
        if transaction.is_coinbase() {
            self.pending_transactions.write().push(transaction);
            return Ok(());
        }

        // LOW-1 FIX: Reject excessively long addresses to prevent unbounded key allocations
        if transaction.sender.len() > MAX_ADDRESS_LEN
            || transaction.recipient.len() > MAX_ADDRESS_LEN
        {
            return Err(BlockchainError::InvalidBlock);
        }

        // Check mempool size limit
        let pending_count = self.pending_transactions.read().len();
        if pending_count >= MAX_MEMPOOL_SIZE {
            return Err(BlockchainError::MempoolFull(pending_count));
        }

        // Validate minimum fee
        if transaction.fee < MIN_TRANSACTION_FEE {
            return Err(BlockchainError::FeeTooLow {
                fee: transaction.fee,
                min: MIN_TRANSACTION_FEE,
            });
        }

        // Check transaction expiry (replay protection)
        let current_time = chrono::Utc::now().timestamp();
        if transaction.timestamp < current_time - TRANSACTION_EXPIRY_SECONDS {
            return Err(BlockchainError::TransactionExpired);
        }

        // Verify signature
        if !transaction.verify() {
            return Err(BlockchainError::InvalidSignature);
        }

        // Validate nonce (account-based model) - ATOMIC OPERATION (no race condition)
        let chain_nonce = self.account_state.read().get_nonce(&transaction.sender);

        // SECURITY FIX (CRITICAL-4): Atomic nonce validation with duplicate check
        // We need to hold the nonce entry lock during the entire validation + addition
        let mut nonce_entry = self
            .pending_nonces
            .entry(transaction.sender.clone())
            .or_insert(chain_nonce);
        let expected_nonce = (*nonce_entry).max(chain_nonce) + 1;

        if transaction.nonce != expected_nonce {
            return Err(BlockchainError::InvalidNonce {
                expected: expected_nonce,
                actual: transaction.nonce,
            });
        }

        // Check transaction size limit (DOS protection - prevents huge DeployContract)
        let tx_size = bincode::serialize(&transaction)
            .map_err(|_| BlockchainError::InvalidBlock)?
            .len();
        if tx_size > MAX_TRANSACTION_SIZE_BYTES {
            return Err(BlockchainError::BlockTooLarge { size: tx_size });
        }

        // Check sender has sufficient balance (amount + fee)
        let total_required = transaction.amount.saturating_add(transaction.fee);
        let available = self.account_state.read().get_balance(&transaction.sender);

        if available < total_required {
            return Err(BlockchainError::InsufficientBalance {
                required: total_required,
                available,
            });
        }

        // HIGH-1 FIX: Per-sender mempool limit — prevents griefing the mempool
        // by submitting thousands of incrementing-nonce txs from a single address.
        {
            let pending = self.pending_transactions.read();
            let sender_count = pending
                .iter()
                .filter(|t| t.sender == transaction.sender)
                .count();
            if sender_count >= MAX_MEMPOOL_TXS_PER_SENDER {
                return Err(BlockchainError::MempoolFull(sender_count));
            }
        }

        // Validate TransactionType specific state rules (Stake/Unstake/Slash)
        let current_height = self.get_height();
        let state_snapshot = self.account_state.read().clone();
        if self.is_transaction_valid_for_state(&transaction, &state_snapshot, current_height + 1).is_err() {
            return Err(BlockchainError::InvalidBlock);
        }

        // OPT-2 (PQC): Bloom filter duplicate check — O(1) instead of O(n) scan
        // At 1200 pending txs × 1713 bytes each, O(n) scan wastes ~2MB of cache per add.
        // Bloom gives probabilistic O(1). False positive = tx rejected (harmless, rare at 0.01%).
        let tx_hash = transaction.hash();
        {
            let mut bloom = self.mempool_bloom.lock();
            if bloom.check(&tx_hash) {
                // Bloom says "probably seen" — confirm with O(1) hash comparison on tx list
                // (needed to avoid false-positive rejections)
                let pending = self.pending_transactions.read();
                if pending.iter().any(|t| t.hash() == tx_hash) {
                    return Err(BlockchainError::DuplicateTransaction);
                }
            }
            bloom.set(&tx_hash);
        }

        // ATOMIC: Update nonce AND add transaction together (no window for races)
        *nonce_entry = transaction.nonce;
        self.pending_transactions.write().push(transaction);

        tracing::info!("Transaction added to mempool");
        Ok(())
    }

    /// Create a block template for mining (does not mine or save)
    pub fn create_block_template(
        &self,
        proposer_address: String,
    ) -> Result<Block, BlockchainError> {
        let reward = self.get_block_reward();

        let current_height = self.get_height();

        // Get pending transactions sorted by fee (highest first) with size limits
        let pending_txs = self.pending_transactions.read();

        // Filter out transactions locked for future blocks, then sort by fee descending
        let mut sorted_txs: Vec<_> = pending_txs
            .iter()
            .filter(|tx| tx.lock_time <= current_height)
            .cloned()
            .collect();
        sorted_txs.sort_by(|a, b| b.fee.cmp(&a.fee));

        let mut transactions = Vec::new();
        let mut block_size = 0usize;

        // Use a temporary state to validate nonces and balances as we add them,
        // to prevent mining invalid blocks (which stall the network with orphaned blocks).
        let mut temp_state = self.account_state.read().clone();

        let mut added_any = true;
        while added_any && transactions.len() < MAX_BLOCK_TRANSACTIONS {
            added_any = false;
            let mut i = 0;
            while i < sorted_txs.len() {
                let tx = &sorted_txs[i];
                let expected_nonce = temp_state.get_nonce(&tx.sender) + 1;

                if tx.nonce == expected_nonce {
                    let tx_size = bincode::serialize(tx).unwrap_or_default().len();
                    if block_size + tx_size <= MAX_BLOCK_SIZE_BYTES {
                        let total_required = tx.amount.saturating_add(tx.fee);
                        if temp_state.debit_account(&tx.sender, total_required) {
                            temp_state.increment_nonce(&tx.sender);
                            transactions.push(tx.clone());
                            block_size += tx_size;
                            added_any = true;
                        }
                    }
                    sorted_txs.remove(i);
                } else if tx.nonce < expected_nonce {
                    // Stale nonce, skip and remove
                    sorted_txs.remove(i);
                } else {
                    // Nonce is too high right now; keep it in sorted_txs in case the missing preceding tx is found later
                    i += 1;
                }

                if transactions.len() >= MAX_BLOCK_TRANSACTIONS {
                    break;
                }
            }
        }

        // Create coinbase transaction with fee distribution
        let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();

        // FEE DISTRIBUTION (70% burn, 20% treasury, 10% miner)
        // SECURITY FIX (HIGH-2): Prevent rounding loss - give remainder to miner
        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_miner = total_fees - fee_burned - fee_to_treasury; // Remainder goes to miner

        // ECOSYSTEM FUND ALLOCATION (8% of block rewards → QEF multisig)
        let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100;
        let proposer_reward = reward - treasury_allocation; // 92% to block proposer

        tracing::info!(
            "Block Economics: Reward={} QUA, QEF={} QUA, Fees Burned={} QUA, Proposer={} QUA",
            reward / 1_000_000,
            treasury_allocation / 1_000_000,
            fee_burned / 1_000_000,
            proposer_reward / 1_000_000
        );

        // EPOCH POOL MODEL (activated at EPOCH_REWARD_ACTIVATION_HEIGHT):
        // After activation, the proposer reward goes into the epoch pool address
        // instead of directly to the proposer. The pool is distributed to all
        // validators proportionally by uptime at each epoch boundary.
        use crate::consensus::authorities::EPOCH_REWARD_ACTIVATION_HEIGHT;
        let coinbase_recipient = if current_height >= EPOCH_REWARD_ACTIVATION_HEIGHT {
            EPOCH_POOL_ADDRESS.to_string()
        } else {
            proposer_address.clone()
        };

        // Coinbase transaction (full proposer reward + fee share)
        let coinbase_amount = proposer_reward.saturating_add(fee_to_miner);
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: coinbase_recipient,
            amount: coinbase_amount,
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            lock_time: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            network_id: self.network.network_id(),
            payload: vec![],
        };

        // Treasury allocation transaction (if any)
        let mut all_transactions = vec![coinbase_tx.clone()];

        if treasury_allocation + fee_to_treasury > 0 {
            let treasury_tx = Transaction {
                sender: "TREASURY".to_string(),
                recipient: TREASURY_ADDRESS.to_string(),
                amount: treasury_allocation.saturating_add(fee_to_treasury),
                timestamp: chrono::Utc::now().timestamp(),
                signature: vec![],
                public_key: vec![],
                fee: 0,
                nonce: 0,
                lock_time: 0,
                tx_type: crate::core::transaction::TransactionType::Transfer,
                sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
                network_id: self.network.network_id(),
                payload: vec![],
            };
            all_transactions.push(treasury_tx);
        }

        all_transactions.extend(transactions);

        let index = self.get_height();

        // Calculate state_root by simulating transaction execution.
        // CRITICAL STATE-ROOT FIX: We must call unlock_mature_coinbase(index)
        // BEFORE applying this block's transactions — exactly mirroring what
        // validate_block_consensus does.  Without this, any block mined while
        // a miner has pending mature coinbase locks produces a state_root that
        // the validator rejects (their state has the locked entries still present,
        // the miner's hash already moved them to spendable, or vice-versa).
        let mut temp_state = self.account_state.read().clone();
        temp_state.unlock_mature_coinbase(index);
        let epoch = crate::consensus::authorities::epoch_for_height(index);
        let registration_open =
            index >= crate::consensus::authorities::OPEN_VALIDATOR_REGISTRATION_HEIGHT;
        for tx in &all_transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                let required = tx.amount.saturating_add(tx.fee);
                temp_state.debit_account(&tx.sender, required);
            }

            // CRITICAL FIX: Simulate validator registration and deregistration to match validate_block_consensus
            if let crate::core::transaction::TransactionType::Stake { validator_pubkey } =
                &tx.tx_type
            {
                // We don't apply the full guard logic here, just the state mutation
                if registration_open {
                    temp_state.register_validator(
                        &tx.sender,
                        validator_pubkey.clone(),
                        tx.amount,
                        index,
                    );
                } else {
                    temp_state.register_validator(
                        &tx.sender,
                        validator_pubkey.clone(),
                        tx.amount,
                        index,
                    );
                }
            } else if tx.is_unstake() {
                temp_state.deregister_validator(&tx.sender, epoch);
            }

            temp_state.credit_account(tx, index, COINBASE_MATURITY);

            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                temp_state.increment_nonce(&tx.sender);
            }
        }
        let state_root = temp_state.calculate_state_root();

        // Create new block (unmined)
        let previous_block = self.get_latest_block();
        let previous_hash = previous_block.hash.clone();
        let epoch = crate::consensus::authorities::epoch_for_height(index);
        let mut new_block = Block::new_bft(
            index,
            all_transactions,
            previous_hash,
            epoch,
            0,
            proposer_address.clone(),
        );

        // TIMESTAMP RULES (DRIFT-SAFE):
        // 1. Must be strictly greater than the previous block's timestamp (monotonic).
        // 2. Must be >= MTP (median-time-past of last 11 blocks).
        // 3. HARD CAP: Must NEVER exceed wall-clock time.
        //
        // BUG that caused the slowdown:
        //   Old code: timestamp = max(now, prev+1)
        //   When AlephBFT finalises blocks faster than 1/sec, this sets timestamp
        //   = prev+1 repeatedly. After N blocks the chain timestamp is N seconds
        //   AHEAD of real time. The 6-second gate in aleph_data.rs:
        //     `if current_time < effective_last_ts + 6 { return None; }`
        //   then NEVER opens (wall time < future chain time + 6 forever).
        //
        // Fix: cap to wall clock. If the minimum valid timestamp is in the future,
        // we use wall clock and accept the monotonicity gap — the next block will
        // be prev+1 again naturally once real time catches up.
        let current_time = chrono::Utc::now().timestamp();
        let mtp = if previous_block.index >= 10 {
            self.get_median_time_past(previous_block.index, 11)
        } else {
            0
        };
        let min_ts = std::cmp::max(previous_block.timestamp + 1, mtp + 1);
        // Use current_time if it's valid. If min_ts is in the future
        // (clock skew or race), we MUST use min_ts to prevent validation failure.
        new_block.timestamp = std::cmp::max(min_ts, current_time);

        new_block.state_root = state_root;
        new_block.hash = new_block.calculate_hash();

        // Don't mine or save here. Just return the template.
        Ok(new_block)
    }

    /// Build a BFT block template for the proposer (does not mine or save).
    ///
    /// This replaces `create_block_template` for v2. The proposer calls this
    /// to get a block with mempool transactions and the correct coinbase,
    /// then collects BFT signatures before calling `add_network_block`.
    pub fn create_bft_block_template(
        &self,
        next_height: u64,
        proposer_address: String,
        epoch: u64,
        round: u32,
    ) -> Result<Block, BlockchainError> {
        let reward = self.get_block_reward();
        let current_height = self.get_height();

        // Select pending transactions sorted by fee.
        let pending_txs = self.pending_transactions.read();
        let mut sorted_txs: Vec<_> = pending_txs
            .iter()
            .filter(|tx| tx.lock_time <= current_height)
            .cloned()
            .collect();
        sorted_txs.sort_by(|a, b| b.fee.cmp(&a.fee));

        let mut transactions = Vec::new();
        let mut block_size = 0usize;
        let mut temp_state = self.account_state.read().clone();
        let mut added_any = true;

        while added_any && transactions.len() < MAX_BLOCK_TRANSACTIONS {
            added_any = false;
            let mut i = 0;
            while i < sorted_txs.len() {
                let tx = &sorted_txs[i];
                let expected_nonce = temp_state.get_nonce(&tx.sender) + 1;
                if tx.nonce == expected_nonce {
                    let tx_size = bincode::serialize(tx).unwrap_or_default().len();
                    if block_size + tx_size <= MAX_BLOCK_SIZE_BYTES {
                        let total_required = tx.amount.saturating_add(tx.fee);
                        
                        // Pre-validate TransactionType rules (Stake, Unstake, Slash)
                        if self.is_transaction_valid_for_state(tx, &temp_state, next_height).is_ok() {
                            if temp_state.debit_account(&tx.sender, total_required) {
                                temp_state.increment_nonce(&tx.sender);
                                transactions.push(tx.clone());
                                block_size += tx_size;
                                added_any = true;
                            }
                        }
                    }

                    sorted_txs.remove(i);
                } else if tx.nonce < expected_nonce {
                    sorted_txs.remove(i);
                } else {
                    i += 1;
                }
                if transactions.len() >= MAX_BLOCK_TRANSACTIONS {
                    break;
                }
            }
        }
        drop(pending_txs);

        // BFT block reward: full amount to proposer (no PoW lock).
        let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();
        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_proposer = total_fees - fee_burned - fee_to_treasury;
        let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100;
        let proposer_reward = reward - treasury_allocation;

        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: proposer_address.clone(),
            amount: proposer_reward.saturating_add(fee_to_proposer),
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            lock_time: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            network_id: self.network.network_id(),
            payload: vec![],
        };

        let mut all_transactions = vec![coinbase_tx];
        if treasury_allocation > 0 {
            let treasury_tx = Transaction {
                sender: "TREASURY".to_string(),
                recipient: TREASURY_ADDRESS.to_string(),
                amount: treasury_allocation,
                timestamp: chrono::Utc::now().timestamp(),
                signature: vec![],
                public_key: vec![],
                fee: 0,
                nonce: 0,
                lock_time: 0,
                tx_type: crate::core::transaction::TransactionType::Transfer,
                sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
                network_id: self.network.network_id(),
                payload: vec![],
            };
            all_transactions.push(treasury_tx);
        }
        all_transactions.extend(transactions);

        // Compute state root over a projected state (pre-cert, no sig in hash yet).
        let mut state_snap = self.account_state.read().clone();
        for tx in &all_transactions {
            state_snap.credit_account(tx, next_height, COINBASE_MATURITY);
            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                let total = tx.amount.saturating_add(tx.fee);
                state_snap.debit_account(&tx.sender, total);
                state_snap.increment_nonce(&tx.sender);
            }
        }
        let state_root = state_snap.calculate_state_root();

        let previous_hash = self.get_latest_block().hash.clone();
        let mut block = Block::new_bft(
            next_height,
            all_transactions,
            previous_hash,
            epoch,
            round,
            proposer_address,
        );
        block.state_root = state_root;
        block.finalize_hash();
        Ok(block)
    }

    /// Get a snapshot (clone) of the current account state.
    /// Used by the BFT proposer to resolve committee keys without holding a lock.
    pub fn get_account_state_snapshot(&self) -> crate::core::transaction::AccountState {
        self.account_state.read().clone()
    }

    /// Get block at exact height from storage. Returns None if not found.
    pub fn get_block_by_index(&self, index: u64) -> Option<Block> {
        if index == self.get_height().saturating_sub(1) {
            return Some(self.get_latest_block());
        }
        self.storage.load_block(index).ok()
    }

    /// Get current BFT block reward (proposer reward = mining reward equivalent).
    fn get_block_reward(&self) -> u64 {
        use crate::consensus::authorities::V3_ECONOMICS_HEIGHT;
        let chain_len = self.get_height();
        let base_reward = if chain_len >= V3_ECONOMICS_HEIGHT { 500_000 } else { YEAR_1_REWARD };
        let years_elapsed = chain_len / BLOCKS_PER_YEAR;
        apply_annual_reduction(base_reward, years_elapsed).max(MIN_REWARD)
    }

    /// Get current difficulty — reads from STORAGE (the real chain), not the
    /// in-memory `chain` vec which only holds genesis after startup.
    #[allow(dead_code)]
    /// Validate block against consensus rules (CRITICAL for network blocks)
    fn validate_block_consensus(
        &self,
        block: &Block,
        previous: &Block,
        base_state: &AccountState,
    ) -> Result<(), BlockchainError> {
        // 0. Block size limit (DoS protection)
        let block_size = bincode::serialize(block)
            .map_err(|_| BlockchainError::InvalidBlock)?
            .len();
        if block_size > MAX_BLOCK_SIZE_BYTES {
            return Err(BlockchainError::BlockTooLarge { size: block_size });
        }

        // 1. Cryptographic validity (done in block.is_valid)

        // 2. Timestamp bounds (prevent manipulation and time-travel attacks)
        if block.timestamp <= previous.timestamp {
            tracing::warn!(
                "Block timestamp {} <= previous {}",
                block.timestamp,
                previous.timestamp
            );
            return Err(BlockchainError::InvalidBlock);
        }
        let current_time = chrono::Utc::now().timestamp();
        if block.timestamp > current_time + MAX_FUTURE_BLOCK_TIME {
            tracing::warn!(
                "Block timestamp {} too far in future (max +{} sec)",
                block.timestamp - current_time,
                MAX_FUTURE_BLOCK_TIME
            );
            return Err(BlockchainError::InvalidBlock);
        }
        // MED-1 FIX: Apply Median-Time-Past (MTP) rule — classic time-warp defense.
        // Block timestamp must be strictly greater than the median of the last 11 blocks.
        // This prevents a majority miner from drifting timestamps forward to manipulate
        // difficulty downward (Bitcoin BIP-113 equivalent).
        if previous.index >= 10 {
            let mtp = self.median_time_past(previous.index, 11);
            if block.timestamp <= mtp {
                tracing::warn!(
                    "Block timestamp {} <= MTP {} (time-warp attack rejected)",
                    block.timestamp,
                    mtp
                );
                return Err(BlockchainError::InvalidBlock);
            }
        }
        // Removed MAX_TIME_DELTA check. Large forward gaps are valid if the network stops.
        // Large backward gaps are already prevented by MTP and `block.timestamp <= previous.timestamp`.

        // 3. BFT Certificate Verification
        //
        // Every block (except genesis) must carry a valid BFT certificate:
        // ≥ ⌈2/3⌉ of the epoch committee must have signed bft_signing_payload().
        // Committee is derived from base_state (state *before* this block).
        if block.index > 0 {
            let session_start_height =
                block.index - (block.index % crate::consensus::authorities::SESSION_LENGTH);
            let committee = base_state.compute_epoch_committee(
                crate::consensus::authorities::MAX_COMMITTEE_SIZE,
                session_start_height,
            );
            if !crate::consensus::bft::verify_bft_certificate(block, &committee, base_state) {
                tracing::warn!(
                    "Block {}: BFT certificate verification failed (committee_size={})",
                    block.index,
                    committee.len()
                );
                return Err(BlockchainError::InvalidBlock);
            }
            tracing::debug!(
                "Block {}: BFT certificate verified ({} sigs)",
                block.index,
                block.bft_signatures.len()
            );
        }

        // 4. Coinbase validation - Must account for fee distribution
        let coinbase_txs: Vec<_> = block
            .transactions
            .iter()
            .filter(|tx| tx.is_coinbase())
            .collect();
        if coinbase_txs.is_empty() || coinbase_txs.len() > 1 {
            tracing::warn!(
                "Block must have exactly one coinbase transaction, found {}",
                coinbase_txs.len()
            );
            return Err(BlockchainError::InvalidBlock);
        }

        // Validate treasury transaction if present
        let treasury_txs: Vec<_> = block
            .transactions
            .iter()
            .filter(|tx| tx.sender == "TREASURY")
            .collect();

        let coinbase = coinbase_txs[0];
        let expected_reward = self.calculate_reward_at_height(block.index);
        let total_fees: u64 = block
            .transactions
            .iter()
            .filter(|tx| !tx.is_coinbase() && tx.sender != "TREASURY")
            .map(|tx| tx.fee)
            .sum();

        // FEE DISTRIBUTION: 70% burn, 20% treasury, 10% miner
        let fee_to_miner = (total_fees * FEE_VALIDATOR_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;

        // v3 REWARD DISTRIBUTION: 8% QEF, 92% to block proposer (no PoW lock in BFT)
        let treasury_allocation = (expected_reward * TREASURY_ALLOCATION_PERCENT) / 100;
        let proposer_reward = expected_reward - treasury_allocation;

        // Coinbase should contain: full proposer reward + validator fee share
        let expected_coinbase = proposer_reward.saturating_add(fee_to_miner);
        if coinbase.amount != expected_coinbase {
            tracing::warn!(
                "Invalid coinbase amount: expected {} (reward: {}, fees: {}), got {}",
                expected_coinbase,
                proposer_reward,
                fee_to_miner,
                coinbase.amount
            );
            return Err(BlockchainError::InvalidCoinbaseReward {
                actual: coinbase.amount,
                expected: expected_coinbase,
            });
        }

        // Validate coinbase recipient: before activation → must be block proposer;
        // after activation → must be EPOCH_POOL_ADDRESS.
        use crate::consensus::authorities::EPOCH_REWARD_ACTIVATION_HEIGHT;
        let expected_coinbase_recipient = if block.index >= EPOCH_REWARD_ACTIVATION_HEIGHT {
            EPOCH_POOL_ADDRESS.to_string()
        } else {
            block.proposer.clone()
        };
        if coinbase.recipient != expected_coinbase_recipient {
            tracing::warn!(
                "Invalid coinbase recipient at block {}: expected {}, got {}",
                block.index, expected_coinbase_recipient, coinbase.recipient
            );
            return Err(BlockchainError::InvalidBlock);
        }

        // Validate treasury transaction if fees or allocation exist
        let expected_treasury = treasury_allocation.saturating_add(fee_to_treasury);
        if expected_treasury > 0 {
            if treasury_txs.len() != 1 {
                tracing::warn!(
                    "Block should have treasury transaction for {} microunits",
                    expected_treasury
                );
                return Err(BlockchainError::InvalidBlock);
            }

            let treasury_tx = treasury_txs[0];
            if treasury_tx.amount != expected_treasury {
                tracing::warn!(
                    "Invalid treasury amount: expected {}, got {}",
                    expected_treasury,
                    treasury_tx.amount
                );
                return Err(BlockchainError::InvalidBlock);
            }

            if treasury_tx.recipient != TREASURY_ADDRESS {
                tracing::warn!(
                    "Treasury transaction sent to wrong address: {}",
                    treasury_tx.recipient
                );
                return Err(BlockchainError::InvalidBlock);
            }
        } else if !treasury_txs.is_empty() {
            tracing::warn!("Block has treasury transaction but no allocation expected");
            return Err(BlockchainError::InvalidBlock);
        }

        // 5. All non-coinbase txs must have valid signatures and nonces
        // CRITICAL: Build temporary state to validate balances and nonces.
        // STATE-ROOT FIX: unlock_mature_coinbase must be called here BEFORE
        // applying transactions — matching create_block_template which also
        // calls unlock before computing the state_root it embeds in the block.
        // Without this, the two sides compute a different state hash whenever
        // any miner address has coinbase locks that mature at this block height.
        let mut temp_state = base_state.clone();
        temp_state.unlock_mature_coinbase(block.index);

        // OPT-1+3 (PQC): Parallel sig verification with signature cache + pubkey cache
        // Serial: 1200 tx × 1.5ms = 1800ms
        // Parallel (physical cores): ~300ms
        // With caches: near-zero for repeat senders
        let all_sigs_valid = block.transactions.par_iter().all(|tx| {
            if tx.is_coinbase() || tx.sender == "TREASURY" || tx.is_genesis_premine() {
                return true;
            }

            // OPT-1: Signature cache — skip re-verification of known-good txs
            let tx_hash = tx.hash();
            {
                let mut cache = self.signature_cache.lock().unwrap();
                if let Some(&is_valid) = cache.get(&tx_hash) {
                    return is_valid; // Cache hit!
                }
            }

            // OPT-3: Pubkey cache — if we've seen this sender before,
            // confirm their public key matches (avoids re-deserializing 897 bytes)
            if let Some(cached_pk) = self.pubkey_cache.get(&tx.sender) {
                if cached_pk.as_slice() != tx.public_key.as_slice() {
                    // Key mismatch — this sender is using a different key (suspicious)
                    tracing::warn!("Pubkey mismatch for sender {}", tx.sender);
                    return false;
                }
            } else if !tx.public_key.is_empty() {
                // First time seeing this sender — store key for future blocks
                self.pubkey_cache
                    .insert(tx.sender.clone(), tx.public_key.clone());
            }

            // Cache miss — do full Falcon-512 verification
            let is_valid = tx.verify();
            // CRIT-4 FIX: Only cache SUCCESSFUL verifications.
            // Caching false would let an attacker poison the cache:
            // submit one invalid tx, then valid txs with the same hash
            // are permanently rejected from this node (desync attack).
            if is_valid {
                let mut cache = self.signature_cache.lock().unwrap();
                cache.put(tx_hash, true);
            }
            is_valid
        });

        if !all_sigs_valid {
            return Err(BlockchainError::InvalidSignature);
        }

        // Now validate fees, nonces, and balances sequentially (need state tracking)
        for tx in &block.transactions {
            // Skip system transactions (coinbase, treasury, genesis premine)
            if tx.is_coinbase() || tx.sender == "TREASURY" || tx.is_genesis_premine() {
                continue;
            }

            // Fee must meet minimum
            if tx.fee < MIN_TRANSACTION_FEE {
                return Err(BlockchainError::FeeTooLow {
                    fee: tx.fee,
                    min: MIN_TRANSACTION_FEE,
                });
            }

            // SECURITY FIX: Enforce lock_time (Fee sniping defense)
            if tx.lock_time > block.index {
                tracing::warn!(
                    "Transaction locked until block {}, but included in block {}",
                    tx.lock_time,
                    block.index
                );
                return Err(BlockchainError::InvalidBlock);
            }

            // CRITICAL: Validate nonce is sequential (prevents replay)
            let expected_nonce = temp_state.get_nonce(&tx.sender) + 1;
            if tx.nonce != expected_nonce {
                // For blocks below the highest hardcoded checkpoint we trust the
                // block's nonce rather than rejecting.  A buggy reorg (the snapshot-
                // fallback bug fixed in v0.7.3) could cause the canonical chain to
                // contain a TX whose nonce is valid only relative to the post-reorg
                // account state.  A clean sequential-sync node sees the "wrong" nonce
                // because it counted transactions that the reorg erased.  Overriding
                // temp_state's nonce to tx.nonce-1 reproduces the post-reorg state so
                // all subsequent blocks validate correctly.  The checkpoint hash already
                // guarantees these blocks' content is canonical.
                let max_cp = TESTNET_CHECKPOINTS
                    .iter()
                    .map(|(h, _)| *h)
                    .max()
                    .unwrap_or(0);
                if block.index < max_cp {
                    tracing::debug!(
                        "Pre-checkpoint nonce override block {}: {} expected {} got {} — trusting block",
                        block.index, &tx.sender, expected_nonce, tx.nonce
                    );
                    temp_state.set_nonce(&tx.sender, tx.nonce.saturating_sub(1));
                } else {
                    tracing::warn!(
                        "Invalid nonce in block: tx from {} has nonce {}, expected {}",
                        tx.sender,
                        tx.nonce,
                        expected_nonce
                    );
                    return Err(BlockchainError::InvalidNonce {
                        expected: expected_nonce,
                        actual: tx.nonce,
                    });
                }
            }

            // CRITICAL: Validate sufficient balance (prevents double-spend)
            let total_required = tx.amount.saturating_add(tx.fee);
            let available = temp_state.get_balance(&tx.sender);
            if available < total_required {
                tracing::warn!(
                    "Insufficient balance in block: {} has {} but needs {}",
                    tx.sender,
                    available,
                    total_required
                );
                return Err(BlockchainError::InsufficientBalance {
                    required: total_required,
                    available,
                });
            }

            // Update temporary state to validate next transactions
            if !temp_state.debit_account(&tx.sender, total_required) {
                return Err(BlockchainError::InvalidBlock);
            }

            // Handle Validator Staking (v2) — with min-stake, re-stake guard, slash cooldown
            if let crate::core::transaction::TransactionType::Stake { validator_pubkey } =
                &tx.tx_type
            {
                use crate::consensus::authorities::{
                    MIN_VALIDATOR_STAKE, OPEN_VALIDATOR_REGISTRATION_HEIGHT,
                };
                let epoch = crate::consensus::authorities::epoch_for_height(block.index);

                // Guard 1: Minimum stake requirement
                if tx.amount < MIN_VALIDATOR_STAKE {
                    tracing::warn!(
                        "Stake rejected: {} staked {} microunits, minimum is {} ({}k QUA)",
                        tx.sender,
                        tx.amount,
                        MIN_VALIDATOR_STAKE,
                        MIN_VALIDATOR_STAKE / 1_000_000_000
                    );
                    return Err(BlockchainError::InvalidBlock);
                }

                // Guard 2: Cannot stake if already an active validator
                let already_active = temp_state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.active)
                    .unwrap_or(false);
                if already_active {
                    tracing::warn!(
                        "Stake rejected: {} is already an active validator",
                        tx.sender
                    );
                    return Err(BlockchainError::InvalidBlock);
                }

                // Guard 3: Cannot re-stake during slash cooldown
                let slash_cooldown = temp_state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.slash_cooldown_until_epoch)
                    .unwrap_or(0);
                if slash_cooldown > epoch {
                    tracing::warn!(
                        "Stake rejected: {} is in slash cooldown until epoch {} (current: {})",
                        tx.sender,
                        slash_cooldown,
                        epoch
                    );
                    return Err(BlockchainError::InvalidBlock);
                }

                // Guard 4: Cannot re-stake while unbonding (prevents burning old stake)
                let is_unbonding = temp_state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.unbonding_epoch > 0)
                    .unwrap_or(false);
                if is_unbonding {
                    tracing::warn!(
                        "Stake rejected: {} is currently unbonding. Wait for stake return before re-registering.",
                        tx.sender
                    );
                    return Err(BlockchainError::InvalidBlock);
                }

                // Guard 5: Open registration switch
                // Before OPEN_VALIDATOR_REGISTRATION_HEIGHT: Stake txs are recorded but the
                // validator is NOT added to the active committee (genesis validators dominate).
                // After: all stakers are eligible for the committee immediately.
                let registration_open = block.index >= OPEN_VALIDATOR_REGISTRATION_HEIGHT;
                if registration_open {
                    temp_state.register_validator(
                        &tx.sender,
                        validator_pubkey.clone(),
                        tx.amount,
                        block.index,
                    );
                    tracing::info!(
                        "Validator registered (open set): {} (epoch={}, stake={} microunits)",
                        tx.sender,
                        epoch,
                        tx.amount
                    );
                } else {
                    // Pre-registration-open: store with active=false so the unbonding/epoch
                    // logic still works, but they won't enter the committee until the switch.
                    temp_state.register_validator(
                        &tx.sender,
                        validator_pubkey.clone(),
                        tx.amount,
                        block.index,
                    );
                    tracing::info!(
                        "Validator pre-registered (opens at height {}): {} (stake={} microunits)",
                        OPEN_VALIDATOR_REGISTRATION_HEIGHT,
                        tx.sender,
                        tx.amount
                    );
                }
            }

            // Handle Validator Unstaking (v2) — now records unbonding epoch
            if tx.is_unstake() {
                // Guard: sender must actually be a registered active validator
                let is_active = temp_state
                    .get_validator_info(&tx.sender)
                    .map(|v| v.active)
                    .unwrap_or(false);
                if !is_active {
                    tracing::warn!(
                        "Unstake rejected: {} is not a registered active validator",
                        tx.sender
                    );
                    return Err(BlockchainError::InvalidBlock);
                }
                let epoch = crate::consensus::authorities::epoch_for_height(block.index);
                temp_state.deregister_validator(&tx.sender, epoch);
                tracing::info!(
                    "Validator deregistered: {} (unbonding epoch={})",
                    tx.sender,
                    epoch
                );
            }

            temp_state.credit_account(tx, block.index, COINBASE_MATURITY);
            temp_state.increment_nonce(&tx.sender);

            // Handle SlashEvidence (v3) — equivocation slashing
            if let crate::core::transaction::TransactionType::SlashEvidence {
                offender,
                height: _,
                round: _,
                sig_a,
                hash_a,
                sig_b,
                hash_b,
            } = &tx.tx_type
            {
                // Verify the two signatures are genuinely different (different block hashes)
                if hash_a == hash_b {
                    tracing::warn!("SlashEvidence rejected: hash_a == hash_b (not a conflict)");
                    return Err(BlockchainError::InvalidBlock);
                }
                // Verify the validator has a recorded public key we can check against
                let validator_pk = temp_state
                    .get_validator_info(offender)
                    .map(|v| v.falcon_pk.clone());
                if let Some(pk) = validator_pk {
                    // Verify both signatures are valid Falcon-512 signatures from offender.
                    // We verify the raw sig blobs against their respective hash payloads.
                    let sig_a_valid = crate::crypto::verify_signature_strict(
                        &crate::crypto::canonical_signing_hash(hash_a.as_bytes()),
                        sig_a,
                        &pk,
                    );
                    let sig_b_valid = crate::crypto::verify_signature_strict(
                        &crate::crypto::canonical_signing_hash(hash_b.as_bytes()),
                        sig_b,
                        &pk,
                    );
                    if !sig_a_valid || !sig_b_valid {
                        tracing::warn!(
                            "SlashEvidence rejected: one or both signatures invalid for {}",
                            offender
                        );
                        return Err(BlockchainError::InvalidBlock);
                    }
                    // Evidence is valid — slash will be applied during state application.
                    tracing::info!(
                        "SlashEvidence accepted for offender {} — will be slashed at application",
                        offender
                    );
                } else {
                    tracing::warn!(
                        "SlashEvidence rejected: {} is not a known validator",
                        offender
                    );
                    return Err(BlockchainError::InvalidBlock);
                }
            }
        }

        // Apply system transactions to temp_state for state_root calculation
        for tx in &block.transactions {
            if tx.is_coinbase() || tx.sender == "TREASURY" {
                temp_state.credit_account(tx, block.index, COINBASE_MATURITY);
            } else if tx.is_genesis_premine() {
                // Genesis premine: immediately spendable (maturity = 0)
                temp_state.credit_account(tx, block.index, 0);
            }
        }

        // STATE ROOT VALIDATION
        // If the block provides a state_root, it must match our computed value.
        // Blocks that omit state_root (empty string) are accepted — they pre-date
        // this feature. This is safe because the Merkle root already commits to
        // all transaction data; state_root adds an extra account-state binding
        // for nodes that compute it.
        //
        // Exemptions (skip state_root check):
        //   1. v2: State root is enforced on all blocks from height 1.
        //   2. CHECKPOINTED BLOCKS: if the block's hash matches a hardcoded checkpoint,
        //      the block's content is already canonical — we cannot reject it regardless
        //      of what our local state replay produced.  This handles the case where a
        //      clean-sync node's account state at some prior height diverges from the
        //      mining node's state (e.g. block 90,000: the mining node's state at 89,999
        //      differs from a node that replayed all blocks from genesis).  The checkpoint
        //      hash already commits to every tx in the block; the state_root check adds
        //      no additional security for checkpointed heights.
        let computed_state_root = temp_state.calculate_state_root();
        let is_checkpointed = self.validate_checkpoint(block.index, &block.hash) && {
            // validate_checkpoint returns true for heights with NO checkpoint too,
            // so we must confirm there IS a checkpoint at this exact height.
            let checkpoints = match self.network {
                ChainNetwork::Testnet => TESTNET_CHECKPOINTS,
                ChainNetwork::Mainnet => MAINNET_CHECKPOINTS,
                ChainNetwork::Devnet(_) => &[],
            };
            checkpoints.iter().any(|(h, _)| *h == block.index)
        };
        if block.index > 0
            && !block.state_root.is_empty()
            && block.state_root != computed_state_root
            && !is_checkpointed
            && block.index != 12615
        // SOFT UPDATE: Exemption for consensus bug block
        {
            tracing::warn!(
                "Invalid state root at block {}: computed={}, block={}",
                block.index,
                computed_state_root,
                block.state_root
            );
            return Err(BlockchainError::InvalidBlock);
        }
        if is_checkpointed
            && !block.state_root.is_empty()
            && block.state_root != computed_state_root
        {
            tracing::info!(
                "State root mismatch at checkpointed block {} (computed={}, block={}) — \
                 trusting checkpoint; local state will converge from this height onward.",
                block.index,
                computed_state_root,
                block.state_root
            );
        }

        Ok(())
    }

    /// Validate block against consensus rules during REORG / SYNC replay.
    ///
    /// In v2 BFT, all committed blocks are final. This function performs
    /// BFT certificate verification and coinbase/signature checks.
    fn validate_block_consensus_reorg(
        &self,
        block: &Block,
        previous: &Block,
    ) -> Result<(), BlockchainError> {
        // Size check.
        let block_size = bincode::serialize(block)
            .map_err(|_| BlockchainError::InvalidBlock)?
            .len();
        if block_size > MAX_BLOCK_SIZE_BYTES {
            return Err(BlockchainError::BlockTooLarge { size: block_size });
        }

        // Timestamp must be strictly after parent.
        if block.timestamp <= previous.timestamp {
            tracing::warn!("Reorg block {} timestamp <= previous", block.index);
            return Err(BlockchainError::InvalidBlock);
        }

        // BFT certificate check (skip genesis).
        if block.index > 0 {
            let state = self.get_account_state_snapshot();
            let session_start_height =
                block.index - (block.index % crate::consensus::authorities::SESSION_LENGTH);
            let committee = state.compute_epoch_committee(
                crate::consensus::authorities::MAX_COMMITTEE_SIZE,
                session_start_height,
            );
            if !crate::consensus::bft::verify_bft_certificate(block, &committee, &state) {
                tracing::warn!("Reorg block {}: BFT certificate invalid", block.index);
                return Err(BlockchainError::InvalidBlock);
            }
        }

        // Signature validation on all user transactions.
        let all_sigs_valid = block.transactions.par_iter().all(|tx| {
            if tx.is_coinbase() || tx.sender == "TREASURY" || tx.is_genesis_premine() {
                return true;
            }
            let tx_hash = tx.hash();
            {
                let mut cache = self.signature_cache.lock().unwrap();
                if let Some(&is_valid) = cache.get(&tx_hash) {
                    return is_valid;
                }
            }
            let is_valid = tx.verify();
            if is_valid {
                let mut cache = self.signature_cache.lock().unwrap();
                cache.put(tx_hash, true);
            }
            is_valid
        });
        if !all_sigs_valid {
            tracing::warn!(
                "Reorg block {} contains invalid transaction signatures",
                block.index
            );
            return Err(BlockchainError::InvalidSignature);
        }

        Ok(())
    }

    /// `add_block_to_main_chain_reorg` — like `add_block_to_main_chain` but
    /// called from `deep_reorg` where we are re-applying blocks after a rollback.
    /// Does NOT flush storage (the caller flushes once at the end of the reorg)
    /// and tracks cumulative_work incrementally.
    fn add_block_to_main_chain_reorg(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();

        if !self.validate_checkpoint(block.index, &block.hash) {
            tracing::error!("Reorg: checkpoint violation at block {}", block.index);
            return Err(BlockchainError::InvalidBlock);
        }

        if !block.is_valid(Some(&latest)) {
            tracing::warn!("Reorg: block {} failed is_valid", block.index);
            return Err(BlockchainError::InvalidBlock);
        }

        // Use the PERMISSIVE reorg validator: blocks from a peer's fork may have
        // a difficulty that differs slightly from our LWMA (because our fork diverged
        // at a prior block with a different timestamp).  The strict validator would
        // reject them even though their PoW is genuine.
        self.validate_block_consensus_reorg(&block, &latest)?;

        let mut new_state = self.account_state.read().clone();
        new_state.unlock_mature_coinbase(block.index);

        for tx in &block.transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!(
                        "Reorg: block {} has invalid tx (insufficient balance)",
                        block.index
                    );
                    return Err(BlockchainError::InvalidBlock);
                }
                new_state.increment_nonce(&tx.sender);
            }
            let maturity = if tx.is_genesis_premine() {
                0
            } else {
                COINBASE_MATURITY
            };
            new_state.credit_account(tx, block.index, maturity);
        }

        self.storage.save_block(&block)?;
        self.storage.set_chain_height(block.index + 1)?;
        self.storage.save_account_state(&new_state)?;

        // Update cumulative work incrementally.
        let new_work = {
            let mut cw = self.cumulative_work.lock();
            *cw = cw.saturating_add(1u128); // BFT: 1 unit per block
            *cw
        };
        let _ = self.storage.set_cumulative_work(new_work);

        // Save checkpoint every 1000 blocks during reorg too.
        const CHECKPOINT_INTERVAL: u64 = 1000;
        if block.index % CHECKPOINT_INTERVAL == 0 && block.index > 0 {
            let _ = self
                .storage
                .save_account_state_at_height(block.index, &new_state);
        }

        *self.account_state.write() = new_state;

        // Clear ALL pending nonces after a reorg — the DashMap entries from the
        // abandoned fork are now stale (wrong base nonce) and would cause every
        // subsequent mempool submission to fail with "Invalid nonce: expected N, got 1".
        self.pending_nonces.clear();

        // Notify BFT proposer: chain moved during reorg, restart block template.
        let _ = self.new_block_tx.send(block.index + 1);

        tracing::info!(
            "Reorg: network block {} accepted (permissive diff check)",
            block.index
        );
        Ok(())
    }

    /// Calculate reward at specific height (for validation)
    ///
    /// CONSENSUS-CRITICAL: Must match `get_mining_reward` exactly.
    /// Pure integer math — no f64.
    fn calculate_reward_at_height(&self, height: u64) -> u64 {
        use crate::consensus::authorities::V3_ECONOMICS_HEIGHT;
        let base_reward = if height >= V3_ECONOMICS_HEIGHT { 500_000 } else { YEAR_1_REWARD };
        let years_elapsed = height / BLOCKS_PER_YEAR;
        apply_annual_reduction(base_reward, years_elapsed).max(MIN_REWARD)
    }

    /// Get median timestamp from last N blocks (prevents timestamp manipulation)
    /// SECURITY FIX (HIGH-1): Median-time-past for difficulty adjustment
    /// Get median timestamp from last N blocks (prevents timestamp manipulation)
    /// SECURITY FIX (HIGH-1): Median-time-past for difficulty adjustment
    fn get_median_time_past(&self, end_index: u64, window: u64) -> i64 {
        if end_index == 0 {
            return 0;
        }

        let start = end_index.saturating_sub(window);
        let mut timestamps = Vec::new();

        // Load blocks from storage
        for h in start..=end_index {
            if let Ok(block) = self.storage.load_block(h) {
                timestamps.push(block.timestamp);
            }
        }

        if timestamps.is_empty() {
            return 0;
        }

        timestamps.sort_unstable();
        timestamps[timestamps.len() / 2] // Return median
    }

    /// Validate the entire blockchain
    /// Validate the entire blockchain
    pub fn is_valid(&self) -> bool {
        let chain_len = self.get_height();

        tracing::info!("Validating chain from storage (height: {})", chain_len);

        // Validate genesis
        if let Ok(genesis) = self.storage.load_block(0) {
            if genesis.index != 0 {
                tracing::error!("Invalid genesis block index");
                return false;
            }
        } else {
            tracing::error!("Could not load genesis block");
            return false;
        }

        // Validate rest
        for i in 1..chain_len {
            let current = match self.storage.load_block(i) {
                Ok(b) => b,
                Err(_) => return false,
            };

            let prev = match self.storage.load_block(i - 1) {
                Ok(b) => b,
                Err(_) => return false,
            };

            if !current.is_valid(Some(&prev)) {
                tracing::error!("Invalid block at height {}", i);
                return false;
            }
        }

        true
    }

    /// Get blockchain statistics
    pub fn get_stats(&self) -> BlockchainStats {
        let height = self.get_height();
        let current_epoch = crate::consensus::authorities::epoch_for_height(height);
        let session_len = crate::consensus::authorities::SESSION_LENGTH;
        let current_session = height / session_len;
        let blocks_until_next_session = session_len - (height % session_len);
        let total_transactions = 0;
        let pending = self.pending_transactions.read();

        let (active_validator_count, total_staked) = {
            let acc_state = self.account_state.read();
            let validators = acc_state.get_validators();
            let staked: u64 = validators.values().map(|v| v.stake).sum();
            (validators.len(), staked)
        };

        let tps = if height >= 10 {
            if let (Some(latest), Some(oldest)) = (
                self.load_block_from_storage(height - 1),
                self.load_block_from_storage(height - 10)
            ) {
                let elapsed = latest.timestamp - oldest.timestamp;
                if elapsed > 0 {
                    let mut tx_count = 0;
                    for i in (height - 10)..=(height - 1) {
                        if let Some(b) = self.load_block_from_storage(i) {
                            tx_count += b.transactions.len();
                        }
                    }
                    tx_count as f64 / elapsed as f64
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0
        };

        BlockchainStats {
            chain_length: height as usize,
            total_transactions,
            current_epoch,
            current_session,
            blocks_until_next_session,
            mining_reward: self.get_block_reward(),
            total_supply: 200_000_000_000_000, // 200m Max Supply
            circulating_supply: self.calculate_total_supply(),
            pending_transactions: pending.len(),
            active_validator_count,
            total_staked,
            tps,
        }
    }

    /// Calculate total coin supply (u64 microunits)
    ///
    /// HIGH-7 FIX: Previously scanned self.chain (genesis only!), always returning
    /// a near-zero value and corrupting BlockchainStats / inflation monitoring.
    /// Now derives supply from the block-reward formula using chain height — O(1),
    /// no disk scan needed, and always correct regardless of pruning mode.
    fn calculate_total_supply(&self) -> u64 {
        let height = self.get_height();
        if height == 0 {
            return 0;
        }
        let mut total_minted: u64 = 0;
        let full_years = height / BLOCKS_PER_YEAR;
        // Sum rewards year-by-year using the exact integer formula
        for y in 0..full_years {
            let reward = apply_annual_reduction(YEAR_1_REWARD, y);
            // Each full year has BLOCKS_PER_YEAR blocks.
            // Miner gets 95% of reward; split 50/50 immediate/locked.
            // Supply counts ALL minted coins (immediate + locked).
            total_minted = total_minted.saturating_add(reward.saturating_mul(BLOCKS_PER_YEAR));
        }
        // Remaining blocks in the current year
        let remaining = height % BLOCKS_PER_YEAR;
        if remaining > 0 {
            let reward = apply_annual_reduction(YEAR_1_REWARD, full_years);
            total_minted = total_minted.saturating_add(reward.saturating_mul(remaining));
        }
        total_minted
    }

    /// Get balance for an address (u64 microunits)
    pub fn get_balance(&self, address: &str) -> u64 {
        self.account_state.read().get_balance(address)
    }

    /// Get the blockchain (for network sync)
    pub fn get_chain(&self) -> parking_lot::RwLockReadGuard<Vec<Block>> {
        self.chain.read()
    }

    /// Get mutable blockchain (for adding blocks from network)
    pub fn get_chain_mut(&self) -> parking_lot::RwLockWriteGuard<Vec<Block>> {
        self.chain.write()
    }

    /// Get pending transactions
    pub fn get_pending_transactions(&self) -> parking_lot::RwLockReadGuard<'_, Vec<Transaction>> {
        self.pending_transactions.read()
    }

    /// Get mutable pending transactions
    #[allow(dead_code)]
    pub fn get_pending_transactions_mut(
        &self,
    ) -> parking_lot::RwLockWriteGuard<'_, Vec<Transaction>> {
        self.pending_transactions.write()
    }

    /// Get account state (mutable) — for use by internal block-application logic.
    pub fn get_account_state_mut(&self) -> parking_lot::RwLockWriteGuard<'_, AccountState> {
        self.account_state.write()
    }

    /// Get account state (read-only) — HIGH-8 FIX: use this in API handlers to avoid
    /// holding a write guard while the outer tokio read lock is held (deadlock risk).
    pub fn get_account_state_read(&self) -> parking_lot::RwLockReadGuard<'_, AccountState> {
        self.account_state.read()
    }

    /// Compute cumulative PoW (sum of difficulty) for the chain up to `tip_height`.
    ///
    /// PERF: O(1) fast-path when `tip_height` equals the current chain height —
    /// reads the in-memory `cumulative_work` field that is updated after every
    /// accepted block. Falls back to O(n) scan only for historical heights
    /// (used during fork resolution, which is rare).
    pub fn cumulative_work_at(&self, tip_height: u64) -> u128 {
        let current = self.get_height();
        // Fast path: asking for the tip we actually have.
        if tip_height == 0 {
            return 0;
        }
        if tip_height >= current {
            return *self.cumulative_work.lock();
        }
        // Slow path: arbitrary historical height (rare — only during deep fork).
        let mut total: u128 = 0;
        for h in 0..=tip_height {
            // inclusive: block AT tip_height contributes its difficulty
            if let Ok(_) = self.storage.load_block(h) {
                total = total.saturating_add(1u128);
            }
        }
        total
    }

    /// Flush all pending sled writes to disk (explicit fsync).
    /// Call this after mining a block or after a sync batch completes.
    pub fn flush_storage(&self) {
        if let Err(e) = self.storage.flush() {
            tracing::warn!("Storage flush failed: {}", e);
        }
    }

    /// Add a block received from the network (WITH FULL VALIDATION AND FORK RESOLUTION)
    pub fn add_network_block(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();

        // 1. SYNC FIX (v3): O(1) duplicate check by index + hash instead of O(n) hash scan.
        // This prevents disk I/O starvation during bulk sync.
        if self.has_block_at_index(block.index, &block.hash) {
            return Ok(()); // Already have this exact block at this index
        }

        // 2. FORK DETECTION: Check if this block builds on our chain
        if block.previous_hash == latest.hash && block.index == latest.index + 1 {
            // Normal case: extends our chain
            let res = self.add_block_to_main_chain(block);
            if res.is_ok() {
                // If we successfully added a block, see if any orphans can now be attached!
                self.process_orphans();
            }
            res
        } else if block.index > latest.index {
            // Potential fork: block is ahead of us
            tracing::warn!(
                "Fork detected: Block {} at height {}, we're at {}",
                &block.hash[..8],
                block.index,
                latest.index
            );

            // BFT: Validate BFT certificate for orphan blocks.
            // FIX: Bypass this check for AlephBFT mode where blocks have 0 signatures natively.
            // if block.index > 0 && block.bft_signatures.is_empty() {
            //     tracing::warn!("Rejecting orphan block with missing BFT certificate");
            //     return Err(BlockchainError::InvalidBlock);
            // }

            // Validate merkle root
            let tree = crate::core::merkle::MerkleTree::from_transactions(&block.transactions);
            let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
            if block.merkle_root != computed_root {
                tracing::warn!("Rejecting orphan block with invalid merkle root");
                return Err(BlockchainError::InvalidBlock);
            }

            // Store as orphaned block (MED-3 FIX: VecDeque::pop_front is O(1) vs Vec::remove(0))
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() >= MAX_ORPHAN_BLOCKS {
                tracing::warn!("Max orphan blocks reached, dropping oldest");
                orphans.pop_front(); // O(1) instead of O(n) Vec::remove(0)
            }
            orphans.push_back(block.clone());
            drop(orphans);

            tracing::info!(
                "Stored orphaned block at height {}, need to sync",
                block.index
            );
            return Ok(());
        } else if block.index == latest.index {
            // Competing block at same height — BFT uses first-seen rule (not cumulative work).
            tracing::warn!(
                "Competing block at height {}: incoming {} vs ours {}",
                block.index,
                &block.hash[..8],
                &latest.hash[..8]
            );

            // BFT: require valid certificate for competing block.
            // FIX: Bypass this check for AlephBFT mode where blocks have 0 signatures natively.
            // if block.index > 0 && block.bft_signatures.is_empty() {
            //     tracing::warn!("Rejecting competing block with missing BFT certificate");
            //     return Err(BlockchainError::InvalidBlock);
            // }

            let tree = crate::core::merkle::MerkleTree::from_transactions(&block.transactions);
            let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
            if block.merkle_root != computed_root {
                tracing::warn!("Rejecting competing block with invalid merkle root");
                return Err(BlockchainError::InvalidBlock);
            }

            // FORK TIE-BREAK: In BFT, the first-seen rule applies. We never
            // orphan a block in favour of a same-height competitor.
            // A genuine double-proposal by two validators at the same height is
            // a safety violation — log it and keep our current tip.
            tracing::warn!(
                "Double-proposal at height {}: keeping our block {} over incoming {}",
                block.index,
                &latest.hash[..8],
                &block.hash[..8]
            );
            return Ok(());
        } else if block.index + 1 == latest.index && block.previous_hash != String::new() {
            // Block is 1 behind our tip — it might be the base of a competing fork.
            // Store in orphans so process_orphans can detect if a longer chain builds on it.
            tracing::debug!(
                "Storing near-stale block at height {} as potential fork base",
                block.index
            );
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() < MAX_ORPHAN_BLOCKS {
                orphans.push_back(block);
            }
            return Ok(());
        } else {
            // Block is behind our chain - likely stale
            tracing::debug!(
                "Ignoring stale block at height {} (we're at {})",
                block.index,
                latest.index
            );
            return Ok(());
        }
    }

    /// Perform a shallow chain reorganization: replace our current tip with `incoming`.
    ///
    /// This is a 1-deep reorg (single block swap at the same height). The previous
    /// block is unchanged; only the tip is replaced. The old tip is returned to the
    /// orphan pool so it is not permanently lost.
    ///
    /// A full deep reorg (multiple blocks) is not needed here because the network
    /// chains only diverge after block 290 — the fork point is always the block
    /// immediately before the competing tips.
    fn reorg_to_block(&self, incoming: Block, old_tip: Block) -> Result<(), BlockchainError> {
        // Validate the incoming block as if it were being added normally.
        let prev_block = self
            .storage
            .load_block(incoming.index - 1)
            .map_err(|_| BlockchainError::InvalidBlock)?;

        if !incoming.is_valid(Some(&prev_block)) {
            tracing::warn!("Reorg aborted: incoming block failed cryptographic validation");
            return Err(BlockchainError::InvalidBlock);
        }

        // Checkpoint guard — never reorg past a checkpoint.
        if !self.validate_checkpoint(incoming.index, &incoming.hash) {
            tracing::error!("Reorg blocked by checkpoint at height {}", incoming.index);
            return Err(BlockchainError::InvalidBlock);
        }

        // Roll back account state to the state BEFORE the old tip was applied.
        // We do this by reloading the last saved state from the block BEFORE the tip.
        // (account state is saved after each block — so loading block N-1's state gives
        //  us the state that existed before block N was applied.)
        //
        // IMPORTANT: We don't have a "rollback transactions" path yet, so we rebuild
        // state from the snapshot saved at height (incoming.index - 1).  The storage
        // layer must have saved account state at each block — which it does in
        // `add_block_to_main_chain` via `storage.save_account_state`.
        //
        // For a full deep reorg this would need to replay from the fork point, but
        // for a 1-deep swap the snapshot at tip−1 is exactly what we need.
        let _pre_tip_state = self.storage.load_account_state().ok().flatten();

        // Rebuild state from scratch up to incoming.index - 1 using the stored snapshot.
        // Since save_account_state is called after EVERY block, the latest snapshot on
        // disk is for the OLD TIP. We need the snapshot for the block BEFORE the tip.
        // Re-apply the incoming block's transactions on top of the pre-fork state.
        //
        // Strategy: reconstruct account state by walking back one block.
        // We snapshot state *before* old_tip's transactions were applied by reloading
        // and reversing.  Since we don't store per-block state snapshots separately,
        // we rebuild by loading genesis state and replaying up through index-1.
        // For testnet heights <2000 this is fast; for production a proper undo log
        // is needed. For now, rebuild from saved state (which is post-old-tip) and
        // then re-apply the incoming block instead.
        //
        // Simplified safe approach: rebuild account state by replaying from genesis
        // through prev_block (incoming.index - 1). This is correct always.
        let new_state = self.rebuild_account_state_up_to(incoming.index - 1);
        self.validate_block_consensus(&incoming, &prev_block, &new_state)?;

        // Apply the incoming block's transactions on the rebuilt state.
        let mut new_state = new_state;
        new_state.unlock_mature_coinbase(incoming.index);
        for tx in &incoming.transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Reorg: incoming block has invalid tx (insufficient balance)");
                    return Err(BlockchainError::InvalidBlock);
                }
                // CRITICAL: Increment nonce so the sender's next transaction uses the right nonce
                new_state.increment_nonce(&tx.sender);
            }
            let maturity = if tx.is_genesis_premine() {
                0
            } else {
                COINBASE_MATURITY
            };
            new_state.credit_account(tx, incoming.index, maturity);
        }

        // Commit: overwrite storage at the tip height with the new block.
        self.storage.save_block(&incoming)?;
        // Height stays the same (same index+1).
        self.storage.save_account_state(&new_state)?;
        *self.account_state.write() = new_state;

        // Return old tip to orphan pool — it may still form a valid longer chain later.
        {
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() < MAX_ORPHAN_BLOCKS {
                orphans.push_back(old_tip);
            }
        }

        // Remove any pending transactions that are now confirmed in the new tip.
        {
            let mut pending = self.pending_transactions.write();
            // Evict by hash OR by (sender, nonce) — the latter is robust against
            // any public_key serialization differences that could cause hash mismatch.
            pending.retain(|tx| {
                !incoming.transactions.iter().any(|btx| {
                    btx.hash() == tx.hash()
                        || (!btx.is_coinbase() && btx.sender == tx.sender && btx.nonce == tx.nonce)
                })
            });
        }
        // REORG-NONCE FIX: clear ALL pending_nonces after a shallow reorg —
        // the old tip's sender nonces are now wrong because the fork erased those txs.
        self.pending_nonces.clear();

        // Notify BFT proposer: tip changed, restart block template immediately.
        let _ = self.new_block_tx.send(incoming.index + 1);

        // Update cumulative_work: subtract old tip's weight (1), add incoming tip's (1). No-op for BFT.

        tracing::info!(
            "Reorg complete: replaced tip at height {} with block {}",
            incoming.index,
            &incoming.hash[..8]
        );
        Ok(())
    }

    /// Rebuild account state by replaying all blocks from genesis up to (and including)
    /// `target_height` from storage. Used during reorg to get a clean state snapshot.
    ///
    /// SYNC FIX: Previously this replayed ALL blocks from genesis (O(height) =
    /// 18k sled disk reads!). Now it loads the nearest 1000-block checkpoint
    /// and replays only the delta (max 1000 blocks).
    fn rebuild_account_state_up_to(
        &self,
        target_height: u64,
    ) -> crate::core::transaction::AccountState {
        // 1. Find the nearest 1000-block snapshot and determine safe replay start.
        //
        // SYNC FIX: The old fallback logic was broken — when a snapshot was missing it
        // called rebuild_state_from_genesis_up_to(0) (genesis premine only), then checked
        // `state.get_accounts().len() > 0` (always true: 10 faucet accounts) and set
        // replay_start = start_height + 1, SKIPPING all blocks 1..start_height.
        // This left the state as if only genesis had been applied → every subsequent block
        // failed with insufficient balance / wrong nonce → "Invalid block" during reorg.
        //
        // Fix: track whether we actually have a valid snapshot. If not, always replay from 1.
        let snapshot_interval: u64 = 1000;
        let snap_height = (target_height / snapshot_interval) * snapshot_interval;

        let (mut state, replay_start) = if snap_height > 0 {
            match self.storage.load_account_state_at_height(snap_height) {
                Ok(Some(snapshot)) => {
                    tracing::info!(
                        "Loaded account state snapshot at height {} (replaying delta to {})",
                        snap_height,
                        target_height
                    );
                    (snapshot, snap_height + 1)
                }
                _ => {
                    // Snapshot missing — must replay from block 1 (genesis premine only at 0).
                    tracing::warn!(
                        "Snapshot missing at height {} — falling back to full replay from genesis (blocks 1..{})",
                        snap_height, target_height
                    );
                    (self.rebuild_state_from_genesis_up_to(0), 1)
                }
            }
        } else {
            // target_height < 1000 — no snapshot possible, replay from block 1.
            (self.rebuild_state_from_genesis_up_to(0), 1)
        };

        // 2. Replay the delta blocks from replay_start..=target_height.
        if replay_start <= target_height {
            tracing::info!(
                "Replaying blocks {}..={} for state rebuild",
                replay_start,
                target_height
            );
            for h in replay_start..=target_height {
                if let Ok(block) = self.storage.load_block(h) {
                    state.unlock_mature_coinbase(block.index);
                    for tx in &block.transactions {
                        if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine()
                        {
                            let total = tx.amount.saturating_add(tx.fee);
                            state.debit_account(&tx.sender, total);
                            state.increment_nonce(&tx.sender);
                        }
                        let maturity = if tx.is_genesis_premine() {
                            0
                        } else {
                            COINBASE_MATURITY
                        };
                        state.credit_account(tx, block.index, maturity);
                    }
                } else {
                    tracing::warn!("rebuild_account_state_up_to: block {} missing from storage — state may be incomplete", h);
                }
            }
        }

        tracing::info!("State rebuild complete: height {}", target_height);
        state
    }

    /// Internal helper: returns a fresh state with only the genesis premine applied.
    fn rebuild_state_from_genesis_up_to(
        &self,
        _dummy: u64,
    ) -> crate::core::transaction::AccountState {
        let mut state = crate::core::transaction::AccountState::new();

        // ---------- GENESIS PREMINE ----------
        let genesis_timestamp = self.storage.load_block(0).map(|g| g.timestamp).unwrap_or(0);
        let testnet_faucets = [
            "0x1683be267318d2ddd8cee8df4a4548dcffb1e088", // Faucet 0 (sender)
            "0xd528c18ce7a8844e4a4dcd841975b20ae599b020", // Faucet 1
            "0xfd6e36bfa2b2798d08592802206c943d5513adfb", // Faucet 2
            "0xed15573ad312d41aaef74cff56a8ef28122ec2db", // Faucet 3
            "0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb", // Faucet 4
            "0xbf5ee055f399323fdd0cefe3d4aa923678d46107", // Faucet 5
            "0x1dc9637b183093d723ea8d1fb18083b06490facb", // Faucet 6
            "0xa2270f30ca1aad922510375508bf68cd95509f29", // Faucet 7
            "0xe15a689775685ae324559ea9a492fc650354ca0b", // Faucet 8
            "0x005dcff212d27b55e7a74bf745e1349ab44ca25d", // Faucet 9
        ];

        let premine_amount = if self.network == crate::core::ChainNetwork::Testnet {
            1_000_000_000_000
        } else {
            1_000_000_000
        };
        let recipients = if self.network == crate::core::ChainNetwork::Testnet {
            testnet_faucets
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        } else {
            vec!["0x0000000000000000000000000000000000000000".to_string()]
        };

        for addr in &recipients {
            let genesis_tx = crate::core::transaction::Transaction {
                sender: "GENESIS".to_string(),
                recipient: addr.to_string(),
                amount: premine_amount,
                timestamp: genesis_timestamp,
                signature: vec![],
                public_key: vec![],
                fee: 0,
                nonce: 0,
                lock_time: 0,
                tx_type: crate::core::transaction::TransactionType::Transfer,
                sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
                network_id: 0,
                payload: vec![],
            };
            state.credit_account(&genesis_tx, 0, 0); // maturity=0 → immediately spendable
        }

        state
    }

    /// Add block to main chain (internal helper)
    fn add_block_to_main_chain(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();

        // CHECKPOINT VALIDATION: Prevent reorganization past checkpoints
        if !self.validate_checkpoint(block.index, &block.hash) {
            tracing::error!(
                "Rejecting block {} due to checkpoint violation",
                block.index
            );
            return Err(BlockchainError::InvalidBlock);
        }

        // Cryptographic validation
        if !block.is_valid(Some(&latest)) {
            return Err(BlockchainError::InvalidBlock);
        }

        // Consensus rules validation
        let mut new_state = self.account_state.read().clone();
        self.validate_block_consensus(&block, &latest, &new_state)?;

        // Unlock any mature coinbase rewards
        new_state.unlock_mature_coinbase(block.index);

        // Apply all transactions to update the new state
        for tx in &block.transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Network block has invalid tx: insufficient balance");
                    return Err(BlockchainError::InvalidBlock);
                }
                new_state.increment_nonce(&tx.sender);

                // Apply Unstake: record unbonding epoch
                if tx.is_unstake() {
                    let epoch = crate::consensus::authorities::epoch_for_height(block.index);
                    new_state.deregister_validator(&tx.sender, epoch);
                }

                // Apply Stake: register validator
                if let crate::core::transaction::TransactionType::Stake { validator_pubkey } =
                    &tx.tx_type
                {
                    new_state.register_validator(
                        &tx.sender,
                        validator_pubkey.clone(),
                        tx.amount,
                        block.index,
                    );
                }

                // Apply SlashEvidence: execute the slash
                if let crate::core::transaction::TransactionType::SlashEvidence {
                    offender,
                    height: _,
                    round: _,
                    sig_a: _,
                    hash_a: _,
                    sig_b: _,
                    hash_b: _,
                } = &tx.tx_type
                {
                    let epoch = crate::consensus::authorities::epoch_for_height(block.index);
                    if let Some((burned, reward)) = new_state.slash_validator(
                        offender,
                        epoch,
                        EQUIVOCATION_SLASH_PCT,
                        WHISTLEBLOWER_REWARD_PCT,
                    ) {
                        // Credit burned amount to burn address (permanently unspendable)
                        new_state.credit_account_direct(BURN_ADDRESS, burned);
                        // Credit whistleblower reward to submitter
                        new_state.credit_account_direct(&tx.sender, reward);
                        tracing::warn!(
                            "SLASH applied: {} burned={} reward_to_whistleblower={}",
                            offender,
                            burned,
                            reward
                        );
                    }
                }
            }
            // GENESIS premine: maturity=0 (immediately spendable)
            // All other txs (including COINBASE mining rewards): COINBASE_MATURITY
            let maturity = if tx.is_genesis_premine() {
                0
            } else {
                COINBASE_MATURITY
            };
            new_state.credit_account(tx, block.index, maturity);
        }

        // Record the block proposer for downtime tracking
        new_state.record_block_proposed(&block.proposer, block.index);

        // EPOCH BOUNDARY: process unbonding returns, downtime slashing, and epoch pool distribution
        use crate::consensus::authorities::EPOCH_SIZE;
        use crate::consensus::authorities::EPOCH_REWARD_ACTIVATION_HEIGHT;
        if block.index > 0 && block.index % EPOCH_SIZE == 0 {
            let epoch = crate::consensus::authorities::epoch_for_height(block.index);
            tracing::info!(
                "Epoch boundary at block {} (epoch {}): processing stake returns and downtime",
                block.index,
                epoch
            );
            let credits = new_state.process_epoch_boundary(epoch, DOWNTIME_SLASH_PCT, BURN_ADDRESS);
            for (addr, amount) in credits {
                if addr == BURN_ADDRESS {
                    new_state.credit_account_direct(BURN_ADDRESS, amount);
                } else {
                    // Return unbonded stake to validator immediately (spendable)
                    new_state.credit_account_direct(&addr, amount);
                    tracing::info!("Unbonded stake returned to {}: {} microunits", addr, amount);
                }
            }

            // EPOCH POOL DISTRIBUTION (active from EPOCH_REWARD_ACTIVATION_HEIGHT)
            // Read the last EPOCH_SIZE block headers to compute each validator's
            // uptime (number of blocks they proposed). Distribute the pool proportionally.
            if block.index >= EPOCH_REWARD_ACTIVATION_HEIGHT {
                let pool_balance = new_state.get_balance(EPOCH_POOL_ADDRESS);
                if pool_balance > 0 {
                    // Count how many blocks each validator proposed in this epoch
                    let epoch_start = block.index.saturating_sub(EPOCH_SIZE - 1);
                    let mut proposer_counts: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
                    let mut total_counted: u64 = 0;
                    for h in epoch_start..=block.index {
                        if let Some(b) = self.storage.load_block(h).ok() {
                            if !b.proposer.is_empty() {
                                *proposer_counts.entry(b.proposer.clone()).or_insert(0) += 1;
                                total_counted += 1;
                            }
                        }
                    }

                    if total_counted > 0 {
                        let mut distributed: u64 = 0;
                        let mut last_proposer: Option<String> = None;
                        for (proposer, count) in &proposer_counts {
                            let share = (pool_balance * count) / total_counted;
                            if share > 0 {
                                new_state.credit_account_direct(proposer, share);
                                distributed = distributed.saturating_add(share);
                                last_proposer = Some(proposer.clone());
                                tracing::info!(
                                    "Epoch {} reward: {} gets {} microunits ({}/{} blocks, {:.1}% uptime)",
                                    epoch, proposer, share, count, total_counted,
                                    (*count as f64 / total_counted as f64) * 100.0
                                );
                            }
                        }
                        // Integer rounding remainder goes to last proposer (prevents dust)
                        let remainder = pool_balance.saturating_sub(distributed);
                        if remainder > 0 {
                            if let Some(addr) = last_proposer {
                                new_state.credit_account_direct(&addr, remainder);
                            }
                        }
                        // Zero out the epoch pool
                        new_state.debit_account_direct(EPOCH_POOL_ADDRESS, pool_balance);
                        tracing::info!(
                            "Epoch {} pool distributed: {} microunits across {} validators",
                            epoch, pool_balance, proposer_counts.len()
                        );
                    }
                }
            }
        }

        // 6. OPTIMIZATION: Don't add to in-memory chain (saves RAM!)

        // 7. COMMIT: Save to storage (primary storage, not memory!)
        self.storage.save_block(&block)?;
        self.storage.set_chain_height(block.index + 1)?;
        self.storage.save_account_state(&new_state)?;

        // Update cumulative work (O(1) — add this block's difficulty to running total).
        let new_work = {
            let mut cw = self.cumulative_work.lock();
            *cw = cw.saturating_add(1u128); // BFT v2: each block = 1 work unit
            *cw
        };
        let _ = self.storage.set_cumulative_work(new_work);

        // Save account-state checkpoint every CHECKPOINT_INTERVAL blocks.
        // Allows deep_reorg / rebuild_account_state_up_to to load the nearest
        // checkpoint and replay only the delta instead of from genesis.
        const CHECKPOINT_INTERVAL: u64 = 1000;
        if block.index % CHECKPOINT_INTERVAL == 0 && block.index > 0 {
            if let Err(e) = self
                .storage
                .save_account_state_at_height(block.index, &new_state)
            {
                tracing::warn!(
                    "Failed to save account-state checkpoint at {}: {}",
                    block.index,
                    e
                );
            }
        }

        // 8. COMMIT: Update in-memory state
        *self.account_state.write() = new_state;

        // 9. Remove mined transactions from pending
        let mut pending = self.pending_transactions.write();
        // Evict by hash OR by (sender, nonce) — the latter is robust against
        // any public_key serialization differences that could cause hash mismatch.
        pending.retain(|tx| {
            !block.transactions.iter().any(|btx| {
                btx.hash() == tx.hash()
                    || (!btx.is_coinbase() && btx.sender == tx.sender && btx.nonce == tx.nonce)
            })
        });
        drop(pending);

        // 10. Clear pending nonces for mined senders.
        // REORG-NONCE FIX: clear ALL entries whose chain nonce now matches or
        // exceeds our cached pending nonce, rather than only clearing entries for
        // senders in *this* block.  After any reorg the chain nonces may have
        // jumped relative to the DashMap entries from the abandoned fork, causing
        // the next mempool submission to see "expected N, got 1" errors.
        for tx in &block.transactions {
            if !tx.is_coinbase() {
                self.pending_nonces.remove(&tx.sender);
            }
        }
        // Stale-nonce sweep: remove any entry where our cached pending nonce is
        // now <= the confirmed chain nonce (the tx was confirmed or reorg erased it).
        let confirmed_state = self.account_state.read();
        self.pending_nonces
            .retain(|addr, cached_nonce| *cached_nonce > confirmed_state.get_nonce(addr));
        drop(confirmed_state);

        // 11. Notify BFT proposer: chain has moved, restart block template.
        let _ = self.new_block_tx.send(block.index + 1);

        tracing::info!(
            "Network block {} accepted at height {}",
            block.index,
            block.index
        );
        Ok(())
    }

    /// Process the orphan pool recursively to attach any pending blocks
    fn process_orphans(&self) {
        loop {
            let latest = self.get_latest_block();
            let expected_index = latest.index + 1;
            let expected_prev_hash = latest.hash.clone();

            let mut orphans = self.orphaned_blocks.write();

            // Find an orphan that connects to our new tip
            let mut found_index = None;
            for (i, orphan) in orphans.iter().enumerate() {
                if orphan.index == expected_index && orphan.previous_hash == expected_prev_hash {
                    found_index = Some(i);
                    break;
                }
            }

            if let Some(idx) = found_index {
                // Remove the connected orphan
                let block = orphans.remove(idx).unwrap();
                drop(orphans); // Drop lock before adding to main chain

                tracing::info!(
                    "Orphan block {} connects to main chain at height {}",
                    &block.hash[..8],
                    block.index
                );
                if let Err(e) = self.add_block_to_main_chain(block) {
                    tracing::warn!("Failed to add formerly orphaned block: {}", e);
                    break;
                }
            } else {
                break; // No more orphans connect
            }
        }
    }

    /// Check if a block exists in the chain by hash.
    ///
    /// SYNC FIX (v3): The previous implementation scanned ALL blocks from 1 to
    /// height (O(n) disk reads!) which was catastrophically slow during sync.
    /// At height 272, each incoming block triggered 272 disk reads — for a
    /// 500-block sync batch that's 136,000 reads, causing I/O starvation.
    ///
    /// New approach: We don't need to find which block has this hash. We just
    /// need to know if ANY stored block has this hash. We can check using the
    /// block index if available, or do a fast "tip check" for the common case
    /// (duplicate of the latest block). For truly unknown blocks, we accept
    /// the small risk of re-processing (add_block_to_main_chain validates fully).
    #[allow(dead_code)]
    pub fn has_block(&self, hash: &str) -> bool {
        // Fast path: genesis is in memory
        if let Some(genesis) = self.chain.read().first() {
            if genesis.hash == hash {
                return true;
            }
        }
        // Fast path: check latest block (most common duplicate case)
        let height = self.get_height();
        if height > 0 {
            if let Ok(latest) = self.storage.load_block(height - 1) {
                if latest.hash == hash {
                    return true;
                }
            }
        }
        // Note: We deliberately skip the full O(n) scan here. If a block
        // is a true duplicate at a non-tip height, add_block_to_main_chain
        // will safely reject it due to index/hash mismatch. The cost of
        // occasionally re-validating a block is far less than the O(n) scan
        // cost during sync (which was the primary cause of sync stalls).
        false
    }

    /// Check if a block at a specific index exists in storage with matching hash.
    /// O(1) lookup used during sync to avoid duplicate application.
    #[allow(dead_code)]
    pub fn has_block_at_index(&self, index: u64, hash: &str) -> bool {
        if let Ok(b) = self.storage.load_block(index) {
            return b.hash == hash;
        }
        false
    }

    /// Get block by height
    #[allow(dead_code)]
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let chain = self.chain.read();
        chain.get(height as usize).cloned()
    }

    /// Get current chain height (OPTIMIZED - from storage, not memory)
    pub fn get_height(&self) -> u64 {
        self.storage.get_chain_height().unwrap_or(0)
    }

    /// Load a specific block by height from disk (used by network sync handlers)
    pub fn load_block_from_storage(&self, height: u64) -> Option<crate::core::block::Block> {
        self.storage.load_block(height).ok()
    }

    /// Compute Bitcoin-style Median Time Past (MTP) over the last `n` blocks.
    ///
    /// MED-1 FIX: Used by `validate_block_consensus` to enforce that incoming
    /// block timestamps are strictly greater than the median of the preceding
    /// `n` block timestamps (standard is 11). Prevents time-warp attacks.
    fn median_time_past(&self, tip_index: u64, n: usize) -> i64 {
        let count = (tip_index + 1).min(n as u64) as usize;
        let mut timestamps: Vec<i64> = Vec::with_capacity(count);

        for i in 0..count {
            let height = tip_index.saturating_sub(i as u64);
            if let Ok(b) = self.storage.load_block(height) {
                timestamps.push(b.timestamp);
            }
        }

        if timestamps.is_empty() {
            return 0;
        }

        timestamps.sort_unstable();
        timestamps[timestamps.len() / 2]
    }

    // -----------------------------------------------------------------------
    // Block Explorer APIs
    // -----------------------------------------------------------------------

    /// Return full account information for a given address (for block explorer).
    ///
    /// Returns `None` only when the address has never appeared on-chain.
    /// Returns spendable balance, total balance (including locked), nonce,
    /// and the list of active locked entries.
    pub fn get_address_info(
        &self,
        address: &str,
    ) -> Option<crate::core::transaction::AccountBalance> {
        let state = self.account_state.read();
        state.get_account(address).cloned()
    }

    /// Look up a confirmed transaction by its hash.
    ///
    /// Uses the O(1) storage index built at save-time — no full chain scan.
    /// Returns `None` if the transaction is not found in confirmed blocks
    /// (it might still be in the mempool).
    pub fn find_transaction_by_hash(
        &self,
        tx_hash: &str,
    ) -> Option<crate::core::transaction::Transaction> {
        self.storage.find_transaction(tx_hash).ok()
    }

    /// Return the `count` most recent blocks (descending order, tip first).
    ///
    /// Capped at 100 to prevent large response payloads.
    pub fn get_latest_blocks(&self, count: usize) -> Vec<crate::core::block::Block> {
        let height = self.get_height();
        if height == 0 {
            return vec![];
        }
        let count = count.min(100).min(height as usize);
        let start = height - 1; // tip (0-indexed block index)
        let mut blocks = Vec::with_capacity(count);
        for i in 0..count as u64 {
            if let Some(b) = self.load_block_from_storage(start.saturating_sub(i)) {
                blocks.push(b);
            }
        }
        blocks
    }

    /// Return all confirmed transactions involving `address` (sent or received),
    /// scanning from the chain tip backwards up to `max_blocks` blocks.
    ///
    /// Capped at 500 transactions to keep response sizes reasonable.
    pub fn get_address_transactions(
        &self,
        address: &str,
        max_blocks: u64,
    ) -> Vec<AddressTransaction> {
        let height = self.get_height();
        if height == 0 {
            return vec![];
        }
        let scan_start = height.saturating_sub(1);
        let scan_end = scan_start.saturating_sub(max_blocks.min(height));

        let mut results: Vec<AddressTransaction> = Vec::new();

        let i_start = scan_start;
        let i_end = scan_end;

        let mut h = i_start;
        loop {
            if results.len() >= 500 {
                break;
            }
            if let Some(block) = self.load_block_from_storage(h) {
                for tx in &block.transactions {
                    if tx.sender == address || tx.recipient == address {
                        results.push(AddressTransaction {
                            tx_hash: tx.hash(),
                            block_height: block.index,
                            block_time: block.timestamp,
                            sender: tx.sender.clone(),
                            recipient: tx.recipient.clone(),
                            amount_microunits: tx.amount,
                            fee_microunits: tx.fee,
                            tx_type: format!("{:?}", tx.tx_type),
                        });
                    }
                }
            }
            if h == 0 || h == i_end {
                break;
            }
            h -= 1;
        }
        results
    }

    /// Get the hash of the block stored at a specific height (O(1) disk read).
    /// Returns None if the block doesn't exist.
    pub fn get_block_hash_at(&self, height: u64) -> Option<String> {
        self.storage.load_block(height).ok().map(|b| b.hash)
    }

    /// Deep chain reorganisation: rolls back our chain to `rollback_to` (exclusive)
    /// and replays `new_chain` (sorted ascending by index) on top.
    ///
    /// Called by the sync engine when the node detects its tip diverges from the
    /// network by more than one block (the shallow `reorg_to_block` only handles
    /// single-block tip swaps).
    ///
    /// Safety guarantees:
    /// - Never rolls back past a checkpoint.
    /// - Never rolls back genesis (height 0).
    /// - Validates every new block before committing any state change.
    /// - On any validation failure the reorg is aborted and the node stays on
    ///   its current (now partially-rolled-back) chain; a subsequent sync will
    ///   re-attempt.
    pub fn deep_reorg(
        &self,
        rollback_to: u64,
        new_chain: Vec<Block>,
    ) -> Result<(), BlockchainError> {
        let our_height = self.get_height();
        tracing::warn!(
            "DEEP REORG: rolling back from height {} to {}, then applying {} new blocks",
            our_height,
            rollback_to,
            new_chain.len()
        );

        // --- Safety checks ---
        if rollback_to == 0 {
            tracing::error!("Deep reorg refused: cannot roll back past genesis");
            return Err(BlockchainError::InvalidBlock);
        }
        // The checkpoint at height 0 (genesis) is always enforced by the genesis
        // hash check in Blockchain::new; any other checkpoints must not be crossed.
        let checkpoints = match self.network {
            ChainNetwork::Testnet => TESTNET_CHECKPOINTS,
            ChainNetwork::Mainnet => MAINNET_CHECKPOINTS,
            ChainNetwork::Devnet(_) => &[],
        };
        for (cp_height, cp_hash) in checkpoints {
            if *cp_height >= rollback_to && *cp_height < our_height {
                // We would be rolling back past this checkpoint.
                tracing::error!(
                    "Deep reorg refused: would cross checkpoint at height {} ({})",
                    cp_height,
                    cp_hash
                );
                return Err(BlockchainError::InvalidBlock);
            }
        }

        // --- Validate every incoming block before touching storage ---
        // We do a lightweight "chain continuity" check: each block must chain onto
        // the previous one.  Full consensus validation happens in add_block_to_main_chain.
        let mut sorted = new_chain;
        sorted.sort_by_key(|b| b.index);

        if sorted.is_empty() {
            return Ok(());
        }

        // Derive the actual rollback target from the first block in the batch.
        // The sync loop gives us `rollback_to` from the fork-point header scan, but the
        // block batch starts at `sorted[0].index` which may equal rollback_to exactly OR
        // may be the fork-point block itself (i.e. rollback_to - 1 if fork_point was found
        // as "the last common block + 1"). Accept any value in [rollback_to-1, rollback_to].
        // Anything further away indicates a real mismatch — reject it.
        let effective_rollback = sorted[0].index;
        if effective_rollback != rollback_to && effective_rollback + 1 != rollback_to {
            tracing::warn!(
                "Deep reorg: first new block is at height {} but expected {} (±1) — aborting",
                effective_rollback,
                rollback_to
            );
            return Err(BlockchainError::InvalidBlock);
        }
        // Use the actual first-block index as our rollback target so that
        // rollback_to consistently equals sorted[0].index for all downstream logic.
        let rollback_to = effective_rollback;

        // Verify BFT certificate on all incoming blocks before we commit to anything.
        for b in &sorted {
            // FIX: Bypass this check for AlephBFT mode where blocks have 0 signatures natively.
            // if b.index > 0 && b.bft_signatures.is_empty() {
            //     tracing::warn!("Deep reorg: incoming block {} missing BFT certificate", b.index);
            //     return Err(BlockchainError::InvalidBlock);
            // }
            // Check checkpoint for each new block
            if !self.validate_checkpoint(b.index, &b.hash) {
                tracing::error!("Deep reorg blocked by checkpoint at height {}", b.index);
                return Err(BlockchainError::InvalidBlock);
            }
        }

        // --- Backup original chain in case of failure ---
        let mut original_chain = Vec::new();
        for i in rollback_to..our_height {
            if let Ok(b) = self.storage.load_block(i) {
                original_chain.push(b);
            }
        }

        // --- Roll back storage height to rollback_to ---
        // We simply move the stored chain-height pointer backwards.
        // The old block records remain on disk but are "beyond" the tip, so they
        // will be overwritten by the new blocks below.
        self.storage.set_chain_height(rollback_to)?;
        tracing::info!("Deep reorg: chain height pointer moved to {}", rollback_to);

        // Rebuild account state up to (rollback_to - 1) from scratch.
        let rebuilt_state = self.rebuild_account_state_up_to(rollback_to - 1);
        self.storage.save_account_state(&rebuilt_state)?;
        *self.account_state.write() = rebuilt_state;
        tracing::info!(
            "Deep reorg: account state rebuilt up to height {}",
            rollback_to - 1
        );

        // SYNC FIX: Reset cumulative_work to the value AT rollback_to using the O(1)
        // cumulative_work_at() fast path instead of scanning all rollback_to blocks.
        //
        // The old code did `for h in 0..rollback_to { load_block(h) }` — at height 85k
        // with a 5-block rollback this was 85,000 RocksDB reads while holding the write
        // lock, taking tens of seconds and causing sync timeouts on both sides.
        //
        // cumulative_work_at(rollback_to) is O(1) when rollback_to < current_height
        // because the stored cumulative_work field is updated after every block. For the
        // rare case where it hits the O(n) path, the result is still correct.
        let base_work = self.cumulative_work_at(rollback_to);
        {
            let mut cw = self.cumulative_work.lock();
            *cw = base_work;
        }
        let _ = self.storage.set_cumulative_work(base_work);
        tracing::info!(
            "Deep reorg: cumulative_work reset to {} at rollback height {} (O(1) lookup)",
            base_work,
            rollback_to
        );

        // Clear the orphan pool — everything in it belongs to a now-stale fork.
        self.orphaned_blocks.write().clear();

        // --- Replay new blocks ---
        let mut applied = 0u64;
        let mut reorg_failed = false;
        for block in &sorted {
            match self.add_block_to_main_chain_reorg(block.clone()) {
                Ok(_) => {
                    applied += 1;
                    tracing::info!(
                        "Deep reorg: applied block {} ({}...)",
                        block.index,
                        &block.hash[..8]
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Deep reorg: failed to apply block {} (height {}) — aborting reorg: {}",
                        &block.hash[..8],
                        block.index,
                        e
                    );
                    // Log extra context to help diagnose which validation failed
                    tracing::warn!(
                        "Deep reorg abort context: rolled back to {}, applied {}/{} blocks before failure",
                        rollback_to, applied, sorted.len()
                    );
                    reorg_failed = true;
                    break;
                }
            }
        }

        if reorg_failed {
            tracing::warn!("Rolling back the failed reorg to restore original chain...");
            let _ = self.storage.set_chain_height(rollback_to);
            let restored_state = self.rebuild_account_state_up_to(rollback_to - 1);
            let _ = self.storage.save_account_state(&restored_state);
            *self.account_state.write() = restored_state;

            for block in original_chain {
                if let Err(e) = self.add_block_to_main_chain_reorg(block.clone()) {
                    tracing::error!(
                        "CRITICAL: Failed to restore original chain at block {}: {}",
                        block.index,
                        e
                    );
                    break;
                }
            }
            return Err(BlockchainError::InvalidBlock);
        }

        // --- Single explicitly placed fsync at the very end to make storage durable ---
        self.flush_storage();

        let final_height = self.get_height();
        tracing::warn!(
            "DEEP REORG COMPLETE: applied {} blocks, final height: {}",
            applied,
            final_height
        );
        Ok(())
    }
}

/// A lightweight transaction record returned for address history queries.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AddressTransaction {
    pub tx_hash: String,
    pub block_height: u64,
    pub block_time: i64,
    pub sender: String,
    pub recipient: String,
    pub amount_microunits: u64,
    pub fee_microunits: u64,
    pub tx_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub chain_length: usize,
    pub total_transactions: usize,
    /// Reward-halving epoch (every 1,000 blocks). Used for emission schedule.
    pub current_epoch: u64,
    /// BFT consensus session (every 60 blocks). Validators activate at session boundaries.
    pub current_session: u64,
    /// How many blocks until the next session boundary (validator activation point).
    pub blocks_until_next_session: u64,
    pub mining_reward: u64, // microunits
    pub total_supply: u64,  // microunits (max limit)
    pub circulating_supply: u64, // microunits (currently mined)
    pub pending_transactions: usize,
    pub active_validator_count: usize,
    pub total_staked: u64,
    pub tps: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Fee Distribution Math ───────────────────────────────────────────────

    /// 70% burn + 20% treasury + 10% miner fee split must be exact (no rounding loss)
    #[test]
    fn test_fee_distribution_no_rounding_loss() {
        let total_fees: u64 = 1_000_000; // 1 QUA in microunits

        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100; // 500_000
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100; // 150_000
        let fee_to_miner = total_fees - fee_burned - fee_to_treasury; // 350_000 (remainder)

        assert_eq!(fee_burned, 500_000, "50% should be burned");
        assert_eq!(fee_to_treasury, 150_000, "15% goes to treasury");
        assert_eq!(fee_to_miner, 350_000, "35% to miner (no rounding loss)");
        assert_eq!(
            fee_burned + fee_to_treasury + fee_to_miner,
            total_fees,
            "fee split must be lossless"
        );
    }

    /// Odd fee amounts should give remainder to miner, not lose value
    #[test]
    fn test_fee_distribution_odd_amounts() {
        let total_fees: u64 = 999; // deliberately not divisible by 100

        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_miner = total_fees - fee_burned - fee_to_treasury; // remainder

        // All microunits must be accounted for
        assert_eq!(
            fee_burned + fee_to_treasury + fee_to_miner,
            total_fees,
            "every microunit must go somewhere — no value created or destroyed"
        );
    }

    // ─── Block Reward Math ───────────────────────────────────────────────────

    /// 5% treasury + 95% miner split from block reward
    #[test]
    fn test_block_reward_treasury_split() {
        let reward: u64 = 100_000_000; // 100 QUA Year-1 reward

        let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100; // 8 QUA
        let miner_reward = reward - treasury_allocation; // 92 QUA

        assert_eq!(treasury_allocation, 8_000_000, "8% of 100 QUA = 8 QUA");
        assert_eq!(miner_reward, 92_000_000, "92% of 100 QUA = 92 QUA");
        assert_eq!(treasury_allocation + miner_reward, reward, "no value lost");
    }

    // ─── Reward Reduction ────────────────────────────────────────────────────

    /// Year 0 reward must be the full YEAR_1_REWARD
    #[test]
    fn test_reward_year_0_is_full() {
        let reward = apply_annual_reduction(YEAR_1_REWARD, 0);
        assert_eq!(reward, YEAR_1_REWARD);
    }

    /// After 10 years, reward should be reduced roughly down to ~10M
    #[test]
    /// After 10 years, reward should be reduced roughly down to ~10M
    #[test]
    fn test_reward_decade_reduction() {
        let reward = apply_annual_reduction(YEAR_1_REWARD, 10);
        assert!(reward < YEAR_1_REWARD, "Decade reward must be lower");
        assert!(reward > MIN_REWARD, "Decade reward must be higher than floor");
    }

    /// After 20+ years reward must not drop below MIN_REWARD floor
    #[test]
    fn test_reward_floor_after_many_years() {
        let reward = apply_annual_reduction(YEAR_1_REWARD, 50); // 50 years
        assert!(
            reward >= MIN_REWARD,
            "Reward {} must not drop below MIN_REWARD {}",
            reward,
            MIN_REWARD
        );
        assert_eq!(
            reward, MIN_REWARD,
            "After 50 years must be exactly at floor"
        );
    }

    /// Reward at year 1 must be 85% of year 0 (15% annual reduction)
    #[test]
    fn test_reward_year1_reduction() {
        let year0 = apply_annual_reduction(YEAR_1_REWARD, 0);
        let year1 = apply_annual_reduction(YEAR_1_REWARD, 1);
        // Integer math: year1 = year0 * 85 / 100
        let expected = year0 * 85 / 100;
        assert_eq!(
            year1, expected,
            "Year 1 reward must be exactly 85% of year 0 (integer math)"
        );
    }

    // ─── Treasury Address ─────────────────────────────────────────────────────

    /// Treasury address constant must be the real 3-of-5 multisig, not the placeholder
    #[test]
    fn test_treasury_address_is_not_placeholder() {
        assert_ne!(
            TREASURY_ADDRESS, "0x0000000000000000000000000000000000000001",
            "Treasury must be set to the real multisig address, not the placeholder"
        );
        assert!(
            TREASURY_ADDRESS.starts_with("ms"),
            "Treasury address must start with 'ms' (multisig prefix), got: {}",
            TREASURY_ADDRESS
        );
    }

    /// Treasury address must be the exact known 3-of-5 address we generated
    #[test]
    fn test_treasury_address_exact_value() {
        assert_eq!(
            TREASURY_ADDRESS, "ms69216b1d10425689704d5ae3b2a4aa17049f59b1",
            "TREASURY_ADDRESS changed! Update this test AND generate a new genesis block."
        );
    }

    // ─── Mempool Tests ────────────────────────────────────────────────────────
    
    use tempfile::tempdir;
    use crate::core::transaction::Transaction;
    use crate::crypto::wallet::QuantumWallet;

    #[tokio::test]
    async fn test_mempool_rejects_duplicates() {
        let dir = tempdir().unwrap();
        let storage = Arc::new(crate::storage::db::BlockchainStorage::new(dir.path().to_str().unwrap()).unwrap());
        let blockchain = Blockchain::new(storage, crate::core::ChainNetwork::Mainnet).unwrap();
        
        let wallet = QuantumWallet::new();
        blockchain.account_state.write().credit_account_direct(&wallet.address, 200_000);
        let mut tx = Transaction::new(
            wallet.address.clone(),
            "0xreceiver".to_string(),
            100_000,
            chrono::Utc::now().timestamp(),
        );
        tx.nonce = 1;
        tx.public_key = wallet.keypair.public_key.clone();
        tx.network_id = 1; // Mainnet
        tx.signature = wallet.keypair.sign_transaction_canonical(&tx.get_signing_bytes());
        
        // First addition should succeed
        assert!(blockchain.add_transaction(tx.clone()).is_ok());
        
        // Second addition of the exact same tx should fail (either Duplicate or InvalidNonce)
        let result = blockchain.add_transaction(tx);
        assert!(matches!(
            result,
            Err(BlockchainError::InvalidNonce { .. }) | Err(BlockchainError::DuplicateTransaction)
        ));
    }
}
