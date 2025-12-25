use crate::block::Block;
use crate::transaction::{Transaction, UTXOSet};
use crate::storage::{BlockchainStorage, StorageError};
use serde::{Serialize, Deserialize};
use parking_lot::RwLock;
use std::sync::Arc;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlockchainError {
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Insufficient balance: required {required}, available {available}")]
    InsufficientBalance { required: f64, available: f64 },
    #[error("Transaction already pending")]
    DuplicateTransaction,
    #[error("Invalid block")]
    InvalidBlock,
}

const TARGET_BLOCK_TIME: u64 = 10; // 10 seconds
const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 10; // Adjust every 10 blocks
const INITIAL_MINING_REWARD: f64 = 50.0;
const HALVING_INTERVAL: u64 = 210; // Reward halves every 210 blocks

/// Thread-safe blockchain with persistent storage
pub struct Blockchain {
    chain: Arc<RwLock<Vec<Block>>>,
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    utxo_set: Arc<RwLock<UTXOSet>>,
    difficulty: Arc<RwLock<u32>>,
    storage: Arc<BlockchainStorage>,
}

impl Blockchain {
    /// Create or load blockchain from storage
    pub fn new(storage: Arc<BlockchainStorage>) -> Result<Self, BlockchainError> {
        // Try to load existing chain
        let chain = storage.load_chain()?;
        let utxo_set = storage.load_utxo_set()?.unwrap_or_else(UTXOSet::new);
        
        let (chain, utxo_set, difficulty) = if chain.is_empty() {
            // Create genesis block
            tracing::info!("Creating new blockchain with genesis block");
            let genesis = Block::genesis();
            let mut utxo_set = UTXOSet::new();
            
            // Genesis distribution
            let genesis_address = "0000000000000000000000000000000000000000";
            let genesis_tx = Transaction {
                sender: "COINBASE".to_string(),
                recipient: genesis_address.to_string(),
                amount: 1000.0,
                timestamp: genesis.timestamp,
                signature: vec![],
                public_key: vec![],
                fee: 0.0,
            };
            utxo_set.add_utxo(&genesis_tx);
            
            storage.save_block(&genesis)?;
            storage.set_chain_height(1)?;
            storage.save_utxo_set(&utxo_set)?;
            
            (vec![genesis], utxo_set, 4)
        } else {
            tracing::info!("Loaded existing blockchain with {} blocks", chain.len());
            let difficulty = chain.last().map(|b| b.difficulty).unwrap_or(4);
            (chain, utxo_set, difficulty)
        };

        Ok(Self {
            chain: Arc::new(RwLock::new(chain)),
            pending_transactions: Arc::new(RwLock::new(Vec::new())),
            utxo_set: Arc::new(RwLock::new(utxo_set)),
            difficulty: Arc::new(RwLock::new(difficulty)),
            storage,
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

        // Verify signature
        if !transaction.verify() {
            return Err(BlockchainError::InvalidSignature);
        }

        // Check sender has sufficient balance (amount + fee)
        let total_required = transaction.amount + transaction.fee;
        let available = self.utxo_set.read().get_balance(&transaction.sender);
        
        if available < total_required {
            return Err(BlockchainError::InsufficientBalance {
                required: total_required,
                available,
            });
        }

        // Check for double-spending
        let pending = self.pending_transactions.read();
        for pending_tx in pending.iter() {
            if pending_tx.sender == transaction.sender {
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
        let difficulty = *self.difficulty.read();
        
        // Get pending transactions
        let mut pending_txs = self.pending_transactions.write();
        let transactions = pending_txs.clone();
        
        // Create coinbase transaction
        let total_fees: f64 = transactions.iter().map(|tx| tx.fee).sum();
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: miner_address.clone(),
            amount: reward + total_fees,
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0.0,
        };

        let mut all_transactions = vec![coinbase_tx.clone()];
        all_transactions.extend(transactions);

        // Update UTXO set
        let mut utxo_set = self.utxo_set.write();
        for tx in &all_transactions {
            if !tx.is_coinbase() {
                let total = tx.amount + tx.fee;
                if !utxo_set.spend_utxos(&tx.sender, total) {
                    tracing::warn!("Failed to spend UTXOs for {}", tx.sender);
                    continue;
                }
            }
            utxo_set.add_utxo(tx);
        }
        drop(utxo_set);

        // Create and mine new block
        let previous_hash = self.get_latest_block().hash.clone();
        let index = self.chain.read().len() as u64;
        let mut new_block = Block::new(index, all_transactions, previous_hash, difficulty);
        
        new_block.mine();

        // Save to disk
        self.storage.save_block(&new_block)?;
        self.storage.set_chain_height(index + 1)?;
        self.storage.save_utxo_set(&self.utxo_set.read())?;

        // Add block to chain
        self.chain.write().push(new_block);
        pending_txs.clear();
        drop(pending_txs);

        // Adjust difficulty
        self.adjust_difficulty();
        
        tracing::info!("Block {} mined successfully", index);
        Ok(())
    }

    /// Get current mining reward with halving
    fn get_mining_reward(&self) -> f64 {
        let chain_len = self.chain.read().len() as u64;
        let halvings = chain_len / HALVING_INTERVAL;
        INITIAL_MINING_REWARD / 2_f64.powi(halvings as i32)
    }

    /// Adjust mining difficulty based on block time
    fn adjust_difficulty(&self) {
        let chain = self.chain.read();
        let chain_len = chain.len();
        
        if chain_len % DIFFICULTY_ADJUSTMENT_INTERVAL as usize != 0 {
            return;
        }

        if chain_len < DIFFICULTY_ADJUSTMENT_INTERVAL as usize {
            return;
        }

        let last_adjustment_block = &chain[chain_len - DIFFICULTY_ADJUSTMENT_INTERVAL as usize];
        let latest_block = chain.last().unwrap();
        
        let time_taken = (latest_block.timestamp - last_adjustment_block.timestamp) as u64;
        let expected_time = TARGET_BLOCK_TIME * DIFFICULTY_ADJUSTMENT_INTERVAL;

        let mut difficulty = self.difficulty.write();
        if time_taken < expected_time / 2 {
            *difficulty += 1;
            tracing::info!("Difficulty increased to {}", *difficulty);
        } else if time_taken > expected_time * 2 && *difficulty > 1 {
            *difficulty -= 1;
            tracing::info!("Difficulty decreased to {}", *difficulty);
        }
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
            current_difficulty: *self.difficulty.read(),
            mining_reward: self.get_mining_reward(),
            total_supply,
            pending_transactions: pending.len(),
        }
    }

    /// Calculate total coin supply
    fn calculate_total_supply(&self) -> f64 {
        let chain = self.chain.read();
        chain
            .iter()
            .flat_map(|block| &block.transactions)
            .filter(|tx| tx.is_coinbase())
            .map(|tx| tx.amount)
            .sum()
    }

    /// Get balance for an address
    pub fn get_balance(&self, address: &str) -> f64 {
        self.utxo_set.read().get_balance(address)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BlockchainStats {
    pub chain_length: usize,
    pub total_transactions: usize,
    pub current_difficulty: u32,
    pub mining_reward: f64,
    pub total_supply: f64,
    pub pending_transactions: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blockchain_creation() {
        let blockchain = Blockchain::new();
        assert_eq!(blockchain.chain.len(), 1);
        assert!(blockchain.is_valid());
    }

    #[test]
    fn test_mining_reward_halving() {
        let mut blockchain = Blockchain::new();
        assert_eq!(blockchain.get_mining_reward(), 50.0);
        
        // Simulate 210 blocks
        for _ in 0..210 {
            blockchain.chain.push(Block::genesis());
        }
        assert_eq!(blockchain.get_mining_reward(), 25.0);
    }
}
