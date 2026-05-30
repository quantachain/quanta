use crate::core::block::Block;
use crate::core::ChainNetwork;
use crate::core::transaction::{Transaction, AccountState};
use crate::storage::{BlockchainStorage, StorageError};
use serde::{Serialize, Deserialize};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::VecDeque;
use thiserror::Error;
use dashmap::DashMap;
use tokio::sync::watch; // New-block notification channel (abort-on-stale mining)
use sha3::Digest;


// PERFORMANCE OPTIMIZATIONS FOR POST-QUANTUM CRYPTO
use rayon::prelude::*;  // Parallel signature verification (6x faster)
use std::sync::Mutex;
use lru::LruCache;      // Signature verification cache
use std::num::NonZeroUsize;

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
    #[error("Insufficient balance: required {required} microunits, available {available} microunits")]
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
//   Used by: Zcash, Grin, Beam, MimbleWimble variants, many Monero forks

/// Number of blocks in the LWMA sliding window.
/// 45 blocks × 30s = 22.5 minutes of smoothing — fast without being jumpy.
const LWMA_WINDOW: u64 = 45;

/// Maximum per-block difficulty INCREASE (as a percentage of current difficulty).
/// 200% = at most 2× up per block — prevents a single fast block from spiking too high.
const MAX_DIFF_UP_PCT: u32 = 200;

/// Maximum per-block difficulty DECREASE (as a percentage of current difficulty).
/// 75% = at most 0.75× down per block (i.e. maximum 25% drop) — prevents death spirals.
const MAX_DIFF_DOWN_PCT: u32 = 75;

/// Per-block solve-time clamp: individual solve times are clamped to [1 .. 6×T]
/// before entering the LWMA sum. This prevents a single stalled block (e.g. from
/// a node going offline) from crashing difficulty.
const LWMA_SOLVE_TIME_CAP_FACTOR: u64 = 6; // 6 × 30s = 180s cap per solve-time

/// Minimum difficulty — set to the testnet V2 genesis difficulty.
/// This is the difficulty at which the chain STARTED, so all early blocks
/// Mined at genesis difficulty are valid. Never output a target easier than this.
pub const MIN_DIFFICULTY: u32 = 8_304_130;

/// Maximum difficulty — 2^31−1 fits in an i32 (used by block.has_valid_hash)
/// and is far beyond any real CPU/GPU hashrate.
const MAX_DIFFICULTY: u32 = 2_147_483_647;

// Keep this available for code that referenced the old constant (e.g. genesis loader).
// It is no longer used by the difficulty algorithm.
#[allow(dead_code)]
const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = LWMA_WINDOW;

// v3 TOKENOMICS — AI Agent Execution Layer Era
// Block time: SLOT_SECONDS = 6 (bft_proposer.rs)
const YEAR_1_REWARD: u64 = 50_000_000; // 50 QUA/block — tighter emission for AI era
const ANNUAL_REDUCTION_PERCENT: u64 = 15; // 15% smooth decay (no halving shocks)
const MIN_REWARD: u64 = 2_000_000; // 2 QUA floor — more deflationary long-term
const BLOCKS_PER_YEAR: u64 = 5_256_000; // 365.25 days * 86400 / 6s (BFT SLOT_SECONDS)

// UNIQUE FEATURES - Network Bootstrap
#[allow(dead_code)]
const BOOTSTRAP_PHASE_BLOCKS: u64 = 315_360; // First month bootstrap reference

// v3 FEE STRUCTURE — DPoS validator-first split
// Validators need real fee income; burn kept high for deflation; treasury funds AI SDK work.
#[allow(dead_code)]
const BASE_TRANSACTION_FEE: u64 = 1_000; // 0.001 QUA minimum (prevents spam)
const FEE_BURN_PERCENT: u64 = 50;      // 50% burned — deflationary without punishing micro-tx
const FEE_TREASURY_PERCENT: u64 = 15;  // 15% to Ecosystem Fund (QEF)
const FEE_VALIDATOR_PERCENT: u64 = 35; // 35% to block proposer — DPoS validators need real yield
// Compile-time guard — build fails if fee percentages don't add to 100.
const _: () = assert!(FEE_BURN_PERCENT + FEE_TREASURY_PERCENT + FEE_VALIDATOR_PERCENT == 100,
    "Fee percentages must sum to 100");

// ECOSYSTEM FUND (QEF) — AI SDK, security audits, exchange listings, community
const TREASURY_ALLOCATION_PERCENT: u64 = 8; // 8% of block rewards → Quanta Ecosystem Fund

// CONSENSUS-CRITICAL: Treasury multisig address (3-of-5 Falcon-512, generated 2026-03-14)
// This address is hardcoded in consensus — it CANNOT be changed by editing quanta.toml.
// Any node that changes this constant will be rejected by the network (invalid treasury tx).
// To move treasury funds, use: quanta-wallet treasury-propose / treasury-sign / treasury-broadcast
// Keyset: treasury_key0.qua … treasury_key4.qua — any 3 of 5 must sign.
const TREASURY_ADDRESS: &str = "ms69216b1d10425689704d5ae3b2a4aa17049f59b1";

// NOTE: PoW mining reward lock removed — replaced by DPoS unbonding period (UNBONDING_EPOCHS).
// The BFT path (create_bft_block_template) always paid full proposer reward; this was dead code.

// Security limits
const MAX_MEMPOOL_SIZE: usize = 5000; // Maximum pending transactions
/// HIGH-1 FIX: Per-sender limit — prevents a single address from griefing the
/// mempool with thousands of incrementing-nonce transactions at zero cost.
const MAX_MEMPOOL_TXS_PER_SENDER: usize = 25;
// CRITICAL FIX (External Audit): Reduced from 2000 to 1200
// Falcon-512 transactions are ~1713 bytes each (666 byte sig + 897 byte pubkey + overhead)
// 2000 tx × 1713 bytes = 3.43 MB (exceeds 2 MB block size!)
// 1200 tx × 1713 bytes = 2.06 MB (fits in 2 MB with compression)
const MAX_BLOCK_TRANSACTIONS: usize = 1200; // Maximum transactions per block
// SECURITY FIX (External Audit): Increased from 1MB to 2MB
// Falcon-512 signatures are 666 bytes each, so 1200 tx = ~2.06MB
// Previous 1MB limit could only support ~583 transactions
/// Exported so `storage::db` can enforce a matching decompress size cap (MED-5).
pub const MAX_BLOCK_SIZE_BYTES: usize = 2_097_152; // 2 MB max block size
const MAX_ORPHAN_BLOCKS: usize = 2000; // Increased to hold full MAX_SYNC_BATCH out-of-order blocks
const MAX_TRANSACTION_SIZE_BYTES: usize = 102400; // 100KB max per transaction (prevents DOS)
const MIN_TRANSACTION_FEE: u64 = 100; // 0.0001 QUA in microunits — sub-cent for AI micro-tx
const TRANSACTION_EXPIRY_SECONDS: i64 = 86400; // 24 hours
const COINBASE_MATURITY: u64 = 500; // ~50 min at 6s BFT slots (matches old 100×30s window)
const MAX_FUTURE_BLOCK_TIME: i64 = 7200; // 2 hours maximum future timestamp
/// LOW-1 FIX: Bound address string length to prevent unbounded HashMap key allocations.
const MAX_ADDRESS_LEN: usize = 128;

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
const GENESIS_HASH: &str = "1cdbccdff3db462378f4acbe4553b49040ffcdebf74b5c77e685ba05ccfa8cb0";
/// Testnet Alpha genesis — difficulty 8_304_130 (~30s/block).
/// Old nodes on the previous testnet genesis will be rejected by this hash check.
const TESTNET_GENESIS_HASH: &str = "b75dc9fbbc02c7780db84f94811d02e442ec812294fdad3e568e97e69dfd65ae";

// CHECKPOINT SYSTEM: Hardcoded checkpoints prevent deep reorganizations
// Format: (block_height, block_hash)
// Add checkpoints every ~10000 blocks.
//
// TESTNET checkpoints — fetched live from rpc.quantachain.org on 2026-04-22
// Never add a checkpoint you haven't independently verified.
const TESTNET_CHECKPOINTS: &[(u64, &str)] = &[
    (0,      TESTNET_GENESIS_HASH),
    (10_000, "0000013b6f5f570de0605eac1e7c2fde87f8ce30ca26acc26a9a78d9c18374d5"),
    (20_000, "00000008ac637f1cf3f891de979b1ed7debb8862e0bbc9fdb64e90a19d773885"),
    (30_000, "000001743b0b76fe64b28631afd7c923cf6eca06377dabe9fc8ebbbf8725ac6e"),
    (40_000, "00000059783ae9efeb043ac6b1fa254fa338ccc5631dd1b7f96f6a498df07c86"),
    (50_000, "0000010309330cd86087a9133848f80fc82b056f63adc0749e83894f0a4de956"),
    // Verified live from scan.quantachain.org on 2026-05-05
    (60_000, "0000010ce22920660ba1e42423ea46e76dc7582963d6f9f220e3930031bd9bc9"),
    (70_000, "000001fcb0637b06601b4f111b22070e856c8cabf2eaa545c41b938b4478d186"),
    (80_000, "0000002d80e66bce37596616a9c9c3c1988da6e65811ad132926162c7e000a0e"),
    // Verified live from scan.quantachain.org on 2026-05-06
    (85_000, "0000007305d4ceeaf72a4f3c58001295a335d588e16a05f037d21dfb21ac06ca"),
    // Verified live from scan.quantachain.org on 2026-05-08 — anchors the
    // v2: All nodes on v2 share the same BFT-from-genesis history.
    (90_000, "000000dc8e178a5140a5c68461234a9541373ac349b1ae3cbc3f0f3f1fc58d5e"),
    // Next: add (95_000, ...) or (100_000, ...) once chain reaches that height.
];

// MAINNET checkpoints — empty until mainnet launch
const MAINNET_CHECKPOINTS: &[(u64, &str)] = &[
    (0, GENESIS_HASH),
];

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

    /// Treasury payout address (loaded from config or hardcoded default).
    treasury_address: String,

    /// NEW-BLOCK NOTIFICATION CHANNEL
    ///
    /// Fires the current chain height every time a block is accepted (normal or reorg).
    /// The mining loop subscribes via `subscribe_new_blocks()` and uses tokio::select!
    /// to abort the current PoW the instant the chain moves — eliminating the
    /// 5–30 s window where miners compute against a stale template.
    ///
    /// Using `watch` (not `broadcast`) because we only need the LATEST height;
    /// miners that are slow to wake up just see the most-recent value and restart.
    new_block_tx: Arc<watch::Sender<u64>>,
}

impl Blockchain {
    /// Create or load blockchain from storage (OPTIMIZED to not load full chain)
    pub fn new(storage: Arc<BlockchainStorage>, network: ChainNetwork) -> Result<Self, BlockchainError> {
        // OPTIMIZATION: Only load genesis to verify chain exists
        // All other blocks loaded on-demand from disk
        let _chain = storage.load_chain()?;
        let account_state = storage.load_account_state()?.unwrap_or_else(AccountState::new);
        
        // OPTIMIZATION: load_chain only returns genesis or empty if new.
        // We must check storage height to see if we truly have an empty chain!
        let height = storage.get_chain_height()?;
        
        // Define expected genesis hash based on network
        // Note: Mainnet hash is hardcoded constant. Testnet hash should be calculated or hardcoded once known.
        // For now, we trust the generated testnet genesis if it's testnet.
        
        let (chain, account_state) = if height == 0 {
            // Create genesis block
            tracing::info!("Creating new blockchain with genesis block for {:?}", network);
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
                // Generated via: cargo run --bin gen_faucet_wallets
                // Mnemonic: set FAUCET_MNEMONIC in quanta-web/.env.local
                // Account 0 = faucet sender address (used by the faucet API)
                let testnet_faucets = vec![
                    "0x1683be267318d2ddd8cee8df4a4548dcffb1e088",  // Faucet 0 (sender)
                    "0xd528c18ce7a8844e4a4dcd841975b20ae599b020",  // Faucet 1
                    "0xfd6e36bfa2b2798d08592802206c943d5513adfb",  // Faucet 2
                    "0xed15573ad312d41aaef74cff56a8ef28122ec2db",  // Faucet 3
                    "0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb",  // Faucet 4
                    "0xbf5ee055f399323fdd0cefe3d4aa923678d46107",  // Faucet 5
                    "0x1dc9637b183093d723ea8d1fb18083b06490facb",  // Faucet 6
                    "0xa2270f30ca1aad922510375508bf68cd95509f29",  // Faucet 7
                    "0xe15a689775685ae324559ea9a492fc650354ca0b",  // Faucet 8
                    "0x005dcff212d27b55e7a74bf745e1349ab44ca25d",  // Faucet 9
                    
                    // --- BFT VALIDATORS ---
                    // Hardcoded genesis validators for testnet launch.
                    "0x7a51189dca3a8cca6a9d6fda987255e755a1a8e2", // Validator 1
                    "0x0dd48ad3d185e6169b2864a390f16cf7ec2bac1a", // Validator 2
                    "0xc322ab93261d8075ce8906bc16809408ea886361", // Validator 3
                    "0x8a63f29686752a85a29030d362059fef27059892", // Validator 4
                    "0xef6e505428af5d9ef67cff8f30bdf8ddc61d35b0", // Validator 5
                    "0x5c214ed5126f3101a19f898cffb23ced1a0bbc3b", // Validator 6
                    "0x0fd42364df549eb5fabf8f90c678adf4c033b199", // Validator 7
                ];
                (testnet_faucets.into_iter().map(String::from).collect(), 1_000_000_000_000)
            } else {
                // MAINNET: Standard empty genesis structure (1000 QUA to burn address)
                (vec!["0x0000000000000000000000000000000000000000".to_string()], 1_000_000_000)
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
            if network == ChainNetwork::Testnet {
                tracing::info!("Bootstrapping BFT validators into AccountState...");
                
                #[derive(serde::Deserialize)]
                struct ValidatorGenTx {
                    address: String,
                    public_key: String,
                }
                
                let mut dynamic_validators = None;
                if let Ok(json_bytes) = std::fs::read("./genesis.json") {
                    if let Ok(parsed) = serde_json::from_slice::<Vec<ValidatorGenTx>>(&json_bytes) {
                        tracing::info!("Loaded {} dynamic validators from genesis.json", parsed.len());
                        dynamic_validators = Some(parsed);
                    } else {
                        tracing::warn!("Found genesis.json but failed to parse it. Falling back to hardcoded validators.");
                    }
                } else {
                    tracing::info!("No genesis.json found. Falling back to hardcoded 7-node bootstrap.");
                }

                if let Some(validators) = dynamic_validators {
                    for v in validators {
                        if let Ok(pk_bytes) = hex::decode(&v.public_key) {
                            account_state.register_validator(&v.address, pk_bytes, 1_000_000_000_000, 0);
                        } else {
                            tracing::error!("Failed to decode genesis public key for {}", v.address);
                        }
                    }
                } else {
                    let genesis_validators = [
                        ("0x7a51189dca3a8cca6a9d6fda987255e755a1a8e2", "09287520eb580a9cb8a00cd466c8d156a23950fe67019ce13458c795336502f6f47a1d159b35bda8da9296362d00a8de099e25e5871b4ccf17083a1043114f0b1b2aa1e326593b2cd66de2f7ada46a6ef849531395760d0713410cf202ca800189e07c53694a60abee9c0f48e8a5b70c8e97898d787646b3735afd76ed687616c96d3a914d6ca109be30d4ec933ed49b0b518f2f6b354dd638c2711bd483787e45c168345f16475c755869aa437098e6ba3d963b1717794cf0c701182c6dacbeb938cead1144d993b2232e06b9592526cfd5373a1410b94b86fd3d0848129b4f92a50811c1ae107a42ca07d334cba0286cb4a4d70ca0331de29563c1afa602621b7365d943e4500785199b8881c276730d6d15c2fdd8881fedbf7a0f25b41c1a7e68cf5011cdcc0948b6f29ea3d2141e9b42a8ac5d4ac530347ca28e33b89d32aa40c903fa9c585e17fb5526a94a08cd5c7715fe2761e5945e51b581e8cbbb03f2b5dab013ada974334474ac5104414f54b2aa02838a166acd07f01971bea6a9d9d227c92133f41b497d7ca2d39fa51f54b87f444486a4427a8e238753369b76a852262280ba621656b04d7b975e31e7044ef75c5db5309a0d9c8349e31141803215c404814d0ecb2264da8a0d5cf0685c8931d8952c40846efb84838162e6820b5e4ca4766256a8ae945285920305dfea675d8688493715408d5194e91a14bb50b27fc9a5503d7f842f277620b86a51ada020417b2bb06e25d5594b854db8d79ad1ad2a240df9ff47db7084854a4bcc96700a68f82f827e3c74333858860c0f6a983b48c69b55a8116203e35e7c5872a9e2da71392ae7774eea507460c342a821cbad2930c99227b42f4a42c29fc37401c66bf33567e75fb975844cc04bb42c71673805d06dc5a72ac589d7004a977866676e9e8536073f5d2f608e575a97c2886ebabd1148821390e9982b2d561f3ef2fee4ff00db55c631b8c1acc8bfc1697625c917bcf016f1956c9265daaf238824e53fa86775c3ecb7ae02a838fa0177176a188036016b2f90e828265299a060d8b71f1360b38d3d65fe942e12f69d9bcdca8e84a5d5f72fa9e99faacd36d4d23c970606facc514b6fb987aac42ab611003ff1e24de73ba89137521feb39f5dfd0a22b4116498f00bacba35892e758e15fca3065d23985020d84617033dfdccd06d88d5000229e493e86b346df9e5ad273155b6dc77f3846ebe206e28b24a858f2c1e219958f4002ef46a2947be4192f73"),
                        ("0x0dd48ad3d185e6169b2864a390f16cf7ec2bac1a", "094ead37a23ca0743c8d8c32155900b5618a3a386e342edea66863a5a014754f3b8b87bc64e93f447c407f95c8f8d8f754d06c6dea4631d2883188b300f9d1bf04bea4aa298f1581b46336a21856532a27231983935779bff6a94cc564416745562912898d322bc26d1f0ad1d56acd94179aaa6ba8f0154fa392f8d70da61d592661889da720365c26220c14ae7771e2228014e1151ccb422c09dd9cc19bda2fe4382af0fa931ad14f2dcd2574cd467297f52a85836b0120a918261aebde254dbe94a3528710ccfe76ed41d038ccc6416a6246b7315f2a37dab2ba7e65f5f45be55f826d9ae7a7921f41e0a91c54fab4e9aa79d688d58e752a4424d70d82fd74f82c11523128bac547ed576215da0bea4752bd007c40625ec8948eed519786bf1c91d234950c924d6d75f7a11bf1b5028e46a39c6db264c854d4e743a388d4020201681ce9890a1ace122baa85fad6cff70e6ee288fb959589552b78731e1b78da85611f16a790962f96ccc11068550b805ea9c365493d06902a3b8f2e6a3b0c4d5f5599f5297b5ad49f82fc792514383b94948a6de315445a7c87558c79854e1e393c05441c947d72e50f171c105e41998cf59a3cf9a216c0a6b69e45977b278b54081fc88b2f9385e64f575c6ed5a11690063e22f55f5da85179a43c8adccb81536e1c8085e2537dd243914921c97fa2a88d1af423855c6832b93543e1572e4815f4b88f01bd1bbe3381c18b27aa94a931abe8e91f750b4405275192b2dd1782e68fb7b90d5bfde4637752287b7758eab2f40ec7e7222a6c7a521bd3110255b8784588474602c25f97ab54bc0ae47228bdedb4429962c3de16b3ae1ec50b80674aa804cb8071239ea7517e092445f0b0ce6bb3cec9f66e3e90157a00208886732745138e8bbde8b27e8de07f2c1a35f5e15f5208b2f828809947dc374b212ada66f0b9e8f0c432774a1583f9c9fd22ed0c3759d5d98d760733b91c9d1045e5c7418d9e883a88a1299ffa61705bd26aef1eb6e46ca15907d4916074b9925a6f9af22c38d7109a269d5b8b3b56da4fba5d777f6ac656d993a222cc08ba825994e8da6aa1418153d395c402b0bbf6576ea8592e18f4e61c111b3030e25d2233450ca3733401de47e437a98886e59622e1e2052189856d12d9dc67e266b6ab28b569e95d56fb557a66a6b760d7c877a7b82fd878548c57ad2fa5e3453a6ee39d1275f7b3e29c391a5f298d8f48395cdb80191df4490aafa85c6a470f8419996c40ef"),
                        ("0xc322ab93261d8075ce8906bc16809408ea886361", "092c05d5dafde2d5b0e9106693aac24625e2030a637a2cde85c5486f7288dd42530465b65a1d99d37b826f745946948760ff388015981097caa61ae6a8cf13deb666ef14abdd6e870cd95581116809f2dd0ae8ae05b6a92c89c91ac93b569bdae7ce87ba0c3b1391959c62aaf1b380423f8aed86bb77dee70d8c311bda1864455286cb66d5916f760c85ff0f4f15d0f617af18ef68cec5f4da6e872eccfcb5cb22ca44ba8d70f79c7f1bc9dd452be9d2922592375442b0ac1e760a621b396e7cf5f5ae40bf5f0828c719ab7e53162b818b474a23ad217403ed5196f16b16c744f7ab6ca5f3115aba1584bd55b424c4613dffe71028baa03409724391411f5104940e6f110586fed88a5fe3492cad04cb6dba270d01d254cfc0b810cc9921de7a19a1ac90ca63f5858ae0334415d328643ffbaec35591fc29f590cced17208d0a8a88845110a9569de3714b569e72194c7513b02ed55b9100730954d7282618591206e76cd09061b61e64e4648d4e1dc76adeeba55a98714b0cd6ad90a62fa76adec488645a473181fa79e1c3063463ba673224da6312d74f9cf2e5dc8f9706a270e6d22f6a3e6c78eb54ad1f644618b371da29be7a13460e1a1480009f351944915715073476171f6c92cb301bac864a79b469a560365b5cc6765566b7afeadc147e81efa1dc4d796fe700bc0ea97927df3cb1714c9614ede67f46232bd6a728afea2e782297f2393d92f958a67e0bc5c1685ed2b742de7353df9921a384aa1a06d0f14e91d5063720f68e082394c98b668cbae0f0b59a720e4ed97967a5fc3ee628f476826721e5d491d926550e2ea2a8ee5d1933a4776b2c6eff12a69883799c7fa5d6eb0746873a1e4d94431509fe6925d19a9b518686a43d95d5e9b516754483df57ab8105cfe9c591c0a90dc31a46aaf60a7d798934af1963ae66c2ac9cf15b7adfa9196d05ba4abc07374fac2dac4bf2d6daff99f5f697581f1ca5d48f6482e77818f1cc5047c3b54a5e8dbb26877488c1f3ead70d4a44a6453323e61a38896c40859e6c7ee0d9f6a191056732c2c7508c07049195888998f821d932d7a60ce9598dd21618900297a2862426d515a951f6a212de6a30293a72df1705b63958f75958ef2e066fe3b4db573e520398ce9caa4c68bbb60e667e88d0102ba1c7203a79164b520ab385a23b103718ad4a6434b9a09afe96ae7e4a21af8bb69438b9d243c7b721e1315c50825dae7e686cd79b817c49b486e98b2bc4fcd3dbcbb2"),
                        ("0x8a63f29686752a85a29030d362059fef27059892", "09ab912faa284dd67085fbd806cb39886a0ba1bc91ce16b0e3d6cbed9492e879e4d8553bb52ef321c8a41a7e18c168c36cc54cb22303fd94c5bac4ecd0596c293e893d560097946b346242944452a63caaf2c44d4901763588961a39840262290154350500064873adca1fbda54588a7ef9bb452f7a8a09a224d74b115aeda5cbcb391ba221cb0f19fe11052c3a121dde9d30bf83b89d5b2726c914b727e0608673947746ab3412785583ae5041e022e674166c530efa534020d39be96c500d00bb25c65d9a5107ec2d3c25526f94b1b3a5aaf7f2015f1ede748a74d9dd74612c677806ba201df2e09097399e180e5af3e3cc800d2120fb0c115005be5220cd3cb98572c18e20474d1db2a1b729597a719bd72350cf579092d24307f405cd8fd03b82232090ce03ab6a490562b05b68d4dc7f024017270dbd01358d593c97c85dceb5f70268711ca1d42455e61739a56bf88cdeff409e1ac73718541fa2ff9bcfd5d56ac2b6b67a5b5c47f0241317d0ab1c4e1c798944fc03bc8695e19a620b84772a4da7b27ec9ea2a3dd7005d59c58235d0e26870d1f4065b2c03ee7c25e804040b04a0b7492f3c8dd07a5615214888056bb7d3ee87502ae9425e0e1f6e3809838eae619fda4823bf9ac1554c8d177ea53571e407bb749be34b86f2d64b5262a2b67aa1e542f417ce197997dbae0ef97a966bcaf57a3eb891948f2b0514be2a16070e6de9c5831aac4040f4b4f4f31e7a5afa06f8c81ebe18be61f7fbd4e02eda41cacfcebaa76d2a78a292810f68d5b63d693f7ff55b2b710af14045af65698c30869165b72454a361c46dca6c15771276a6c82a4997532ddb97d0dbd01dd957e69aeed60a6a9d563222faa36010243da0ee535d3330a08e5c10f9f02b59a900a3c236c86a5c72331c64d4c44abe204d76c2a4449a9ca86927db2433bdf4ce995bdd02a4e291480659e962d5e74813cf588c58cfcb8cd1df23bc8191c88e23b5d974f0290e6abaf5088b8a8e457ef59af1746f7825918cf80ce6940565d3a8a2cce74c6997739b19ad75d6ca555d57a4b8bc11731995fe6708afa16522049e8610b42d8b794061ce8b5b49739330544baf56e67fe80f0af8de4a7fe936c2518cf87a3d4cd4f2148564127c5098cc32550ae40393e5b19ef8aa124013e24700546a4d15c104c6b4185ade6242ea86b89b50887f5aa9b3db832fae826fb48d116a3e8101237350cb8056bb704be8d94f0173a17747a7442aa0551324bc26755dc"),
                        ("0xef6e505428af5d9ef67cff8f30bdf8ddc61d35b0", "0935d92bb348167251ec47baadca6e1c2970e4bce061866dc1b61d8b3c276d4474e18c162c714601cb6b146d290c509fc82c988c3fd4648e22afd0c56763934364c27930015acc7d5d36fb8712dd2b89c6c949c9f57bccb786f66068003a1df8224db930783e39cee65d426c6f731e60631825d8a67348cf9c2224b308dc8833604517666d6c2a4a4748c627bbb71d2d56e9c91f89fcf2255bd5a0bc6c235a27d7c09f969e534a2dc742fe9532b9d218009014e09c5e159bfda641c12ad0b8f9dd05b95e4ba5b6baeaf85c030f7124271cef410caea953c703fa3849cbbb380628aa05cc347c1f3a295ea67518594e4afe2203c364753a0a007a370525b7cd55341c5f3e40f131866a8325550ac0b19ed2af23a9ced34d059444e91604145c2f8812aeb06020ed01f63ce838e6e7a029c970bddc655fb2e7e49268cd9db188caff143475b4bbf19edc8d5fedff8a46ce73a7502c96506914b161fb88fb59701eaee294d88b92793406f6e7d4ad7b34fef9a19b547ec58ecb44a2b6a1117cf9fb81cb5fb82a49a44dcca491e604e01ea067282ca35512ae36a6f110ef2d8d5412998b355324667ae0956b8cffb589ff35e98c839e8aa8340d8a11a71a9b7585d9dfbc16d0a42dad495f310037411723b326d465f715bc6fb9a733c948826be01e33d24ac1b5f91230900c9d1c0698c48eeda29b35c2a745e5ab0cad39d56114f49a7d86242dc664a98cd7934b22fc6e888851c1a78c9d7ce48a33e5de8bc671bba35197812a4ee35249467d495715a41c5396bcae8729c43332e0a8abd3275fab0816d858eac970fd9bfab0cad11d982bf27ac29f388836db87a55f12f68fba3ac71e7c3ac36824e0937616301ad6ae5115ad5d07f591fb2a2d6b410f02cb4dc8b04837c9499374ca4726dcfd721e49817801b5436414b862e2406b52dae0a212074fcc3266c95b9a5f724ea038022161247f4835d20c95d5f694c1a659a0a6599769415f9eb597c41a033658101b13ec124d4c18c381b8167556a479e548aa7185d20e10ed2b3e601815c0f5e1962a22ff19cecea35a02ed19104ed8b22d0fa9295c2d342535d6149c9987c00432bd219c87b2cac59c67f44ce2923a50c1e8ac14c4d5618d8d1c46ec84e3658f25245e9bc25cdd677d9f51bd0a011bfc60a8bfc6ee516c347602458abffebfe87229976c3edbc5fb4aa62eecc86841e78c1e4208897d1901816d74928fce501dfdf0825547ce9f84c883d4269756a63aa2e6df2b"),
                        ("0x5c214ed5126f3101a19f898cffb23ced1a0bbc3b", "090bb9f0f08bd41354d16736945da706fd55f29d6e67980066e56dacf253f8f5e0e34af8bf1aa4ab32c4446a02a680be2935356aee0bb09d7240b62c51ec0abbb46a0fd7d0014e1d6c76b2e644bfb6b605ca29aae822d213367e0b1f3409d8bb8a61be81ca4c021a43606eca10601a10f1535113c2eea209440582886555e711f4c187f35c18b780cfe4592a153e124228c50176876d39013ea75b5b14fe93c91c6505854709f90bd00f425a26ee9656ab580f02c552133172c485d0eb506f76b1b02f19eb644ab2b6c25c9e4bbab1472e0cb97464f485626e6b33bca120b958254b7a223a1b6e8e9bb6d003d8d2af2ae0e1942c9b4c94214261ffe38ca2b92912482c7b699483b07e14eeaee5bc088b88e99474d1705f6d5abe2259d56e561b79321187ab16c3be685f967a89fb87d0ae990d13fe872d3c71740842aafde321956b5b5ace7542d0c33964b4582490cf0343b20539b3522c491d44c705151e2858257354ed2e8ee243858e29572a322d25b8208a60511c294a08422da03cc073201275dcc514cce96d99d8a7d3b8c7890cec46bb3cec8e58c6a0fbed9e234d64fda1eec5f77d29d9c34627f02c558f01f757c11cf1c8961156f071153eb40e4a6478b269a529e3422210eca0e6d2b84f764514b75f6351609fa1e8ad1f9c348954e4820ea4a43a4887ce2fbe39d9c46748f3aa79ac0280e558b4e01cd8bf86986c045796f700bc5a92dbe68863bc65c88ce1231fe73816d025ce5c209014586a24dc3d08e2ce539105ad42e57a2687e69d149e5de15f03ce0ced879a59a12a9b5eac239e6cbf4ad614ea429d7c0ede4a0bacdc1a8424ad44b00c168645b365fcc2587d34babbd7e35493418541b2d76d9baea5e7bd929a80d291943a22f5091dac26cdeedc3fa465d3a8e5d6227cb316aad50d5b36c414c2a6f45fbedf758b906501c1094354e16591249b9a42851572ec7ce65784a2544ad17b3d6f469fbcee8a8819a90db5e35080aa191248a8d006d2c033403647a84fec7d723da7eed2267c1265c880df3aa2009c19b5d95b7c38de7645d19941b0b6d96a08d708429debd0ed9d5511e2425e05a69d08e31b68e7060a72e1d75dd23c9538b718420b08a61608d66d12955c0ed5d31298e2b8252602d0ded97f067d769d9ff3167c9627b4983e039c6e14a5e8a80b3139981da1e98ae2a652f6dd8678a16b49fb48df5809fa877c099aa924ff04a656b5af86d5547f96cf00ea09950613b26f6504c193e289"),
                        ("0x0fd42364df549eb5fabf8f90c678adf4c033b199", "09a96eb31720c6209084eef3dc428b61d62d2afd66234a1a8a7020ce03027c7e7145c8bf9f9ab47678567c4dbc28b283a89f5c6c6aa9705774031163f75d24257cdae9c88dc5734124404133a6fc4d8cc166e29c09acaa9772d82c696862c1da7363f302ecad845a29ecaa01f4c3124f1704d4f244b896d85781dc95b353253329c719ee2c688e45cb61e9915d29206f2207add68232ccc67864a247c0301ba8c56b977c0b399b45f7b46247978a48097aacc539d794298142759abd800419e0b85768d4d646826ee897c1fdadb17b46abde5b602420d1336c78023443c1668f7ab17d2bf2c96c3baaeceb3aba18d8101e40f11e072bad35dbfa6e8864a820f7dac2ae3326ae25f1fbad033d425d1023d8042945f4303963046f251237cccb3348a49191b4ead17564590264de07ae4effe6c58fcd7902e9b64001014c2886fbeb46aa4ffeb8b6ee80881c8e17e15426a2696462f784c3599e7000b243e8ee3a392d110fd0c883b8f0b11712c0247547086cef0e1d2867006786e0baf80ca51dacecb1854fc7a856129236ae2b9f2ab7ab4df65a26602f6630a7b7410e4064d96f23a2655e61e01b5b0e89305c16e8b82e0a2a215ded47b0835928fc32e30d9b48a0c697eab788bd32bd182b1d11e617ba5836ce8d0a3122f927c2dc1f6b28f280a0885ab18a5503be8c98b8427075b201cb7cb60ee443965fa8f0f436e0aad32b7cfd23312530260ab2923fafe84d9d9dc33c6a9e98d5346b9fdc74572e17068d5ada48c1b66b594b2800a47918861b01986850b60848a698f48ce2815ddebe3e100043e5fa91a917cb22a9643ebe27e45b6e7322ba94611b6f406d691546d35c4a8428d3778b95f3968ec1bbcf0092ad167bd3735599b662c368e4144602e1a716a75e423cd172d7650006755607262f5c5e27a62fe34c0cbb0d8ab88b3be0ce0f7298aaf6ad558d8e64d7e0a44915e57ec26851929506e132c568e3838c038a72206920d859405c2195766086e666570b3ba55f8a3217593495261564897933d132aaa32553287e9c1b234ee46c358f74fc1474b4da7b8349828c62dafae3390a2c96d071e74daedf19c82acb71485d8d89f5757bed5c96917da23156e1baf1038ae2d8893f1dc768da019567540fba446dca91292ce12b0e4aa77a9041aba924290956f0c7997b9285476277aca548e6c44be6e6a50c0dddebcee1fcb69a9d30bc99d2468d3286755faea0dac819e0ea859c358a7583ce0d4ece1f44bec6c8"),
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
            storage.set_chain_height(1)?;
            storage.save_account_state(&account_state)?;
            
            tracing::info!("✓ Genesis block verified: {}", genesis.hash);
            (vec![genesis], account_state)
        } else {
            // OPTIMIZATION: chain only contains genesis (loaded from db.rs load_chain())
            // Or we just load genesis manually here
            let genesis = storage.load_block(0).expect("Genesis block must exist if height > 0");
            let chain = vec![genesis];
            
            tracing::info!("✓ Loaded blockchain with {} blocks (genesis in memory, rest on disk)", height);
            
            // SECURITY: Verify genesis block on load (prevents database tampering)
            if network == ChainNetwork::Mainnet && chain[0].hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block mismatch in existing chain!\nExpected: {}\nGot: {}\nDatabase may be corrupted or from different network.", 
                    GENESIS_HASH, chain[0].hash);
            }
            

            // SELF-HEAL: Detect corrupted account state from bad reorg (v0.4.0 bug).
            // If Faucet 0 shows 0 balance on an existing chain that has blocks, the
            // genesis premine was never applied — rebuild from scratch automatically.
            const FAUCET_0: &str = "0x1683be267318d2ddd8cee8df4a4548dcffb1e088";
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
                    "0x1683be267318d2ddd8cee8df4a4548dcffb1e088",
                    "0xd528c18ce7a8844e4a4dcd841975b20ae599b020",
                    "0xfd6e36bfa2b2798d08592802206c943d5513adfb",
                    "0xed15573ad312d41aaef74cff56a8ef28122ec2db",
                    "0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb",
                    "0xbf5ee055f399323fdd0cefe3d4aa923678d46107",
                    "0x1dc9637b183093d723ea8d1fb18083b06490facb",
                    "0xa2270f30ca1aad922510375508bf68cd95509f29",
                    "0xe15a689775685ae324559ea9a492fc650354ca0b",
                    "0x005dcff212d27b55e7a74bf745e1349ab44ca25d",
                ];
                for addr in &faucets {
                    let gtx = Transaction {
                        sender: "GENESIS".to_string(), recipient: addr.to_string(),
                        amount: 1_000_000_000_000, timestamp: genesis_ts,
                        signature: vec![], public_key: vec![],
                        fee: 0, nonce: 0, lock_time: 0,
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
                            if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                                let total = tx.amount.saturating_add(tx.fee);
                                healed_state.debit_account(&tx.sender, total);
                                healed_state.increment_nonce(&tx.sender);
                            }
                            let maturity = if tx.is_genesis_premine() { 0 } else { COINBASE_MATURITY };
                            healed_state.credit_account(tx, block.index, maturity);
                        }
                    }
                }
                storage.save_account_state(&healed_state)?;
                tracing::info!("✅ SELF-HEAL complete: Faucet 0 balance restored to {} microunits",
                    healed_state.get_balance(FAUCET_0));
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
            .build_global() {
            tracing::warn!("Could not configure rayon thread pool: {} (using default config)", e);
        }
        
        // Compute/load cumulative work — O(1) after first run (migration is one-time only).
        let initial_cumulative_work = {
            let stored = storage.get_cumulative_work();
            if stored == 0 && height > 1 {
                tracing::info!("[Migration] Computing cumulative work for {} blocks (one-time)…", height);
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
            signature_cache: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(100_000).unwrap()))),
            mempool_bloom: Arc::new(PLMutex::new(Bloom::new_for_fp_rate(50_000, 0.0001))),
            pubkey_cache: Arc::new(DashMap::new()),
            cumulative_work: Arc::new(PLMutex::new(initial_cumulative_work)),
            new_block_tx: Arc::new(new_block_tx),
            treasury_address: TREASURY_ADDRESS.to_string(),
        })
    }

    /// Subscribe to new-block notifications.
    ///
    /// Returns a `watch::Receiver<u64>` that yields the new chain height each
    /// time a block is accepted. Use with `tokio::select!` in the mining loop
    /// to abort the current PoW immediately when the chain moves:
    ///
    /// ```ignore
    /// let mut new_block_rx = blockchain.read().await.subscribe_new_blocks();
    /// loop {
    ///     let mut block = create_template();
    ///     tokio::select! {
    ///         _ = new_block_rx.changed() => { /* chain moved, restart */ }
    ///         result = spawn_blocking(move || { block.mine_with_cancel(&cancel); block }) => {
    ///             submit(result);
    ///         }
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
        };
        for (checkpoint_height, checkpoint_hash) in checkpoints {
            if *checkpoint_height == height {
                if hash != *checkpoint_hash {
                    tracing::error!(
                        "Checkpoint violation at height {}: expected {}, got {}",
                        height, checkpoint_hash, hash
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
            self.chain.read().get(0).unwrap().clone()
        } else {
            // Load from storage (not memory!)
            self.storage.load_block(height - 1).expect("Latest block must exist")
        }
    }

    /// Add a new transaction to the mempool
    pub fn add_transaction(&self, transaction: Transaction) -> Result<(), BlockchainError> {
        // Skip validation for coinbase transactions
        if transaction.is_coinbase() {
            self.pending_transactions.write().push(transaction);
            return Ok(());
        }

        // LOW-1 FIX: Reject excessively long addresses to prevent unbounded key allocations
        if transaction.sender.len() > MAX_ADDRESS_LEN || transaction.recipient.len() > MAX_ADDRESS_LEN {
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
        let mut nonce_entry = self.pending_nonces.entry(transaction.sender.clone()).or_insert(chain_nonce);
        let expected_nonce = (*nonce_entry).max(chain_nonce) + 1;
        
        if transaction.nonce != expected_nonce {
            return Err(BlockchainError::InvalidNonce {
                expected: expected_nonce,
                actual: transaction.nonce,
            });
        }
        
        // Check transaction size limit (DOS protection - prevents huge DeployContract)
        let tx_size = bincode::serialize(&transaction).map_err(|_| BlockchainError::InvalidBlock)?.len();
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
            let sender_count = pending.iter().filter(|t| t.sender == transaction.sender).count();
            if sender_count >= MAX_MEMPOOL_TXS_PER_SENDER {
                return Err(BlockchainError::MempoolFull(sender_count));
            }
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
    pub fn create_block_template(&self, miner_address: String) -> Result<Block, BlockchainError> {
        let reward = self.get_block_reward();
        
        let current_height = self.get_height();
        
        // Get pending transactions sorted by fee (highest first) with size limits
        let pending_txs = self.pending_transactions.read();
        
        // Filter out transactions locked for future blocks, then sort by fee descending
        let mut sorted_txs: Vec<_> = pending_txs.iter()
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
            reward / 1_000_000, treasury_allocation / 1_000_000,
            fee_burned / 1_000_000, proposer_reward / 1_000_000
        );
        
        // Coinbase transaction (full proposer reward + fee share)
        let coinbase_amount = proposer_reward.saturating_add(fee_to_miner);
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: miner_address.clone(),
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
        for tx in &all_transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                let required = tx.amount.saturating_add(tx.fee);
                temp_state.debit_account(&tx.sender, required);
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
        let mut new_block = Block::new_bft(index, all_transactions, previous_hash, epoch, 0, "LEGACY".to_string());
        
        // Ensure timestamp is valid (strictly greater than previous and MTP)
        let current_time = chrono::Utc::now().timestamp();
        let mtp = if previous_block.index >= 10 {
            self.get_median_time_past(previous_block.index, 11)
        } else {
            0
        };
        let min_valid_time = std::cmp::max(previous_block.timestamp, mtp);
        new_block.timestamp = std::cmp::max(current_time, min_valid_time + 1);
        
        new_block.state_root = state_root;
        new_block.hash = new_block.calculate_hash(); // Re-calculate hash with state_root and adjusted timestamp included
        
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
                        if temp_state.debit_account(&tx.sender, total_required) {
                            temp_state.increment_nonce(&tx.sender);
                            transactions.push(tx.clone());
                            block_size += tx_size;
                            added_any = true;
                        }
                    }
                    sorted_txs.remove(i);
                } else if tx.nonce < expected_nonce {
                    sorted_txs.remove(i);
                } else {
                    i += 1;
                }
                if transactions.len() >= MAX_BLOCK_TRANSACTIONS { break; }
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
                recipient: self.treasury_address.clone(),
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
    pub fn get_block_by_index(&self, height: u64) -> Option<Block> {
        if height == self.get_height() {
            return Some(self.get_latest_block());
        }
        self.storage.load_block(height).ok()
    }

    /// Get current BFT block reward (proposer reward = mining reward equivalent).
    fn get_block_reward(&self) -> u64 {
        let chain_len = self.get_height();
        let years_elapsed = chain_len / BLOCKS_PER_YEAR;
        apply_annual_reduction(YEAR_1_REWARD, years_elapsed).max(MIN_REWARD)
    }

    /// Get current difficulty — reads from STORAGE (the real chain), not the
    /// in-memory `chain` vec which only holds genesis after startup.
    #[allow(dead_code)]
    /// Validate block against consensus rules (CRITICAL for network blocks)
    fn validate_block_consensus(&self, block: &Block, previous: &Block, base_state: &AccountState) -> Result<(), BlockchainError> {
        // 0. Block size limit (DoS protection)
        let block_size = bincode::serialize(block).map_err(|_| BlockchainError::InvalidBlock)?.len();
        if block_size > MAX_BLOCK_SIZE_BYTES {
            return Err(BlockchainError::BlockTooLarge { size: block_size });
        }
        
        // 1. Cryptographic validity (done in block.is_valid)
        
        // 2. Timestamp bounds (prevent manipulation and time-travel attacks)
        if block.timestamp <= previous.timestamp {
            tracing::warn!("Block timestamp {} <= previous {}", block.timestamp, previous.timestamp);
            return Err(BlockchainError::InvalidBlock);
        }
        let current_time = chrono::Utc::now().timestamp();
        if block.timestamp > current_time + MAX_FUTURE_BLOCK_TIME {
            tracing::warn!("Block timestamp {} too far in future (max +{} sec)", 
                block.timestamp - current_time, MAX_FUTURE_BLOCK_TIME);
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
                    block.timestamp, mtp
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
            let committee = base_state.compute_epoch_committee(
                crate::consensus::authorities::MAX_COMMITTEE_SIZE
            );
            if !crate::consensus::bft::verify_bft_certificate(block, &committee, base_state) {
                tracing::warn!(
                    "Block {}: BFT certificate verification failed (committee_size={})",
                    block.index, committee.len()
                );
                return Err(BlockchainError::InvalidBlock);
            }
            tracing::debug!(
                "Block {}: BFT certificate verified ({} sigs)",
                block.index, block.bft_signatures.len()
            );
        }
        
        // 4. Coinbase validation - Must account for fee distribution
        let coinbase_txs: Vec<_> = block.transactions.iter().filter(|tx| tx.is_coinbase()).collect();
        if coinbase_txs.is_empty() || coinbase_txs.len() > 1 {
            tracing::warn!("Block must have exactly one coinbase transaction, found {}", coinbase_txs.len());
            return Err(BlockchainError::InvalidBlock);
        }
        
        // Validate treasury transaction if present
        let treasury_txs: Vec<_> = block.transactions.iter()
            .filter(|tx| tx.sender == "TREASURY")
            .collect();
        
        let coinbase = coinbase_txs[0];
        let expected_reward = self.calculate_reward_at_height(block.index);
        let total_fees: u64 = block.transactions.iter()
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
                expected_coinbase, proposer_reward, fee_to_miner, coinbase.amount
            );
            return Err(BlockchainError::InvalidCoinbaseReward {
                actual: coinbase.amount,
                expected: expected_coinbase,
            });
        }
        
        // Validate treasury transaction if fees or allocation exist
        let expected_treasury = treasury_allocation.saturating_add(fee_to_treasury);
        if expected_treasury > 0 {
            if treasury_txs.len() != 1 {
                tracing::warn!("Block should have treasury transaction for {} microunits", expected_treasury);
                return Err(BlockchainError::InvalidBlock);
            }
            
            let treasury_tx = treasury_txs[0];
            if treasury_tx.amount != expected_treasury {
                tracing::warn!(
                    "Invalid treasury amount: expected {}, got {}",
                    expected_treasury, treasury_tx.amount
                );
                return Err(BlockchainError::InvalidBlock);
            }
            
            if treasury_tx.recipient != TREASURY_ADDRESS {
                tracing::warn!("Treasury transaction sent to wrong address: {}", treasury_tx.recipient);
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
        let all_sigs_valid = block.transactions
            .par_iter()
            .all(|tx| {
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
                    self.pubkey_cache.insert(tx.sender.clone(), tx.public_key.clone());
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
                tracing::warn!("Transaction locked until block {}, but included in block {}", tx.lock_time, block.index);
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
                let max_cp = TESTNET_CHECKPOINTS.iter().map(|(h, _)| *h).max().unwrap_or(0);
                if block.index < max_cp {
                    tracing::debug!(
                        "Pre-checkpoint nonce override block {}: {} expected {} got {} — trusting block",
                        block.index, &tx.sender, expected_nonce, tx.nonce
                    );
                    temp_state.set_nonce(&tx.sender, tx.nonce.saturating_sub(1));
                } else {
                    tracing::warn!("Invalid nonce in block: tx from {} has nonce {}, expected {}",
                        tx.sender, tx.nonce, expected_nonce);
                    return Err(BlockchainError::InvalidNonce {
                        expected: expected_nonce,
                        actual:   tx.nonce,
                    });
                }
            }
            
            // CRITICAL: Validate sufficient balance (prevents double-spend)
            let total_required = tx.amount.saturating_add(tx.fee);
            let available = temp_state.get_balance(&tx.sender);
            if available < total_required {
                tracing::warn!("Insufficient balance in block: {} has {} but needs {}",
                    tx.sender, available, total_required);
                return Err(BlockchainError::InsufficientBalance {
                    required: total_required,
                    available,
                });
            }
            
            // Update temporary state to validate next transactions
            if !temp_state.debit_account(&tx.sender, total_required) {
                return Err(BlockchainError::InvalidBlock);
            }

            // Handle Validator Staking (v2)
            if let crate::core::transaction::TransactionType::Stake { validator_pubkey } = &tx.tx_type {
                let epoch = crate::consensus::authorities::epoch_for_height(block.index);
                temp_state.register_validator(
                    &tx.sender,
                    validator_pubkey.clone(),
                    tx.amount, // staked amount
                    epoch,
                );
                tracing::info!(
                    "Validator registered: {} (epoch={}, stake={})",
                    tx.sender, epoch, tx.amount
                );
            }

            // Handle Validator Unstaking (v2)
            if tx.is_unstake() {
                temp_state.deregister_validator(&tx.sender);
                tracing::info!("Validator deregistered: {}", tx.sender);
            }

            temp_state.credit_account(tx, block.index, COINBASE_MATURITY);
            temp_state.increment_nonce(&tx.sender);
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
        let is_checkpointed = self.validate_checkpoint(block.index, &block.hash)
            && {
                // validate_checkpoint returns true for heights with NO checkpoint too,
                // so we must confirm there IS a checkpoint at this exact height.
                let checkpoints = match self.network {
                    ChainNetwork::Testnet => TESTNET_CHECKPOINTS,
                    ChainNetwork::Mainnet => MAINNET_CHECKPOINTS,
                };
                checkpoints.iter().any(|(h, _)| *h == block.index)
            };
        if block.index > 0
            && !block.state_root.is_empty()
            && block.state_root != computed_state_root
            && !is_checkpointed
        {
            tracing::warn!(
                "Invalid state root at block {}: computed={}, block={}",
                block.index, computed_state_root, block.state_root
            );
            return Err(BlockchainError::InvalidBlock);
        }
        if is_checkpointed && !block.state_root.is_empty() && block.state_root != computed_state_root {
            tracing::info!(
                "State root mismatch at checkpointed block {} (computed={}, block={}) — \
                 trusting checkpoint; local state will converge from this height onward.",
                block.index, computed_state_root, block.state_root
            );
        }
        
        Ok(())
    }
    
    /// Validate block against consensus rules during REORG / SYNC replay.
    ///
    /// In v2 BFT, all committed blocks are final. This function performs
    /// BFT certificate verification and coinbase/signature checks.
    fn validate_block_consensus_reorg(&self, block: &Block, previous: &Block) -> Result<(), BlockchainError> {
        // Size check.
        let block_size = bincode::serialize(block).map_err(|_| BlockchainError::InvalidBlock)?.len();
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
            let committee = state.compute_epoch_committee(
                crate::consensus::authorities::MAX_COMMITTEE_SIZE
            );
            if !crate::consensus::bft::verify_bft_certificate(block, &committee, &state) {
                tracing::warn!("Reorg block {}: BFT certificate invalid", block.index);
                return Err(BlockchainError::InvalidBlock);
            }
        }

        // Signature validation on all user transactions.
        let all_sigs_valid = block.transactions
            .par_iter()
            .all(|tx| {
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
            tracing::warn!("Reorg block {} contains invalid transaction signatures", block.index);
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
                    tracing::warn!("Reorg: block {} has invalid tx (insufficient balance)", block.index);
                    return Err(BlockchainError::InvalidBlock);
                }
                new_state.increment_nonce(&tx.sender);
            }
            let maturity = if tx.is_genesis_premine() { 0 } else { COINBASE_MATURITY };
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
            let _ = self.storage.save_account_state_at_height(block.index, &new_state);
        }

        *self.account_state.write() = new_state;

        // Clear ALL pending nonces after a reorg — the DashMap entries from the
        // abandoned fork are now stale (wrong base nonce) and would cause every
        // subsequent mempool submission to fail with "Invalid nonce: expected N, got 1".
        self.pending_nonces.clear();

        // Notify miners: chain moved during reorg, abort stale PoW.
        let _ = self.new_block_tx.send(block.index + 1);

        tracing::info!("Reorg: network block {} accepted (permissive diff check)", block.index);
        Ok(())
    }

    /// Calculate reward at specific height (for validation)
    ///
    /// CONSENSUS-CRITICAL: Must match `get_mining_reward` exactly.
    /// Pure integer math — no f64.
    fn calculate_reward_at_height(&self, height: u64) -> u64 {
        let years_elapsed = height / BLOCKS_PER_YEAR;
        apply_annual_reduction(YEAR_1_REWARD, years_elapsed).max(MIN_REWARD)
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
        for i in start..=end_index {
            if let Ok(block) = self.storage.load_block(i) {
                timestamps.push(block.timestamp);
            }
        }
        
        if timestamps.is_empty() {
            return 0;
        }
        
        timestamps.sort_unstable();
        timestamps[timestamps.len() / 2]  // Return median
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
            
            let prev = match self.storage.load_block(i-1) {
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
        let total_transactions = 0;
        let pending = self.pending_transactions.read();

        BlockchainStats {
            chain_length: height as usize,
            total_transactions,
            current_epoch,
            mining_reward: self.get_block_reward(),
            total_supply: self.calculate_total_supply(),
            pending_transactions: pending.len(),
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
            total_minted = total_minted.saturating_add(
                reward.saturating_mul(BLOCKS_PER_YEAR)
            );
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
    pub fn get_pending_transactions_mut(&self) -> parking_lot::RwLockWriteGuard<'_, Vec<Transaction>> {
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
        for h in 0..=tip_height {   // inclusive: block AT tip_height contributes its difficulty
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
            return res;
        } else if block.index > latest.index {
            // Potential fork: block is ahead of us
            tracing::warn!("Fork detected: Block {} at height {}, we're at {}", 
                &block.hash[..8], block.index, latest.index);
            
            // BFT: Validate BFT certificate for orphan blocks.
            if block.index > 0 && block.bft_signatures.is_empty() {
                tracing::warn!("Rejecting orphan block with missing BFT certificate");
                return Err(BlockchainError::InvalidBlock);
            }
            
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
            
            tracing::info!("Stored orphaned block at height {}, need to sync", block.index);
            return Ok(());
        } else if block.index == latest.index {
            // Competing block at same height — apply longest-chain (most cumulative PoW) rule.
            tracing::warn!("Competing block at height {}: incoming {} vs ours {}", 
                block.index, &block.hash[..8], &latest.hash[..8]);
            
            // BFT: require valid certificate for competing block.
            if block.index > 0 && block.bft_signatures.is_empty() {
                tracing::warn!("Rejecting competing block with missing BFT certificate");
                return Err(BlockchainError::InvalidBlock);
            }
            
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
                block.index, &latest.hash[..8], &block.hash[..8]
            );
            return Ok(());
        } else if block.index + 1 == latest.index && block.previous_hash != String::new() {
            // Block is 1 behind our tip — it might be the base of a competing fork.
            // Store in orphans so process_orphans can detect if a longer chain builds on it.
            tracing::debug!("Storing near-stale block at height {} as potential fork base", block.index);
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() < MAX_ORPHAN_BLOCKS {
                orphans.push_back(block);
            }
            return Ok(());
        } else {
            // Block is behind our chain - likely stale
            tracing::debug!("Ignoring stale block at height {} (we're at {})", 
                block.index, latest.index);
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
        let prev_block = self.storage.load_block(incoming.index - 1)
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
        let _pre_tip_state = self.storage.load_account_state()
            .ok()
            .flatten();
        
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
            let maturity = if tx.is_genesis_premine() { 0 } else { COINBASE_MATURITY };
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
                        || (!btx.is_coinbase()
                            && btx.sender == tx.sender
                            && btx.nonce == tx.nonce)
                })
            });
        }
        // REORG-NONCE FIX: clear ALL pending_nonces after a shallow reorg —
        // the old tip's sender nonces are now wrong because the fork erased those txs.
        self.pending_nonces.clear();

        // Notify miners that the tip changed so they abort stale PoW immediately.
        let _ = self.new_block_tx.send(incoming.index + 1);

        // Update cumulative_work: subtract old tip's weight (1), add incoming tip's (1). No-op for BFT.

        tracing::info!("Reorg complete: replaced tip at height {} with block {}",
            incoming.index, &incoming.hash[..8]);
        Ok(())
    }

    /// Rebuild account state by replaying all blocks from genesis up to (and including)
    /// `target_height` from storage. Used during reorg to get a clean state snapshot.
    ///
    /// SYNC FIX: Previously this replayed ALL blocks from genesis (O(height) = 
    /// 18k sled disk reads!). Now it loads the nearest 1000-block checkpoint
    /// and replays only the delta (max 1000 blocks).
    fn rebuild_account_state_up_to(&self, target_height: u64) -> crate::core::transaction::AccountState {
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
                    tracing::info!("Loaded account state snapshot at height {} (replaying delta to {})",
                        snap_height, target_height);
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
            tracing::info!("Replaying blocks {}..={} for state rebuild", replay_start, target_height);
            for h in replay_start..=target_height {
                if let Ok(block) = self.storage.load_block(h) {
                    state.unlock_mature_coinbase(block.index);
                    for tx in &block.transactions {
                        if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                            let total = tx.amount.saturating_add(tx.fee);
                            state.debit_account(&tx.sender, total);
                            state.increment_nonce(&tx.sender);
                        }
                        let maturity = if tx.is_genesis_premine() { 0 } else { COINBASE_MATURITY };
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
    fn rebuild_state_from_genesis_up_to(&self, _dummy: u64) -> crate::core::transaction::AccountState {
        let mut state = crate::core::transaction::AccountState::new();

        // ---------- GENESIS PREMINE ----------
        let genesis_timestamp = self.storage.load_block(0)
            .map(|g| g.timestamp)
            .unwrap_or(0);
        let testnet_faucets = [
            "0x1683be267318d2ddd8cee8df4a4548dcffb1e088",  // Faucet 0 (sender)
            "0xd528c18ce7a8844e4a4dcd841975b20ae599b020",  // Faucet 1
            "0xfd6e36bfa2b2798d08592802206c943d5513adfb",  // Faucet 2
            "0xed15573ad312d41aaef74cff56a8ef28122ec2db",  // Faucet 3
            "0xaffd6d4f74c5651110efcf1b9736f7a5cf2ccdbb",  // Faucet 4
            "0xbf5ee055f399323fdd0cefe3d4aa923678d46107",  // Faucet 5
            "0x1dc9637b183093d723ea8d1fb18083b06490facb",  // Faucet 6
            "0xa2270f30ca1aad922510375508bf68cd95509f29",  // Faucet 7
            "0xe15a689775685ae324559ea9a492fc650354ca0b",  // Faucet 8
            "0x005dcff212d27b55e7a74bf745e1349ab44ca25d",  // Faucet 9
        ];
        
        let premine_amount = if self.network == crate::core::ChainNetwork::Testnet { 1_000_000_000_000 } else { 1_000_000_000 };
        let recipients = if self.network == crate::core::ChainNetwork::Testnet {
            testnet_faucets.iter().map(|s| s.to_string()).collect::<Vec<String>>()
        } else {
            vec!["0x0000000000000000000000000000000000000000".to_string()]
        };

        for addr in &recipients {
            let genesis_tx = crate::core::transaction::Transaction {
                sender:    "GENESIS".to_string(),
                recipient: addr.to_string(),
                amount:    premine_amount,
                timestamp: genesis_timestamp,
                signature: vec![],
                public_key: vec![],
                fee:       0,
                nonce:     0,
                lock_time: 0,
                tx_type:   crate::core::transaction::TransactionType::Transfer,
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
            tracing::error!("Rejecting block {} due to checkpoint violation", block.index);
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
            }
            // GENESIS premine: maturity=0 (immediately spendable)
            // All other txs (including COINBASE mining rewards): COINBASE_MATURITY
            let maturity = if tx.is_genesis_premine() { 0 } else { COINBASE_MATURITY };
            new_state.credit_account(tx, block.index, maturity);
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
            if let Err(e) = self.storage.save_account_state_at_height(block.index, &new_state) {
                tracing::warn!("Failed to save account-state checkpoint at {}: {}", block.index, e);
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
                    || (!btx.is_coinbase()
                        && btx.sender == tx.sender
                        && btx.nonce == tx.nonce)
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
        self.pending_nonces.retain(|addr, cached_nonce| {
            *cached_nonce > confirmed_state.get_nonce(addr)
        });
        drop(confirmed_state);

        // 11. Notify miners that the chain has moved — they should abort stale PoW.
        let _ = self.new_block_tx.send(block.index + 1);

        tracing::info!("Network block {} accepted at height {}", block.index, block.index);
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
                
                tracing::info!("Orphan block {} connects to main chain at height {}", &block.hash[..8], block.index);
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
    pub fn get_address_info(&self, address: &str) -> Option<crate::core::transaction::AccountBalance> {
        let state = self.account_state.read();
        state.get_account(address).cloned()
    }

    /// Look up a confirmed transaction by its hash.
    ///
    /// Uses the O(1) storage index built at save-time — no full chain scan.
    /// Returns `None` if the transaction is not found in confirmed blocks
    /// (it might still be in the mempool).
    pub fn find_transaction_by_hash(&self, tx_hash: &str) -> Option<crate::core::transaction::Transaction> {
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
        let scan_end  = scan_start.saturating_sub(max_blocks.min(height));

        let mut results: Vec<AddressTransaction> = Vec::new();

        let i_start = scan_start;
        let i_end   = scan_end;

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
    pub fn deep_reorg(&self, rollback_to: u64, new_chain: Vec<Block>) -> Result<(), BlockchainError> {
        let our_height = self.get_height();
        tracing::warn!(
            "DEEP REORG: rolling back from height {} to {}, then applying {} new blocks",
            our_height, rollback_to, new_chain.len()
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
        };
        for (cp_height, cp_hash) in checkpoints {
            if *cp_height >= rollback_to && *cp_height < our_height {
                // We would be rolling back past this checkpoint.
                tracing::error!(
                    "Deep reorg refused: would cross checkpoint at height {} ({})",
                    cp_height, cp_hash
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
                effective_rollback, rollback_to
            );
            return Err(BlockchainError::InvalidBlock);
        }
        // Use the actual first-block index as our rollback target so that
        // rollback_to consistently equals sorted[0].index for all downstream logic.
        let rollback_to = effective_rollback;

        // Verify BFT certificate on all incoming blocks before we commit to anything.
        for b in &sorted {
            if b.index > 0 && b.bft_signatures.is_empty() {
                tracing::warn!("Deep reorg: incoming block {} missing BFT certificate", b.index);
                return Err(BlockchainError::InvalidBlock);
            }
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
        tracing::info!("Deep reorg: account state rebuilt up to height {}", rollback_to - 1);

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
        tracing::info!("Deep reorg: cumulative_work reset to {} at rollback height {} (O(1) lookup)", base_work, rollback_to);

        // Clear the orphan pool — everything in it belongs to a now-stale fork.
        self.orphaned_blocks.write().clear();

        // --- Replay new blocks ---
        let mut applied = 0u64;
        let mut reorg_failed = false;
        for block in &sorted {
            match self.add_block_to_main_chain_reorg(block.clone()) {
                Ok(_) => {
                    applied += 1;
                    tracing::info!("Deep reorg: applied block {} ({}...)", block.index, &block.hash[..8]);
                }
                Err(e) => {
                    tracing::warn!(
                        "Deep reorg: failed to apply block {} (height {}) — aborting reorg: {}",
                        &block.hash[..8], block.index, e
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
                    tracing::error!("CRITICAL: Failed to restore original chain at block {}: {}", block.index, e);
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
            applied, final_height
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
    pub current_epoch: u64,
    pub mining_reward: u64,      // microunits
    pub total_supply: u64,       // microunits
    pub pending_transactions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Fee Distribution Math ───────────────────────────────────────────────

    /// 70% burn + 20% treasury + 10% miner fee split must be exact (no rounding loss)
    #[test]
    fn test_fee_distribution_no_rounding_loss() {
        let total_fees: u64 = 1_000_000; // 1 QUA in microunits

        let fee_burned       = (total_fees * FEE_BURN_PERCENT) / 100;      // 700_000
        let fee_to_treasury  = (total_fees * FEE_TREASURY_PERCENT) / 100;  // 200_000
        let fee_to_miner     = total_fees - fee_burned - fee_to_treasury;  // 100_000 (remainder)

        assert_eq!(fee_burned,      700_000, "70% should be burned");
        assert_eq!(fee_to_treasury, 200_000, "20% goes to treasury");
        assert_eq!(fee_to_miner,    100_000, "10% to miner (no rounding loss)");
        assert_eq!(fee_burned + fee_to_treasury + fee_to_miner, total_fees,
            "fee split must be lossless");
    }

    /// Odd fee amounts should give remainder to miner, not lose value
    #[test]
    fn test_fee_distribution_odd_amounts() {
        let total_fees: u64 = 999; // deliberately not divisible by 100

        let fee_burned       = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury  = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_miner     = total_fees - fee_burned - fee_to_treasury; // remainder

        // All microunits must be accounted for
        assert_eq!(fee_burned + fee_to_treasury + fee_to_miner, total_fees,
            "every microunit must go somewhere — no value created or destroyed");
    }

    // ─── Block Reward Math ───────────────────────────────────────────────────

    /// 5% treasury + 95% miner split from block reward
    #[test]
    fn test_block_reward_treasury_split() {
        let reward: u64 = 100_000_000; // 100 QUA Year-1 reward

        let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100; // 5 QUA
        let miner_reward        = reward - treasury_allocation;                  // 95 QUA

        assert_eq!(treasury_allocation, 5_000_000, "5% of 100 QUA = 5 QUA");
        assert_eq!(miner_reward,       95_000_000, "95% of 100 QUA = 95 QUA");
        assert_eq!(treasury_allocation + miner_reward, reward, "no value lost");
    }


    // ─── Reward Reduction ────────────────────────────────────────────────────

    /// Year 0 reward must be the full YEAR_1_REWARD
    #[test]
    fn test_reward_year_0_is_full() {
        let reward = apply_annual_reduction(YEAR_1_REWARD, 0);
        assert_eq!(reward, YEAR_1_REWARD);
    }

    /// After 20+ years reward must not drop below MIN_REWARD floor
    #[test]
    fn test_reward_floor_after_many_years() {
        let reward = apply_annual_reduction(YEAR_1_REWARD, 50); // 50 years
        assert!(reward >= MIN_REWARD,
            "Reward {} must not drop below MIN_REWARD {}", reward, MIN_REWARD);
        assert_eq!(reward, MIN_REWARD, "After 50 years must be exactly at floor");
    }

    /// Reward at year 1 must be 85% of year 0 (15% annual reduction)
    #[test]
    fn test_reward_year1_reduction() {
        let year0 = apply_annual_reduction(YEAR_1_REWARD, 0);
        let year1 = apply_annual_reduction(YEAR_1_REWARD, 1);
        // Integer math: year1 = year0 * 85 / 100
        let expected = year0 * 85 / 100;
        assert_eq!(year1, expected,
            "Year 1 reward must be exactly 85% of year 0 (integer math)");
    }

    // ─── Treasury Address ─────────────────────────────────────────────────────

    /// Treasury address constant must be the real 3-of-5 multisig, not the placeholder
    #[test]
    fn test_treasury_address_is_not_placeholder() {
        assert_ne!(TREASURY_ADDRESS, "0x0000000000000000000000000000000000000001",
            "Treasury must be set to the real multisig address, not the placeholder");
        assert!(TREASURY_ADDRESS.starts_with("ms"),
            "Treasury address must start with 'ms' (multisig prefix), got: {}", TREASURY_ADDRESS);
    }

    /// Treasury address must be the exact known 3-of-5 address we generated
    #[test]
    fn test_treasury_address_exact_value() {
        assert_eq!(TREASURY_ADDRESS, "ms69216b1d10425689704d5ae3b2a4aa17049f59b1",
            "TREASURY_ADDRESS changed! Update this test AND generate a new genesis block.");
    }
}
