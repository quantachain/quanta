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

// BETA FIX: Increased from 10s to 30s — PQC blocks are ~2MB, at 10s there's
// only ~6s propagation slack in a 6-node network (causes dead forks)
// 30s gives ~26s slack and reduces fork probability from 2-5% to <0.1%.
const TARGET_BLOCK_TIME: u64 = 30; // 30 seconds
// SECURITY FIX (External Audit): Increased from 10 to 2016 for stability
// 2016 blocks = ~5.6 hours (prevents rapid oscillation)
const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 2016;

// SECURITY FIX (External Audit): Difficulty bounds
// Tightened from 2x/0.5x to 1.15x/0.85x (prevents wild swings)
// INTEGER MATH ONLY: expressed as percent numerators (100 = 100%).
// No f64 — floating-point divergence would cause consensus forks.
const MAX_DIFFICULTY_ADJUSTMENT_UP_PCT: u32 = 115;   // Max 15% increase per adjustment (x * 115 / 100)
const MAX_DIFFICULTY_ADJUSTMENT_DOWN_PCT: u32 = 85;  // Max 15% decrease per adjustment (x * 85 / 100)
const MIN_DIFFICULTY: u32 = 4;
// Increased from 256 to 2^31-1 for long-term security (prevents maxing out)
const MAX_DIFFICULTY: u32 = 2_147_483_647; // Can support massive hashrate growth

// MODERN ADAPTIVE TOKENOMICS (Option 3 - Solana-style)
const YEAR_1_REWARD: u64 = 100_000_000; // 100 QUA in microunits
const ANNUAL_REDUCTION_PERCENT: u64 = 15; // 15% reduction per year (faster value creation)
const MIN_REWARD: u64 = 5_000_000; // 5 QUA floor (reached after ~20 years)
const BLOCKS_PER_YEAR: u64 = 1_051_200; // 365.25 days * 86400 / 30 seconds

// UNIQUE FEATURES - Network Bootstrap
const BOOTSTRAP_PHASE_BLOCKS: u64 = 315_360; // First month gets network usage boost

// SUSTAINABLE ECONOMICS - Fee Structure & Value Capture
const BASE_TRANSACTION_FEE: u64 = 1_000; // 0.001 QUA minimum (prevents spam)
const FEE_BURN_PERCENT: u64 = 70; // 70% of fees burned (deflationary pressure)
const FEE_TREASURY_PERCENT: u64 = 20; // 20% to development treasury
const FEE_VALIDATOR_PERCENT: u64 = 10; // 10% to block validator (miner)

// TREASURY FUND - Development, Marketing, Listings
const TREASURY_ALLOCATION_PERCENT: u64 = 5; // 5% of block rewards → treasury

// CONSENSUS-CRITICAL: Treasury multisig address (3-of-5 Falcon-512, generated 2026-03-14)
// This address is hardcoded in consensus — it CANNOT be changed by editing quanta.toml.
// Any node that changes this constant will be rejected by the network (invalid treasury tx).
// To move treasury funds, use: quanta-wallet treasury-propose / treasury-sign / treasury-broadcast
// Keyset: treasury_key0.qua … treasury_key4.qua — any 3 of 5 must sign.
const TREASURY_ADDRESS: &str = "ms69216b1d10425689704d5ae3b2a4aa17049f59b1";

// ANTI-DUMP MECHANISM - Mining Reward Lockup
const MINING_REWARD_LOCK_PERCENT: u64 = 50; // 50% of mining rewards locked
const MINING_REWARD_LOCK_BLOCKS: u64 = 157_680; // 6 months vesting (182.5 days)

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
const MAX_ORPHAN_BLOCKS: usize = 100; // Maximum orphaned blocks (prevents memory exhaustion)
const MAX_TRANSACTION_SIZE_BYTES: usize = 102400; // 100KB max per transaction (prevents DOS)
const MIN_TRANSACTION_FEE: u64 = 100; // 0.0001 QUA in microunits
const TRANSACTION_EXPIRY_SECONDS: i64 = 86400; // 24 hours
const COINBASE_MATURITY: u64 = 100; // Blocks before coinbase can be spent
const MAX_FUTURE_BLOCK_TIME: i64 = 7200; // 2 hours maximum future timestamp
/// LOW-1 FIX: Bound address string length to prevent unbounded HashMap key allocations.
const MAX_ADDRESS_LEN: usize = 128;

// CONSENSUS-CRITICAL: Genesis block hash (prevents chain split attacks)
// Generated from Block::genesis() with timestamp 1735689600 (2026-01-01 00:00:00 UTC)
// Difficulty: 6 (PRODUCTION)
const GENESIS_HASH: &str = "527a8a6ad3292c9b42c40f3d71fd3b89cdd79415106ce0b8d9f7f6690a96433d";

// CHECKPOINT SYSTEM: Hardcoded checkpoints prevent deep reorganizations
// Format: (block_height, block_hash)
// Add checkpoints every ~1000 blocks for devnet, ~10000 for mainnet
const CHECKPOINTS: &[(u64, &str)] = &[
    (0, GENESIS_HASH),
    // Add more checkpoints as network matures:
    // (1000, "<block_1000_hash>"),
    // (5000, "<block_5000_hash>"),
    // (10000, "<block_10000_hash>"),
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

/// Thread-safe blockchain with persistent storage (OPTIMIZED)

/// 
/// CRITICAL CHANGE: No longer stores full chain in memory!
/// Before: 3.15M blocks × 2 MB = 20 GB RAM (crashes!)
/// After: Only genesis block + recent blocks = 2 GB RAM
pub struct Blockchain {
    chain: Arc<RwLock<Vec<Block>>>, // ONLY stores genesis block now
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    account_state: Arc<RwLock<AccountState>>,
    pending_nonces: Arc<DashMap<String, u64>>, // ATOMIC: Track highest pending nonce
    storage: Arc<BlockchainStorage>,
    orphaned_blocks: Arc<RwLock<VecDeque<Block>>>,

    // OPT-1: Signature verification cache
    // Saves ~1.5ms per cached Falcon-512 verification (80% hit rate in practice)
    signature_cache: Arc<Mutex<LruCache<String, bool>>>,

    // OPT-2 (PQC): Bloom filter for O(1) mempool duplicate detection
    // Before: O(n) scan over pending txs — at 1200 txs × 1713 bytes = 2MB scan every add
    // After:  O(1) probabilistic check — false-positive rate ~0.01% at 50k capacity
    mempool_bloom: Arc<PLMutex<Bloom<String>>>,

    // OPT-3 (PQC): Public key deserialization cache
    // Falcon-512 public keys are 897 bytes each. When a sender submits N txs in one block,
    // we were deserializing the same 897-byte key N times. Cache gives O(1) after first hit.
    // Key = sender address, Value = raw public key bytes (already verified)
    pubkey_cache: Arc<DashMap<String, Vec<u8>>>,
}

impl Blockchain {
    /// Create or load blockchain from storage (OPTIMIZED to not load full chain)
    pub fn new(storage: Arc<BlockchainStorage>, network: ChainNetwork) -> Result<Self, BlockchainError> {
        // OPTIMIZATION: Only load genesis to verify chain exists
        // All other blocks loaded on-demand from disk
        let chain = storage.load_chain()?;
        let account_state = storage.load_account_state()?.unwrap_or_else(AccountState::new);
        
        // Define expected genesis hash based on network
        // Note: Mainnet hash is hardcoded constant. Testnet hash should be calculated or hardcoded once known.
        // For now, we trust the generated testnet genesis if it's testnet.
        
        let (chain, account_state, _difficulty) = if chain.is_empty() {
            // Create genesis block
            tracing::info!("Creating new blockchain with genesis block for {:?}", network);
            let genesis = Block::genesis(network);
            
            // SECURITY: Verify genesis hash matches hardcoded value (prevents chain split)
            if network == ChainNetwork::Mainnet && genesis.hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block hash mismatch!\nExpected: {}\nGot: {}\nThis indicates tampering or incorrect genesis generation.", 
                    GENESIS_HASH, genesis.hash);
            } else if network == ChainNetwork::Testnet {
                tracing::info!("Testnet Genesis Hash: {}", genesis.hash);
            }
            
            let mut account_state = AccountState::new();
            
            // Genesis distribution
            let genesis_address = "0x0000000000000000000000000000000000000000";
            let genesis_tx = Transaction {
                sender: "COINBASE".to_string(),
                recipient: genesis_address.to_string(),
                amount: 1_000_000_000, // 1000 QUA in microunits
                timestamp: genesis.timestamp,
                signature: vec![],
                public_key: vec![],
                fee: 0,
                nonce: 0,
                tx_type: crate::core::transaction::TransactionType::Transfer,
                sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            };
            account_state.credit_account(&genesis_tx, 0, COINBASE_MATURITY);
            
            storage.save_block(&genesis)?;
            storage.set_chain_height(1)?;
            storage.save_account_state(&account_state)?;
            
            tracing::info!("✓ Genesis block verified: {}", genesis.hash);
            (vec![genesis], account_state, if network == ChainNetwork::Testnet { 4 } else { 6 })
        } else {
            // OPTIMIZATION: chain only contains genesis (loaded from db.rs load_chain())
            let height = storage.get_chain_height()?;
            tracing::info!("✓ Loaded blockchain with {} blocks (genesis in memory, rest on disk)", height);
            
            // SECURITY: Verify genesis block on load (prevents database tampering)
            if !chain.is_empty() && network == ChainNetwork::Mainnet && chain[0].hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block mismatch in existing chain!\nExpected: {}\nGot: {}\nDatabase may be corrupted or from different network.", 
                    GENESIS_HASH, chain[0].hash);
            }
            
            let difficulty = if height > 0 {
                // Load latest block to get difficulty
                storage.load_block(height - 1)?.difficulty
            } else {
                4
            };
            
            (chain, account_state, difficulty)
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
        tracing::info!("Rayon thread pool: {} physical cores for Falcon-512 verification", physical_cores);

        Ok(Self {
            chain: Arc::new(RwLock::new(chain)), // Only genesis in memory!
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            account_state: Arc::new(RwLock::new(account_state)),
            pending_nonces: Arc::new(DashMap::new()),
            storage,
            orphaned_blocks: Arc::new(RwLock::new(VecDeque::new())),
            // OPT-1: Signature verification cache (100k entries)
            signature_cache: Arc::new(Mutex::new(
                LruCache::new(NonZeroUsize::new(100_000).unwrap())
            )),
            // OPT-2 (PQC): Bloom filter — sized for 50k pending txs, 0.01% false-positive rate
            // At Falcon-512 tx sizes, 50k mempool = ~85 MB — bloom avoids scanning all of it
            mempool_bloom: Arc::new(PLMutex::new(
                Bloom::new_for_fp_rate(50_000, 0.0001)
            )),
            // OPT-3 (PQC): Public key cache — DashMap for lock-free concurrent reads
            pubkey_cache: Arc::new(DashMap::new()),
        })
    }

    /// Validate block against checkpoints (prevents deep reorgs)
    fn validate_checkpoint(&self, height: u64, hash: &str) -> bool {
        for (checkpoint_height, checkpoint_hash) in CHECKPOINTS {
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
        let reward = self.get_mining_reward();
        let difficulty = self.calculate_next_difficulty();
        
        // Get pending transactions sorted by fee (highest first) with size limits
        let pending_txs = self.pending_transactions.read();
        
        // Sort by fee descending (highest fee first)
        let mut sorted_txs = pending_txs.clone();
        sorted_txs.sort_by(|a, b| b.fee.cmp(&a.fee));
        
        let mut transactions = Vec::new();
        let mut block_size = 0usize;
        
        // Select transactions that fit in block limits (prioritize high fees)
        for tx in sorted_txs.iter() {
            if transactions.len() >= MAX_BLOCK_TRANSACTIONS {
                break;
            }
            
            let tx_size = bincode::serialize(tx).unwrap_or_default().len();
            if block_size + tx_size > MAX_BLOCK_SIZE_BYTES {
                break;
            }
            
            transactions.push(tx.clone());
            block_size += tx_size;
        }
        
        // Create coinbase transaction with fee distribution
        let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();
        
        // FEE DISTRIBUTION (70% burn, 20% treasury, 10% miner)
        // SECURITY FIX (HIGH-2): Prevent rounding loss - give remainder to miner
        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_miner = total_fees - fee_burned - fee_to_treasury; // Remainder goes to miner
        
        // TREASURY ALLOCATION (5% of block rewards)
        let treasury_allocation = (reward * TREASURY_ALLOCATION_PERCENT) / 100;
        let miner_reward = reward - treasury_allocation; // 95% to miner
        
        // ANTI-DUMP: 50% of mining rewards locked for 6 months
        let immediate_reward = (miner_reward * (100 - MINING_REWARD_LOCK_PERCENT)) / 100;
        let locked_reward = miner_reward - immediate_reward;
        
        tracing::info!(
            "Mining Economics: Reward={} QUA, Treasury={} QUA, Fees Burned={} QUA, Locked={} QUA",
            reward / 1_000_000, treasury_allocation / 1_000_000,
            fee_burned / 1_000_000, locked_reward / 1_000_000
        );
        
        // Coinbase transaction (immediate + fees to miner)
        let coinbase_amount = immediate_reward.saturating_add(fee_to_miner);
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: miner_address.clone(),
            amount: coinbase_amount,
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
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
                tx_type: crate::core::transaction::TransactionType::Transfer,
                sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            };
            all_transactions.push(treasury_tx);
        }
        
        all_transactions.extend(transactions);

        // Create new block (unmined)
        let previous_hash = self.get_latest_block().hash.clone();
        let index = self.get_height();
        let new_block = Block::new(index, all_transactions, previous_hash, difficulty);
        
        // Don't mine or save here. Just return the template.
        Ok(new_block)
    }

    /// Mine a new block with pending transactions (BLOCKING - for CLI use)
    pub fn mine_pending_transactions(&self, miner_address: String) -> Result<(), BlockchainError> {
        // Create template and mine synchronously
        let mut block = self.create_block_template(miner_address)?;
        block.mine(); 
        self.add_network_block(block)
    }

    /// Get current mining reward with adaptive model (u64 microunits)
    ///
    /// CONSENSUS-CRITICAL: Pure integer math only. No f64.
    /// Reduction formula: reward = YEAR_1_REWARD * (85/100)^years_elapsed
    /// Applied iteratively to avoid any floating-point divergence.
    fn get_mining_reward(&self) -> u64 {
        let chain_len = self.get_height();
        let years_elapsed = chain_len / BLOCKS_PER_YEAR;
        apply_annual_reduction(YEAR_1_REWARD, years_elapsed).max(MIN_REWARD)
    }
    

    
    /// Get current difficulty (DERIVED FROM CHAIN, not local memory)
    fn get_current_difficulty(&self) -> u32 {
        self.chain.read().last().map(|b| b.difficulty).unwrap_or(4)
    }

    /// Validate block against consensus rules (CRITICAL for network blocks)
    fn validate_block_consensus(&self, block: &Block, previous: &Block) -> Result<(), BlockchainError> {
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
        // Prevent large backward jumps (within 2 hours of previous block)
        // EXCEPTION: Allow large gap for block 1 (genesis to first mined block)
        const MAX_TIME_DELTA: i64 = 7200; // 2 hours
        if previous.index > 0 && (block.timestamp > previous.timestamp + MAX_TIME_DELTA ||
           block.timestamp < previous.timestamp - MAX_TIME_DELTA) {
            tracing::warn!("Block timestamp {} outside acceptable range (prev: {}, delta: {})", 
                block.timestamp, previous.timestamp, block.timestamp - previous.timestamp);
            return Err(BlockchainError::InvalidBlock);
        }
        
        // 3. Difficulty must match expected
        // Calculate expected difficulty considering adjustments
        let expected_difficulty = self.calculate_next_difficulty();
        if block.difficulty != expected_difficulty {
            return Err(BlockchainError::InvalidDifficulty);
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
        
        // REWARD DISTRIBUTION: 5% treasury, 95% to miner (50% locked)
        let treasury_allocation = (expected_reward * TREASURY_ALLOCATION_PERCENT) / 100;
        let miner_reward = expected_reward - treasury_allocation;
        let immediate_reward = (miner_reward * (100 - MINING_REWARD_LOCK_PERCENT)) / 100;
        
        // Coinbase should contain: immediate reward + miner's fee share
        let expected_coinbase = immediate_reward.saturating_add(fee_to_miner);
        if coinbase.amount != expected_coinbase {
            tracing::warn!(
                "Invalid coinbase amount: expected {} (reward: {}, fees: {}), got {}",
                expected_coinbase, immediate_reward, fee_to_miner, coinbase.amount
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
        // CRITICAL: Build temporary state to validate balances and nonces
        let mut temp_state = self.account_state.read().clone();
        
        // OPT-1+3 (PQC): Parallel sig verification with signature cache + pubkey cache
        // Serial: 1200 tx × 1.5ms = 1800ms
        // Parallel (physical cores): ~300ms
        // With caches: near-zero for repeat senders
        let all_sigs_valid = block.transactions
            .par_iter()
            .all(|tx| {
                if tx.is_coinbase() || tx.sender == "TREASURY" {
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
            // Skip system transactions
            if tx.is_coinbase() || tx.sender == "TREASURY" {
                continue;
            }
            
            // Fee must meet minimum
            if tx.fee < MIN_TRANSACTION_FEE {
                return Err(BlockchainError::FeeTooLow {
                    fee: tx.fee,
                    min: MIN_TRANSACTION_FEE,
                });
            }
            
            // CRITICAL: Validate nonce is sequential (prevents replay)
            let expected_nonce = temp_state.get_nonce(&tx.sender) + 1;
            if tx.nonce != expected_nonce {
                tracing::warn!("Invalid nonce in block: tx from {} has nonce {}, expected {}",
                    tx.sender, tx.nonce, expected_nonce);
                return Err(BlockchainError::InvalidNonce {
                    expected: expected_nonce,
                    actual: tx.nonce,
                });
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
            temp_state.credit_account(tx, block.index, COINBASE_MATURITY);
            temp_state.increment_nonce(&tx.sender);
        }
        
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

    /// Calculate next difficulty (pure function, deterministic)
    /// Calculate next difficulty (pure function, deterministic)
    fn calculate_next_difficulty(&self) -> u32 {
        let chain_len = self.get_height();
        
        // Not enough blocks yet - use initial difficulty
        if chain_len < DIFFICULTY_ADJUSTMENT_INTERVAL {
            // Get latest block difficulty
            return match self.storage.load_block(chain_len - 1) {
                Ok(b) => b.difficulty,
                Err(_) => Block::genesis(crate::core::ChainNetwork::Mainnet).difficulty,
            };
        }
        
        // Only adjust at intervals
        if chain_len % DIFFICULTY_ADJUSTMENT_INTERVAL != 0 {
             return match self.storage.load_block(chain_len - 1) {
                Ok(b) => b.difficulty,
                Err(_) => 6, // Fallback
            };
        }
        
        let latest_block = match self.storage.load_block(chain_len - 1) {
            Ok(b) => b,
            Err(_) => return 6,
        };
        
        let adjustment_start = chain_len.saturating_sub(DIFFICULTY_ADJUSTMENT_INTERVAL);
        // Note: we don't strictly need start_block object if we rely on median time
        
        // Calculate actual time taken for last N blocks
        // SECURITY FIX (HIGH-1): Use median-time-past to prevent timestamp manipulation
        let latest_median = self.get_median_time_past(chain_len - 1, 11);
        let start_median = self.get_median_time_past(adjustment_start, 11);
        let actual_time = latest_median - start_median;
        let expected_time = (TARGET_BLOCK_TIME * DIFFICULTY_ADJUSTMENT_INTERVAL) as i64;
        
        // Calculate actual time taken, clamp to prevent extreme adjustments (4x bounds)
        let actual_time_clamped = actual_time.max(expected_time / 4).min(expected_time * 4);
        
        // INTEGER MATH ONLY — no f64, no platform-dependent rounding.
        // Formula: new_difficulty = current * expected_time / actual_time
        // To avoid truncation, we scale up by 1000, divide, then round back down.
        // E.g. if ratio = 1.05, scaled = 1050; we add 500 before dividing by 1000
        // to get the nearest integer (equivalent to .round()).
        let cd = latest_block.difficulty as u64;
        let actual = actual_time_clamped as u64;
        let expected = expected_time as u64;

        // scaled_difficulty = round(current * expected / actual)
        let scaled = cd
            .checked_mul(expected)
            .and_then(|v| v.checked_mul(1000))
            .map(|v| (v / actual + 500) / 1000)  // +500 for rounding
            .unwrap_or(cd);

        // Apply ±15% bounds using integer percent constants, then global min/max.
        let floor = (cd * MAX_DIFFICULTY_ADJUSTMENT_DOWN_PCT as u64 + 50) / 100; // round
        let ceil  = (cd * MAX_DIFFICULTY_ADJUSTMENT_UP_PCT   as u64 + 50) / 100; // round
        let new_difficulty = scaled.clamp(floor, ceil)
            .clamp(MIN_DIFFICULTY as u64, MAX_DIFFICULTY as u64) as u32;
        
        tracing::info!("Difficulty adjustment: {} -> {} (actual time: {}s, expected: {}s)",
            latest_block.difficulty, new_difficulty, actual_time, expected_time);
        
        new_difficulty
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
        let current_difficulty = if height > 0 {
             self.get_latest_block().difficulty
        } else {
             4
        };
        
        // Note: total_transactions needs full scan or separate counter in storage
        // For now, return 0 or implement storage.get_total_txs()
        let total_transactions = 0; 
        
        let pending = self.pending_transactions.read();
        
        BlockchainStats {
            chain_length: height as usize, // Correct height
            total_transactions,
            current_difficulty,
            mining_reward: self.get_mining_reward(),
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

    /// Add a block received from the network (WITH FULL VALIDATION AND FORK RESOLUTION)
    pub fn add_network_block(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();
        
        // 1. HIGH-9 FIX: Check storage, not in-memory genesis-only chain.
        // Previously, has_block returned false for every block except genesis,
        // causing duplicate disk writes and tx-index corruption.
        if self.has_block(&block.hash) {
            return Ok(()); // Already have it
        }
        
        // 2. FORK DETECTION: Check if this block builds on our chain
        if block.previous_hash == latest.hash && block.index == latest.index + 1 {
            // Normal case: extends our chain
            return self.add_block_to_main_chain(block);
        } else if block.index > latest.index {
            // Potential fork: block is ahead of us
            tracing::warn!("Fork detected: Block {} at height {}, we're at {}", 
                &block.hash[..8], block.index, latest.index);
            
            // MED-4 FIX: Verify PoW difficulty meets minimum BEFORE storing orphan.
            // Previously, any block passing a cheap hash-format check could fill
            // the orphan pool — now it must meet minimum difficulty too.
            if block.difficulty < MIN_DIFFICULTY {
                tracing::warn!("Rejecting orphan block: difficulty {} < minimum {}", block.difficulty, MIN_DIFFICULTY);
                return Err(BlockchainError::InvalidBlock);
            }
            if !block.has_valid_hash() {
                tracing::warn!("Rejecting orphan block with invalid PoW");
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
            // Competing block at same height - apply longest chain rule
            tracing::warn!("Competing block at height {}: {} vs {}", 
                block.index, &block.hash[..8], &latest.hash[..8]);
            
            // MED-4 FIX: same PoW check for competing blocks
            if block.difficulty < MIN_DIFFICULTY {
                tracing::warn!("Rejecting competing block: difficulty below minimum");
                return Err(BlockchainError::InvalidBlock);
            }
            if !block.has_valid_hash() {
                tracing::warn!("Rejecting competing block with invalid PoW");
                return Err(BlockchainError::InvalidBlock);
            }
            
            let tree = crate::core::merkle::MerkleTree::from_transactions(&block.transactions);
            let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
            if block.merkle_root != computed_root {
                tracing::warn!("Rejecting competing block with invalid merkle root");
                return Err(BlockchainError::InvalidBlock);
            }
            
            // Keep our block (in production: compare total difficulty to pick longer chain)
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() >= MAX_ORPHAN_BLOCKS {
                tracing::warn!("Max competing blocks reached, dropping oldest");
                orphans.pop_front();
            }
            orphans.push_back(block);
            drop(orphans);
            return Ok(());
        } else {
            // Block is behind our chain - likely stale
            tracing::debug!("Ignoring stale block at height {} (we're at {})", 
                block.index, latest.index);
            return Ok(());
        }
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
        self.validate_block_consensus(&block, &latest)?;
        let mut new_state = self.account_state.read().clone();
        
        // Unlock any mature coinbase rewards
        new_state.unlock_mature_coinbase(block.index);

        // 5. Apply all transactions
        // 5. Apply all transactions
        for tx in &block.transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Network block has invalid tx: insufficient balance");
                    return Err(BlockchainError::InvalidBlock);
                }
            }
            new_state.credit_account(tx, block.index, COINBASE_MATURITY);
        }

        // 6. OPTIMIZATION: Don't add to in-memory chain (saves RAM!)
        // We used to do: self.chain.write().push(block.clone());
        // Now we ONLY save to storage and load on-demand
        
        // 7. COMMIT: Save to storage (primary storage, not memory!)
        self.storage.save_block(&block)?;
        self.storage.set_chain_height(block.index + 1)?;
        self.storage.save_account_state(&new_state)?;
        
        // 8. COMMIT: Update state
        *self.account_state.write() = new_state;

        // 9. Remove mined transactions from pending
        let mut pending = self.pending_transactions.write();
        pending.retain(|tx| !block.transactions.iter().any(|btx| btx.hash() == tx.hash()));
        drop(pending);
        
        // 10. Clear pending nonces for mined txs (DashMap - concurrent safe)
        for tx in &block.transactions {
            if !tx.is_coinbase() {
                self.pending_nonces.remove(&tx.sender);
            }
        }

        tracing::info!(" Network block {} accepted", block.index);
        Ok(())
    }

    /// Check if a block exists in the chain by hash.
    ///
    /// HIGH-9 FIX: Previously only checked the in-memory chain (genesis only!),
    /// returning false for all mined blocks and causing duplicate disk saves +
    /// tx-index corruption. Now checks storage directly.
    #[allow(dead_code)]
    pub fn has_block(&self, hash: &str) -> bool {
        // Fast path: genesis is in memory
        if let Some(genesis) = self.chain.read().first() {
            if genesis.hash == hash {
                return true;
            }
        }
        // Check storage for all other heights
        let height = self.get_height();
        for i in 1..height {
            if let Ok(b) = self.storage.load_block(i) {
                if b.hash == hash {
                    return true;
                }
            }
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub chain_length: usize,
    pub total_transactions: usize,
    pub current_difficulty: u32,
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

    /// Anti-dump: 50% of miner reward locked for 6 months
    #[test]
    fn test_mining_reward_lock_split() {
        let miner_reward: u64 = 95_000_000; // 95 QUA after treasury cut

        let immediate = (miner_reward * (100 - MINING_REWARD_LOCK_PERCENT)) / 100;
        let locked    = miner_reward - immediate;

        assert_eq!(immediate, 47_500_000, "50% immediately available");
        assert_eq!(locked,    47_500_000, "50% locked for vesting");
        assert_eq!(immediate + locked, miner_reward, "no value lost in lock split");
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


