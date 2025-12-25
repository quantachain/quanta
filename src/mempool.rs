use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Enhanced mempool for managing pending transactions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mempool {
    // Transactions indexed by hash
    transactions: HashMap<String, Transaction>,
    // Transactions sorted by timestamp (simple priority)
    by_timestamp: BTreeMap<i64, Vec<String>>,
    // Max size limit
    max_size: usize,
}

impl Mempool {
    /// Create a new mempool
    pub fn new(max_size: usize) -> Self {
        Self {
            transactions: HashMap::new(),
            by_timestamp: BTreeMap::new(),
            max_size,
        }
    }

    /// Add a transaction to the mempool
    pub fn add(&mut self, tx: Transaction) -> Result<(), String> {
        if self.transactions.len() >= self.max_size {
            return Err("Mempool is full".to_string());
        }

        let tx_hash = self.calculate_tx_hash(&tx);
        
        // Check if already exists
        if self.transactions.contains_key(&tx_hash) {
            return Err("Transaction already in mempool".to_string());
        }

        // Add to timestamp index
        let timestamp = chrono::Utc::now().timestamp();
        self.by_timestamp
            .entry(timestamp)
            .or_insert_with(Vec::new)
            .push(tx_hash.clone());

        // Add to main storage
        self.transactions.insert(tx_hash, tx);

        Ok(())
    }

    /// Remove a transaction from mempool
    pub fn remove(&mut self, tx_hash: &str) {
        if let Some(tx) = self.transactions.remove(tx_hash) {
            // Remove from timestamp index
            for (_, hashes) in self.by_timestamp.iter_mut() {
                hashes.retain(|h| h != tx_hash);
            }
            // Clean up empty timestamp entries
            self.by_timestamp.retain(|_, v| !v.is_empty());
        }
    }

    /// Get best transactions for mining (ordered by timestamp, oldest first)
    pub fn get_best_transactions(&self, max_count: usize) -> Vec<Transaction> {
        let mut result = Vec::new();
        
        for (_, hashes) in self.by_timestamp.iter() {
            for hash in hashes {
                if let Some(tx) = self.transactions.get(hash) {
                    result.push(tx.clone());
                    if result.len() >= max_count {
                        return result;
                    }
                }
            }
        }
        
        result
    }

    /// Get all transactions
    pub fn get_all(&self) -> Vec<Transaction> {
        self.transactions.values().cloned().collect()
    }

    /// Remove transactions that are in a mined block
    pub fn remove_mined(&mut self, block_txs: &[Transaction]) {
        for tx in block_txs {
            let tx_hash = self.calculate_tx_hash(tx);
            self.remove(&tx_hash);
        }
    }

    /// Get transaction count
    pub fn len(&self) -> usize {
        self.transactions.len()
    }

    /// Check if mempool is empty
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Clear all transactions
    pub fn clear(&mut self) {
        self.transactions.clear();
        self.by_timestamp.clear();
    }

    /// Calculate transaction hash (simple implementation)
    fn calculate_tx_hash(&self, tx: &Transaction) -> String {
        use sha3::{Digest, Sha3_256};
        let data = format!("{}{}{}", tx.sender, tx.recipient, tx.amount);
        let hash = Sha3_256::digest(data.as_bytes());
        hex::encode(hash)
    }

    /// Check if transaction exists
    pub fn contains(&self, tx_hash: &str) -> bool {
        self.transactions.contains_key(tx_hash)
    }
}

/// Node metrics for monitoring
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct NodeMetrics {
    pub connected_peers: usize,
    pub blocks_mined: u64,
    pub blocks_received: u64,
    pub blocks_sent: u64,
    pub transactions_received: u64,
    pub transactions_sent: u64,
    pub mempool_size: usize,
    pub chain_height: u64,
    pub node_uptime_secs: u64,
    pub last_block_time: Option<i64>,
    pub average_block_time: f64,
}

impl NodeMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update metrics from blockchain state
    pub fn update_from_blockchain(&mut self, chain_height: u64, mempool_size: usize, last_block_time: Option<i64>) {
        self.chain_height = chain_height;
        self.mempool_size = mempool_size;
        self.last_block_time = last_block_time;
    }

    /// Increment blocks mined
    pub fn increment_blocks_mined(&mut self) {
        self.blocks_mined += 1;
    }

    /// Increment blocks received
    pub fn increment_blocks_received(&mut self) {
        self.blocks_received += 1;
    }

    /// Increment transactions received
    pub fn increment_transactions_received(&mut self) {
        self.transactions_received += 1;
    }
}

/// Thread-safe metrics wrapper
pub struct MetricsCollector {
    metrics: Arc<RwLock<NodeMetrics>>,
    start_time: std::time::Instant,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(NodeMetrics::new())),
            start_time: std::time::Instant::now(),
        }
    }

    pub async fn get_metrics(&self) -> NodeMetrics {
        let mut metrics = self.metrics.read().await.clone();
        metrics.node_uptime_secs = self.start_time.elapsed().as_secs();
        metrics
    }

    pub async fn update_peer_count(&self, count: usize) {
        self.metrics.write().await.connected_peers = count;
    }

    pub async fn increment_blocks_mined(&self) {
        self.metrics.write().await.increment_blocks_mined();
    }

    pub async fn increment_blocks_received(&self) {
        self.metrics.write().await.increment_blocks_received();
    }

    pub async fn increment_transactions_received(&self) {
        self.metrics.write().await.increment_transactions_received();
    }

    pub async fn update_blockchain_stats(&self, height: u64, mempool_size: usize, last_block_time: Option<i64>) {
        self.metrics.write().await.update_from_blockchain(height, mempool_size, last_block_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::FalconKeypair;

    #[test]
    fn test_mempool_operations() {
        let mut mempool = Mempool::new(100);
        
        let keypair = FalconKeypair::generate();
        let tx = Transaction {
            sender: "alice".to_string(),
            recipient: "bob".to_string(),
            amount: 10.0,
            signature: vec![],
            public_key: keypair.public_key,
        };
        
        // Add transaction
        assert!(mempool.add(tx.clone()).is_ok());
        assert_eq!(mempool.len(), 1);
        
        // Try to add duplicate
        assert!(mempool.add(tx.clone()).is_err());
        
        // Get transactions
        let txs = mempool.get_best_transactions(10);
        assert_eq!(txs.len(), 1);
        
        // Remove transaction
        let tx_hash = mempool.calculate_tx_hash(&tx);
        mempool.remove(&tx_hash);
        assert_eq!(mempool.len(), 0);
    }
}
