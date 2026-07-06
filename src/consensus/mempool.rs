use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

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
    pub fn update_from_blockchain(
        &mut self,
        chain_height: u64,
        mempool_size: usize,
        last_block_time: Option<i64>,
    ) {
        self.chain_height = chain_height;
        self.mempool_size = mempool_size;
        self.last_block_time = last_block_time;
    }
}

/// Thread-safe metrics wrapper
pub struct MetricsCollector {
    metrics: Arc<RwLock<NodeMetrics>>,
    start_time: std::time::Instant,
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
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

    pub async fn update_blockchain_stats(
        &self,
        height: u64,
        mempool_size: usize,
        last_block_time: Option<i64>,
    ) {
        self.metrics
            .write()
            .await
            .update_from_blockchain(height, mempool_size, last_block_time);
    }
}
