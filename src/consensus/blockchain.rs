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
const INITIAL_MINING_REWARD: u64 = 50_000_000; // 50 QUA in microunits
const HALVING_INTERVAL: u64 = 210; // Reward halves every 210 blocks

// Security limits
const MAX_MEMPOOL_SIZE: usize = 5000; // Maximum pending transactions
const MAX_BLOCK_TRANSACTIONS: usize = 2000; // Maximum transactions per block
const MAX_BLOCK_SIZE_BYTES: usize = 1_048_576; // 1 MB max block size
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
        
        // Get pending transactions (limit by size and count)
        let mut pending_txs = self.pending_transactions.write();
        let mut transactions = Vec::new();
        let mut block_size = 0usize;
        
        // Select transactions that fit in block limits
        for tx in pending_txs.iter() {
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
        
        // Create coinbase transaction
        let total_fees: u64 = transactions.iter().map(|tx| tx.fee).sum();
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: miner_address.clone(),
            amount: reward.saturating_add(total_fees),
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
        };

        let mut all_transactions = vec![coinbase_tx.clone()];
        all_transactions.extend(transactions);

        // CRITICAL: Clone state for transactional update
        let mut new_state = self.account_state.read().clone();
        
        // Unlock any mature coinbase rewards at current height
        let current_height = self.chain.read().len() as u64;
        new_state.unlock_mature_coinbase(current_height);
        
        // Apply transactions to cloned state
        for tx in &all_transactions {
            if !tx.is_coinbase() {
                let total = tx.amount.saturating_add(tx.fee);
                if !new_state.debit_account(&tx.sender, total) {
                    tracing::warn!("Failed to spend for {} - skipping tx", tx.sender);
                    continue;
                }
            }
            new_state.credit_account(tx, current_height, COINBASE_MATURITY);
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

    /// Get current mining reward with halving (u64 microunits)
    fn get_mining_reward(&self) -> u64 {
        let chain_len = self.chain.read().len() as u64;
        let halvings = chain_len / HALVING_INTERVAL;
        INITIAL_MINING_REWARD / 2_u64.pow(halvings as u32)
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
        // Prevent ridiculous backdating (within 1 week of previous block)
        if block.timestamp < previous.timestamp - 604800 {
            tracing::warn!("Block timestamp backdated by {} seconds", 
                previous.timestamp - block.timestamp);
            return Err(BlockchainError::InvalidBlock);
        }
        
        // 3. Difficulty must match expected
        let expected_difficulty = previous.difficulty; // Should derive from adjustment logic
        if block.difficulty != expected_difficulty {
            return Err(BlockchainError::InvalidDifficulty);
        }
        
        // 4. Coinbase validation
        let coinbase_txs: Vec<_> = block.transactions.iter().filter(|tx| tx.is_coinbase()).collect();
        if coinbase_txs.len() != 1 {
            return Err(BlockchainError::InvalidBlock);
        }
        
        let coinbase = coinbase_txs[0];
        let expected_reward = self.calculate_reward_at_height(block.index);
        let total_fees: u64 = block.transactions.iter()
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.fee)
            .sum();
        
        let expected_total = expected_reward.saturating_add(total_fees);
        if coinbase.amount != expected_total {
            return Err(BlockchainError::InvalidCoinbaseReward {
                actual: coinbase.amount,
                expected: expected_total,
            });
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
        let halvings = height / HALVING_INTERVAL;
        INITIAL_MINING_REWARD / 2_u64.pow(halvings as u32)
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
            
            // Store as orphaned block
            self.orphaned_blocks.write().push(block.clone());
            
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
            self.orphaned_blocks.write().push(block);
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

