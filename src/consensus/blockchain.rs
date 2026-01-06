use crate::core::block::Block;
use crate::core::transaction::{Transaction, AccountState};
use crate::storage::{BlockchainStorage, StorageError};
use serde::{Serialize, Deserialize};
use parking_lot::RwLock;
use std::sync::Arc;
use thiserror::Error;
use dashmap::DashMap;

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

const TARGET_BLOCK_TIME: u64 = 10; // 10 seconds
const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 10; // Adjust every 10 blocks

// MODERN ADAPTIVE TOKENOMICS (Option 3 - Solana-style)
const YEAR_1_REWARD: u64 = 100_000_000; // 100 QUA in microunits
const ANNUAL_REDUCTION_PERCENT: u64 = 15; // 15% reduction per year (faster value creation)
const MIN_REWARD: u64 = 5_000_000; // 5 QUA floor (reached after ~20 years)
const BLOCKS_PER_YEAR: u64 = 3_153_600; // 365.25 days * 86400 / 10 seconds

// UNIQUE FEATURES - Early Adopter Incentives
const EARLY_ADOPTER_BONUS_BLOCKS: u64 = 100_000; // First ~11.5 days
const EARLY_ADOPTER_MULTIPLIER: f64 = 1.5; // 1.5x rewards for early miners
const BOOTSTRAP_PHASE_BLOCKS: u64 = 315_360; // First month gets network usage boost

// SUSTAINABLE ECONOMICS - Fee Structure & Value Capture
const BASE_TRANSACTION_FEE: u64 = 1_000; // 0.001 QUA minimum (prevents spam)
const FEE_BURN_PERCENT: u64 = 70; // 70% of fees burned (deflationary pressure)
const FEE_TREASURY_PERCENT: u64 = 20; // 20% to development treasury
const FEE_VALIDATOR_PERCENT: u64 = 10; // 10% to block validator (miner)

// TREASURY FUND - Development, Marketing, Listings
const TREASURY_ALLOCATION_PERCENT: u64 = 5; // 5% of block rewards → treasury
const TREASURY_ADDRESS: &str = "0x0000000000000000000000000000000000000001"; // Hardcoded treasury

// ANTI-DUMP MECHANISM - Mining Reward Lockup
const MINING_REWARD_LOCK_PERCENT: u64 = 50; // 50% of mining rewards locked
const MINING_REWARD_LOCK_BLOCKS: u64 = 157_680; // 6 months vesting (182.5 days)

// Security limits
const MAX_MEMPOOL_SIZE: usize = 5000; // Maximum pending transactions
const MAX_BLOCK_TRANSACTIONS: usize = 2000; // Maximum transactions per block
const MAX_BLOCK_SIZE_BYTES: usize = 1_048_576; // 1 MB max block size
const MAX_ORPHAN_BLOCKS: usize = 100; // Maximum orphaned blocks (prevents memory exhaustion)
const MAX_TRANSACTION_SIZE_BYTES: usize = 102400; // 100KB max per transaction (prevents DOS)
const MIN_TRANSACTION_FEE: u64 = 100; // 0.0001 QUA in microunits
const TRANSACTION_EXPIRY_SECONDS: i64 = 86400; // 24 hours
const COINBASE_MATURITY: u64 = 100; // Blocks before coinbase can be spent
const MAX_FUTURE_BLOCK_TIME: i64 = 7200; // 2 hours maximum future timestamp

// CONSENSUS-CRITICAL: Genesis block hash (prevents chain split attacks)
// Generated from Block::genesis() with timestamp 1735689600 (2026-01-01 00:00:00 UTC)
// Difficulty: 6 (PRODUCTION)
//  VERIFIED: 2026-01-04 - Hash regenerated with correct parameters
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

/// Thread-safe blockchain with persistent storage
pub struct Blockchain {
    chain: Arc<RwLock<Vec<Block>>>,
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    account_state: Arc<RwLock<AccountState>>,
    pending_nonces: Arc<DashMap<String, u64>>, // ATOMIC: Track highest pending nonce (fixes race condition)
    storage: Arc<BlockchainStorage>,
    orphaned_blocks: Arc<RwLock<Vec<Block>>>, // Store competing chain blocks for fork resolution
}

impl Blockchain {
    /// Create or load blockchain from storage
    pub fn new(storage: Arc<BlockchainStorage>) -> Result<Self, BlockchainError> {
        // Try to load existing chain
        let chain = storage.load_chain()?;
        let account_state = storage.load_account_state()?.unwrap_or_else(AccountState::new);
        
        let (chain, account_state, _difficulty) = if chain.is_empty() {
            // Create genesis block
            tracing::info!("Creating new blockchain with genesis block");
            let genesis = Block::genesis();
            
            // SECURITY: Verify genesis hash matches hardcoded value (prevents chain split)
            if genesis.hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block hash mismatch!\nExpected: {}\nGot: {}\nThis indicates tampering or incorrect genesis generation.", 
                    GENESIS_HASH, genesis.hash);
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
            };
            account_state.credit_account(&genesis_tx, 0, COINBASE_MATURITY);
            
            storage.save_block(&genesis)?;
            storage.set_chain_height(1)?;
            storage.save_account_state(&account_state)?;
            
            tracing::info!(" Genesis block verified: {}", GENESIS_HASH);
            (vec![genesis], account_state, 4)
        } else {
            tracing::info!("Loaded existing blockchain with {} blocks", chain.len());
            
            // SECURITY: Verify genesis block on load (prevents database tampering)
            if !chain.is_empty() && chain[0].hash != GENESIS_HASH {
                panic!("CRITICAL: Genesis block mismatch in existing chain!\nExpected: {}\nGot: {}\nDatabase may be corrupted or from different network.", 
                    GENESIS_HASH, chain[0].hash);
            }
            
            let difficulty = chain.last().map(|b| b.difficulty).unwrap_or(4);
            (chain, account_state, difficulty)
        };

        Ok(Self {
            chain: Arc::new(RwLock::new(chain)),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            account_state: Arc::new(RwLock::new(account_state)),
            pending_nonces: Arc::new(DashMap::new()), // Concurrent HashMap - no lock needed
            storage,
            orphaned_blocks: Arc::new(RwLock::new(Vec::new())),
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
        self.chain.read().last().unwrap().clone()
    }

    /// Add a new transaction to the mempool
    pub fn add_transaction(&self, transaction: Transaction) -> Result<(), BlockchainError> {
        // Skip validation for coinbase transactions
        if transaction.is_coinbase() {
            self.pending_transactions.write().push(transaction);
            return Ok(());
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
        
        // CRITICAL FIX: Atomic check-and-increment using DashMap
        // This prevents two parallel txs from using the same nonce
        let expected_nonce = self.pending_nonces
            .entry(transaction.sender.clone())
            .or_insert(chain_nonce)
            .value()
            .max(&chain_nonce) + 1;
        
        if transaction.nonce != expected_nonce {
            return Err(BlockchainError::InvalidNonce {
                expected: expected_nonce,
                actual: transaction.nonce,
            });
        }
        
        // ATOMIC: Update pending nonce (no race - single map entry lock)
        self.pending_nonces.insert(transaction.sender.clone(), transaction.nonce);

        // Check transaction size limit (DOS protection - prevents huge DeployContract)
        let tx_size = bincode::serialize(&transaction).map_err(|_| BlockchainError::InvalidBlock)?.len();
        if tx_size > MAX_TRANSACTION_SIZE_BYTES {
            return Err(BlockchainError::BlockTooLarge { size: tx_size }); // Reuse error type
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

        // Check for duplicate by hash (not sender - multiple txs from same sender OK if nonces differ)
        let tx_hash = transaction.hash();
        let pending = self.pending_transactions.read();
        for pending_tx in pending.iter() {
            if pending_tx.hash() == tx_hash {
                return Err(BlockchainError::DuplicateTransaction);
            }
        }
        drop(pending);

        self.pending_transactions.write().push(transaction);
        tracing::info!("Transaction added to mempool");
        Ok(())
    }

    /// Mine a new block with pending transactions
    pub fn mine_pending_transactions(&self, miner_address: String) -> Result<(), BlockchainError> {
        let reward = self.get_mining_reward();
        let difficulty = self.calculate_next_difficulty();
        
        // Get pending transactions sorted by fee (highest first) with size limits
        let mut pending_txs = self.pending_transactions.write();
        
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
        let fee_burned = (total_fees * FEE_BURN_PERCENT) / 100;
        let fee_to_treasury = (total_fees * FEE_TREASURY_PERCENT) / 100;
        let fee_to_miner = (total_fees * FEE_VALIDATOR_PERCENT) / 100;
        
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
            };
            all_transactions.push(treasury_tx);
        }
        
        all_transactions.extend(transactions);

        // CRITICAL: Clone state for transactional update
        let mut new_state = self.account_state.read().clone();
        
        // Unlock any mature coinbase rewards at current height
        let current_height = self.chain.read().len() as u64;
        new_state.unlock_mature_coinbase(current_height);
        
        // Apply transactions to cloned state
        for tx in &all_transactions {
            if !tx.is_coinbase() && tx.sender != "TREASURY" {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Failed to spend for {} - skipping tx", tx.sender);
                    continue;
                }
            }
            new_state.credit_account(tx, current_height, COINBASE_MATURITY);
        }
        
        // ANTI-DUMP: Add locked mining reward (50% vested over 6 months)
        if locked_reward > 0 {
            let unlock_height = current_height + MINING_REWARD_LOCK_BLOCKS;
            new_state.add_locked_balance(&miner_address, locked_reward, unlock_height);
            tracing::info!(
                "Locked {} QUA for miner {} until block {}",
                locked_reward / 1_000_000, miner_address, unlock_height
            );
        }

        // Create and mine new block
        let previous_hash = self.get_latest_block().hash.clone();
        let index = self.chain.read().len() as u64;
        let mut new_block = Block::new(index, all_transactions, previous_hash, difficulty);
        
        new_block.mine();
        
        // Validate block before committing (paranoid but correct)
        let latest = self.get_latest_block();
        if !new_block.is_valid(Some(&latest)) {
            return Err(BlockchainError::InvalidBlock);
        }

        // COMMIT: Save to disk first (durability)
        self.storage.save_block(&new_block)?;
        self.storage.set_chain_height(index + 1)?;
        self.storage.save_account_state(&new_state)?;

        // COMMIT: Update in-memory state (atomicity)
        *self.account_state.write() = new_state;
        self.chain.write().push(new_block.clone());
        
        // Remove only mined transactions from mempool
        pending_txs.retain(|tx| !new_block.transactions.iter().any(|btx| btx.hash() == tx.hash()));
        drop(pending_txs);
        
        // Clear pending nonces for mined txs (DashMap - no lock needed)
        for tx in &new_block.transactions {
            if !tx.is_coinbase() {
                self.pending_nonces.remove(&tx.sender);
            }
        }
        
        tracing::info!(" Block {} mined: {} txs, reward {} microunits", index, new_block.transactions.len(), reward);
        Ok(())
    }

    /// Get current mining reward with adaptive model (u64 microunits)
    fn get_mining_reward(&self) -> u64 {
        let chain_len = self.chain.read().len() as u64;
        
        // Calculate base reward with annual reduction
        let years_elapsed = chain_len / BLOCKS_PER_YEAR;
        let reduction_factor = (100 - ANNUAL_REDUCTION_PERCENT) as f64 / 100.0;
        let base_reward = (YEAR_1_REWARD as f64 * reduction_factor.powi(years_elapsed as i32)).round() as u64;
        
        // Apply minimum floor
        let base_reward = base_reward.max(MIN_REWARD);
        
        // UNIQUE FEATURE 1: Early adopter bonus (first 100k blocks)
        let reward_with_bonus = if chain_len < EARLY_ADOPTER_BONUS_BLOCKS {
            (base_reward as f64 * EARLY_ADOPTER_MULTIPLIER).round() as u64
        } else {
            base_reward
        };
        
        // UNIQUE FEATURE 2: Network usage adjustment during bootstrap
        let final_reward = if chain_len < BOOTSTRAP_PHASE_BLOCKS {
            // During bootstrap, adjust based on transaction activity
            let usage_factor = self.get_usage_factor();
            let adjusted = (reward_with_bonus as f64 * usage_factor).round() as u64;
            // Clamp between base and 2x base (encourages transaction activity)
            adjusted.clamp(reward_with_bonus, reward_with_bonus * 2)
        } else {
            reward_with_bonus
        };
        
        final_reward
    }
    
    /// Calculate network usage factor (1.0 = baseline, up to 2.0 during high activity)
    /// ANTI-SPAM: Weighted by TOTAL FEES PAID, not transaction count
    fn get_usage_factor(&self) -> f64 {
        let recent_blocks = 100.min(self.chain.read().len());
        if recent_blocks < 10 {
            return 1.0; // Not enough data
        }
        
        let chain = self.chain.read();
        let start_idx = chain.len().saturating_sub(recent_blocks);
        let recent = &chain[start_idx..];
        
        // Sum total fees paid in recent blocks (excludes coinbase which has 0 fee)
        let total_fees: u64 = recent.iter()
            .flat_map(|b| b.transactions.iter())
            .filter(|tx| tx.sender != "COINBASE") // Exclude coinbase
            .map(|tx| tx.fee)
            .sum();
        
        // Average fees per block (in QUA)
        let avg_fees_per_block = (total_fees as f64 / recent_blocks as f64) / 1_000_000.0; // Convert to QUA
        
        // Factor based on economic activity (fee spending), not spam count
        // 0 fees → 1.0x, 50 QUA avg fees/block → 2.0x
        // This makes spam UNPROFITABLE (must pay real fees to boost rewards)
        (1.0 + (avg_fees_per_block / 50.0).min(1.0)).min(2.0)
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
        // Prevent backdating/forward-dating (within 2 hours of previous block)
        const MAX_TIME_DELTA: i64 = 7200; // 2 hours
        if block.timestamp > previous.timestamp + MAX_TIME_DELTA ||
           block.timestamp < previous.timestamp - MAX_TIME_DELTA {
            tracing::warn!("Block timestamp {} outside acceptable range (prev: {}, delta: {})", 
                block.timestamp, previous.timestamp, block.timestamp - previous.timestamp);
            return Err(BlockchainError::InvalidBlock);
        }
        
        // 3. Difficulty must match expected
        let expected_difficulty = previous.difficulty; // Should derive from adjustment logic
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
        
        for tx in &block.transactions {
            if !tx.is_coinbase() {
                if !tx.verify() {
                    return Err(BlockchainError::InvalidSignature);
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
        }
        
        Ok(())
    }
    
    /// Calculate reward at specific height (for validation)
    fn calculate_reward_at_height(&self, height: u64) -> u64 {
        // Use same logic as get_mining_reward but with specified height
        let years_elapsed = height / BLOCKS_PER_YEAR;
        let reduction_factor = (100 - ANNUAL_REDUCTION_PERCENT) as f64 / 100.0;
        let base_reward = (YEAR_1_REWARD as f64 * reduction_factor.powi(years_elapsed as i32)).round() as u64;
        let base_reward = base_reward.max(MIN_REWARD);
        
        // Apply early adopter bonus if applicable
        if height < EARLY_ADOPTER_BONUS_BLOCKS {
            (base_reward as f64 * EARLY_ADOPTER_MULTIPLIER).round() as u64
        } else {
            base_reward
        }
        // Note: Usage factor not included here since it's dynamic and based on recent blocks
    }

    /// Calculate next difficulty (pure function, deterministic)
    fn calculate_next_difficulty(&self) -> u32 {
        let chain = self.chain.read();
        let chain_len = chain.len();
        
        // Not enough blocks yet - use initial difficulty
        if chain_len < DIFFICULTY_ADJUSTMENT_INTERVAL as usize {
            return chain.last().unwrap().difficulty;
        }
        
        // Only adjust at intervals
        if chain_len % DIFFICULTY_ADJUSTMENT_INTERVAL as usize != 0 {
            return chain.last().unwrap().difficulty;
        }
        
        let latest_block = chain.last().unwrap();
        let adjustment_start = chain_len.saturating_sub(DIFFICULTY_ADJUSTMENT_INTERVAL as usize);
        let start_block = &chain[adjustment_start];
        
        // Calculate actual time taken for last N blocks
        let actual_time = latest_block.timestamp - start_block.timestamp;
        let expected_time = (TARGET_BLOCK_TIME * DIFFICULTY_ADJUSTMENT_INTERVAL) as i64;
        
        // SECURITY: Limit adjustment range to prevent manipulation (Bitcoin-style: 4x max)
        let actual_time_clamped = actual_time.max(expected_time / 4).min(expected_time * 4);
        
        let current_difficulty = latest_block.difficulty as i64;
        
        // Adjust difficulty proportionally (clamped to ±25% per adjustment)
        let new_difficulty_raw = (current_difficulty * expected_time) / actual_time_clamped;
        let new_difficulty = new_difficulty_raw
            .max(current_difficulty * 3 / 4)  // Max decrease 25%
            .min(current_difficulty * 5 / 4)  // Max increase 25%
            .max(4)                           // Minimum difficulty
            .min(32) as u32;                  // Maximum difficulty (prevents overflow)
        
        tracing::info!("Difficulty adjustment: {} -> {} (actual time: {}s, expected: {}s)",
            current_difficulty, new_difficulty, actual_time, expected_time);
        
        new_difficulty
    }

    /// Validate the entire blockchain
    pub fn is_valid(&self) -> bool {
        let chain = self.chain.read();
        
        if chain[0].index != 0 {
            tracing::error!("Invalid genesis block");
            return false;
        }

        for i in 1..chain.len() {
            let current_block = &chain[i];
            let previous_block = &chain[i - 1];

            if !current_block.is_valid(Some(previous_block)) {
                tracing::error!("Block {} is invalid", i);
                return false;
            }
        }

        true
    }

    /// Get blockchain statistics
    pub fn get_stats(&self) -> BlockchainStats {
        let chain = self.chain.read();
        let total_transactions: usize = chain.iter().map(|b| b.transactions.len()).sum();
        let total_supply = self.calculate_total_supply();
        let pending = self.pending_transactions.read();
        
        BlockchainStats {
            chain_length: chain.len(),
            total_transactions,
            current_difficulty: self.get_current_difficulty(),
            mining_reward: self.get_mining_reward(),
            total_supply,
            pending_transactions: pending.len(),
        }
    }

    /// Calculate total coin supply (u64 microunits)
    fn calculate_total_supply(&self) -> u64 {
        let chain = self.chain.read();
        chain
            .iter()
            .flat_map(|block| &block.transactions)
            .filter(|tx| tx.is_coinbase())
            .map(|tx| tx.amount)
            .sum()
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

    /// Get account state (mutable)
    pub fn get_account_state_mut(&self) -> parking_lot::RwLockWriteGuard<'_, AccountState> {
        self.account_state.write()
    }

    /// Add a block received from the network (WITH FULL VALIDATION AND FORK RESOLUTION)
    pub fn add_network_block(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();
        
        // 1. Check if we already have this block
        let chain = self.chain.read();
        if chain.iter().any(|b| b.hash == block.hash) {
            return Ok(()); // Already have it
        }
        drop(chain);
        
        // 2. FORK DETECTION: Check if this block builds on our chain
        if block.previous_hash == latest.hash && block.index == latest.index + 1 {
            // Normal case: extends our chain
            return self.add_block_to_main_chain(block);
        } else if block.index > latest.index {
            // Potential fork: block is ahead of us
            tracing::warn!("Fork detected: Block {} at height {}, we're at {}", 
                &block.hash[..8], block.index, latest.index);
            
            // Store as orphaned block (with size limit)
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() >= MAX_ORPHAN_BLOCKS {
                tracing::warn!("Max orphan blocks reached, dropping oldest");
                orphans.remove(0);
            }
            orphans.push(block.clone());
            drop(orphans);
            
            // Try to resolve fork by fetching missing blocks
            // (This would trigger sync - simplified for now)
            tracing::info!("Stored orphaned block, need to sync");
            return Ok(());
        } else if block.index == latest.index {
            // Competing block at same height - apply longest chain rule
            tracing::warn!("Competing block at height {}: {} vs {}", 
                block.index, &block.hash[..8], &latest.hash[..8]);
            
            // For now, keep our block (in production: compare total work)
            // TODO: Implement total difficulty comparison
            let mut orphans = self.orphaned_blocks.write();
            if orphans.len() >= MAX_ORPHAN_BLOCKS {
                tracing::warn!("Max orphan blocks reached, dropping oldest");
                orphans.remove(0);
            }
            orphans.push(block);
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
        for tx in &block.transactions {
            if !tx.is_coinbase() {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Network block has invalid tx: insufficient balance");
                    return Err(BlockchainError::InvalidBlock);
                }
            }
            new_state.credit_account(tx, block.index, COINBASE_MATURITY);
        }

        // 6. COMMIT: Add to chain
        self.chain.write().push(block.clone());
        
        // 7. COMMIT: Save to storage
        self.storage.save_block(&block)?;
        self.storage.set_chain_height(self.get_latest_block().index + 1)?;
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

    /// Check if a block exists in the chain
    #[allow(dead_code)]
    pub fn has_block(&self, hash: &str) -> bool {
        let chain = self.chain.read();
        chain.iter().any(|b| b.hash == hash)
    }

    /// Get block by height
    #[allow(dead_code)]
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let chain = self.chain.read();
        chain.get(height as usize).cloned()
    }

    /// Get current chain height
    pub fn get_height(&self) -> u64 {
        self.chain.read().len() as u64
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

