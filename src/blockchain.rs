use crate::block::Block;
use crate::transaction::{Transaction, UTXOSet, TransactionType};
use crate::storage::{BlockchainStorage, StorageError};
use crate::contract_executor::{ContractExecutor, MAX_GAS_PER_TX};
use crate::contract::{Account, ContractInstruction};
use serde::{Serialize, Deserialize};
use parking_lot::RwLock;
use std::sync::Arc;
use std::collections::HashMap;
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
    #[error("Mempool full: {0} transactions")]
    MempoolFull(usize),
    #[error("Fee too low: {fee}, minimum: {min}")]
    FeeTooLow { fee: f64, min: f64 },
    #[error("Transaction expired")]
    TransactionExpired,
    #[error("Block too large: {size} bytes")]
    BlockTooLarge { size: usize },
    #[error("Contract error: {0}")]
    ContractError(String),
    #[error("Contract not found: {0}")]
    ContractNotFound(String),
}

const TARGET_BLOCK_TIME: u64 = 10; // 10 seconds
const DIFFICULTY_ADJUSTMENT_INTERVAL: u64 = 10; // Adjust every 10 blocks
const INITIAL_MINING_REWARD: f64 = 50.0;
const HALVING_INTERVAL: u64 = 210; // Reward halves every 210 blocks

// Security limits
const MAX_MEMPOOL_SIZE: usize = 5000; // Maximum pending transactions
const MAX_BLOCK_TRANSACTIONS: usize = 2000; // Maximum transactions per block
const MAX_BLOCK_SIZE_BYTES: usize = 1_048_576; // 1 MB max block size
const MIN_TRANSACTION_FEE: f64 = 0.0001; // Minimum fee to prevent spam
const TRANSACTION_EXPIRY_SECONDS: i64 = 86400; // 24 hours

/// Thread-safe blockchain with persistent storage
pub struct Blockchain {
    chain: Arc<RwLock<Vec<Block>>>,
    pending_transactions: Arc<RwLock<Vec<Transaction>>>,
    utxo_set: Arc<RwLock<UTXOSet>>,
    difficulty: Arc<RwLock<u32>>,
    storage: Arc<BlockchainStorage>,
    contract_executor: Arc<RwLock<ContractExecutor>>,
    // In-memory contract accounts (address -> Account)
    contract_accounts: Arc<RwLock<HashMap<String, Account>>>,
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
                tx_type: crate::transaction::TransactionType::Transfer,
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
            storage: storage.clone(),
            contract_executor: Arc::new(RwLock::new(ContractExecutor::new())),
            contract_accounts: Arc::new(RwLock::new(HashMap::new())),
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
        let total_fees: f64 = transactions.iter().map(|tx| tx.fee).sum();
        let coinbase_tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: miner_address.clone(),
            amount: reward + total_fees,
            timestamp: chrono::Utc::now().timestamp(),
            signature: vec![],
            public_key: vec![],
            fee: 0.0,
            tx_type: crate::transaction::TransactionType::Transfer,
        };

        let mut all_transactions = vec![coinbase_tx.clone()];
        all_transactions.extend(transactions);

        // Process transactions (including contract transactions)
        for tx in &all_transactions {
            if !tx.is_coinbase() {
                // Process based on transaction type
                match &tx.tx_type {
                    TransactionType::Transfer => {
                        // Regular transfer - handle with UTXO
                        let mut utxo_set = self.utxo_set.write();
                        let total = tx.amount + tx.fee;
                        if !utxo_set.spend_utxos(&tx.sender, total) {
                            tracing::warn!("Failed to spend UTXOs for {}", tx.sender);
                            continue;
                        }
                        utxo_set.add_utxo(tx);
                        drop(utxo_set);
                    }
                    TransactionType::DeployContract { code } => {
                        // Deploy contract
                        if let Err(e) = self.process_deploy_contract(tx, code) {
                            tracing::error!("Failed to deploy contract: {}", e);
                            continue;
                        }
                        // Deduct fee from deployer
                        let mut utxo_set = self.utxo_set.write();
                        let _ = utxo_set.spend_utxos(&tx.sender, tx.fee);
                        drop(utxo_set);
                    }
                    TransactionType::CallContract { contract, function, args } => {
                        // Execute contract
                        if let Err(e) = self.process_call_contract(tx, contract, function, args) {
                            tracing::error!("Failed to call contract: {}", e);
                            continue;
                        }
                        // Deduct fee from caller
                        let mut utxo_set = self.utxo_set.write();
                        let _ = utxo_set.spend_utxos(&tx.sender, tx.fee);
                        drop(utxo_set);
                    }
                }
            } else {
                // Coinbase transaction
                let mut utxo_set = self.utxo_set.write();
                utxo_set.add_utxo(tx);
                drop(utxo_set);
            }
        }

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
        self.chain.write().push(new_block.clone());
        
        // Remove only mined transactions from mempool
        pending_txs.retain(|tx| !new_block.transactions.contains(tx));
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

    /// Get the blockchain (for network sync)
    pub fn get_chain(&self) -> parking_lot::RwLockReadGuard<Vec<Block>> {
        self.chain.read()
    }

    /// Get mutable blockchain (for adding blocks from network)
    pub fn get_chain_mut(&self) -> parking_lot::RwLockWriteGuard<Vec<Block>> {
        self.chain.write()
    }

    /// Get pending transactions
    pub fn get_pending_transactions(&self) -> parking_lot::RwLockReadGuard<Vec<Transaction>> {
        self.pending_transactions.read()
    }

    /// Get mutable pending transactions
    pub fn get_pending_transactions_mut(&self) -> parking_lot::RwLockWriteGuard<Vec<Transaction>> {
        self.pending_transactions.write()
    }

    /// Get UTXO set
    pub fn get_utxo_set_mut(&self) -> parking_lot::RwLockWriteGuard<UTXOSet> {
        self.utxo_set.write()
    }

    /// Add a block received from the network
    pub fn add_network_block(&self, block: Block) -> Result<(), BlockchainError> {
        let latest = self.get_latest_block();
        
        // Validate block
        if !block.is_valid(Some(&latest)) {
            return Err(BlockchainError::InvalidBlock);
        }

        // Check if we already have this block
        let chain = self.chain.read();
        if chain.iter().any(|b| b.hash == block.hash) {
            return Ok(()); // Already have it
        }
        drop(chain);

        // Add to chain
        let mut chain = self.chain.write();
        chain.push(block.clone());
        drop(chain);

        // Save to storage
        self.storage.save_block(&block)?;
        self.storage.set_chain_height(self.get_latest_block().index + 1)?;

        // Update UTXO set
        let mut utxo_set = self.utxo_set.write();
        for tx in &block.transactions {
            if !tx.is_coinbase() {
                let _ = utxo_set.spend_utxos(&tx.sender, tx.amount + tx.fee);
            }
            utxo_set.add_utxo(tx);
        }
        drop(utxo_set);

        // Remove mined transactions from pending
        let mut pending = self.pending_transactions.write();
        pending.retain(|tx| !block.transactions.contains(tx));

        Ok(())
    }

    /// Check if a block exists in the chain
    pub fn has_block(&self, hash: &str) -> bool {
        let chain = self.chain.read();
        chain.iter().any(|b| b.hash == hash)
    }

    /// Get block by height
    pub fn get_block_by_height(&self, height: u64) -> Option<Block> {
        let chain = self.chain.read();
        chain.get(height as usize).cloned()
    }

    /// Get current chain height
    pub fn get_height(&self) -> u64 {
        self.chain.read().len() as u64
    }
    
    /// Deploy a smart contract
    pub fn deploy_contract(&self, deployer: &str, code: Vec<u8>) -> Result<String, BlockchainError> {
        // Generate contract address from deployer + code hash
        use sha3::{Sha3_256, Digest};
        let mut hasher = Sha3_256::new();
        hasher.update(deployer.as_bytes());
        hasher.update(&code);
        let hash = hasher.finalize();
        let address = hex::encode(&hash[..20]); // Use first 20 bytes
        
        // Check if contract already exists
        if self.storage.load_contract(&address)?.is_some() {
            return Err(BlockchainError::ContractError("Contract already exists".to_string()));
        }
        
        // Save contract code to storage
        self.storage.save_contract(&address, &code)?;
        
        // Create program account
        let program_account = Account::new_program(
            address.clone(),
            code.clone(),
            deployer.to_string(),
        );
        
        // Store in memory
        self.contract_accounts.write().insert(address.clone(), program_account);
        
        tracing::info!("Contract deployed at {}", address);
        Ok(address)
    }
    
    /// Call a smart contract function
    pub fn call_contract(
        &self,
        caller: &str,
        contract_address: &str,
        function: &str,
        args: Vec<u8>,
    ) -> Result<Vec<u8>, BlockchainError> {
        // Load contract code
        let code = self.storage.load_contract(contract_address)?
            .ok_or_else(|| BlockchainError::ContractNotFound(contract_address.to_string()))?;
        
        // Get or create caller account
        let mut accounts = self.contract_accounts.write();
        let caller_copy = accounts.entry(caller.to_string()).or_insert_with(|| {
            let balance = (self.utxo_set.read().get_balance(caller) * 1_000_000.0) as u64; // Convert to smallest unit
            Account::new_user(caller.to_string(), vec![], balance)
        }).clone();
        
        // Get contract account
        let contract_account = accounts.get(contract_address)
            .ok_or_else(|| BlockchainError::ContractNotFound(contract_address.to_string()))?
            .clone();
        
        drop(accounts);
        
        // Prepare instruction
        let mut instruction_data = function.as_bytes().to_vec();
        instruction_data.push(0); // null terminator
        instruction_data.extend_from_slice(&args);
        
        let instruction = ContractInstruction {
            program_id: contract_address.to_string(),
            accounts: vec![], // TODO: proper account metadata
            data: instruction_data,
        };
        
        // Generate quantum entropy from block height
        use sha3::{Sha3_256, Digest};
        let mut hasher = Sha3_256::new();
        hasher.update(self.get_height().to_le_bytes());
        let entropy_hash = hasher.finalize();
        let mut quantum_entropy = [0u8; 32];
        quantum_entropy.copy_from_slice(&entropy_hash[..]);
        
        // Execute contract
        let exec_accounts = vec![caller_copy, contract_account];
        let result = self.contract_executor.write().execute(
            &code,
            &instruction,
            exec_accounts,
            self.get_height(),
            quantum_entropy,
            MAX_GAS_PER_TX,
        ).map_err(|e| BlockchainError::ContractError(e.to_string()))?;
        
        tracing::info!("Contract {} called by {}: success={}, gas_used={}", 
            contract_address, caller, result.success, result.gas_used);
        Ok(result.return_data)
    }
    
    /// Process a contract deployment transaction
    fn process_deploy_contract(&self, tx: &Transaction, code: &[u8]) -> Result<(), BlockchainError> {
        // Deployer must have sufficient balance for fees
        let available = self.utxo_set.read().get_balance(&tx.sender);
        if available < tx.fee {
            return Err(BlockchainError::InsufficientBalance {
                required: tx.fee,
                available,
            });
        }
        
        // Deploy the contract
        let _address = self.deploy_contract(&tx.sender, code.to_vec())?;
        
        Ok(())
    }
    
    /// Process a contract call transaction
    fn process_call_contract(
        &self,
        tx: &Transaction,
        contract: &str,
        function: &str,
        args: &[u8],
    ) -> Result<(), BlockchainError> {
        // Caller must have sufficient balance for fees
        let available = self.utxo_set.read().get_balance(&tx.sender);
        if available < tx.fee {
            return Err(BlockchainError::InsufficientBalance {
                required: tx.fee,
                available,
            });
        }
        
        // Execute the contract
        let _result = self.call_contract(&tx.sender, contract, function, args.to_vec())?;
        
        Ok(())
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
    // Tests need to be updated to work with new storage-based initialization
    // TODO: Add proper integration tests
}
