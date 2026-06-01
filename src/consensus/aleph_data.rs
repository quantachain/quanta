use std::sync::Arc;
use tokio::sync::RwLock;
use async_trait::async_trait;
use aleph_bft::DataProvider;

use crate::core::block::Block;

use crate::consensus::blockchain::Blockchain;

pub struct QuantaDataProvider {
    blockchain: Arc<RwLock<Blockchain>>,
    proposer_address: String,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        proposer_address: String,
    ) -> Self {
        Self {
            blockchain,
            proposer_address,
        }
    }
}

#[async_trait]
impl DataProvider for QuantaDataProvider {
    type Output = Block;

    async fn get_data(&mut self) -> Option<Self::Output> {
        let bc = self.blockchain.read().await;

        // Ensure a consistent 6-second block time without stalling the BFT DAG.
        // If we sleep here, we stall AlephBFT's internal consensus loops for all nodes!
        // Instead, if 6 seconds haven't passed, we return None immediately so AlephBFT 
        // can create an empty heartbeat unit and maintain the DAG speed.
        let last_block = bc.get_latest_block();
        let current_time = chrono::Utc::now().timestamp();

        if current_time < last_block.timestamp + 6 {
            return None;
        }

        // Quanta's blockchain handles creating the block template with the right height/hashes
        // and automatically pulls from its internal pending transactions.
        match bc.create_block_template(self.proposer_address.clone()) {
            Ok(block) => Some(block),
            Err(e) => {
                tracing::error!("Failed to create block template: {:?}", e);
                None
            }
        }
    }
}

pub struct QuantaFinalizationHandler {
    finalized_tx: tokio::sync::mpsc::UnboundedSender<Block>,
}

impl QuantaFinalizationHandler {
    pub fn new(finalized_tx: tokio::sync::mpsc::UnboundedSender<Block>) -> Self {
        Self { finalized_tx }
    }
}

impl aleph_bft::FinalizationHandler<Block> for QuantaFinalizationHandler {
    fn data_finalized(&mut self, data: Block) {
        // We send the block over a channel to a background async task that will
        // acquire the RwLock on Blockchain and append it.
        if let Err(e) = self.finalized_tx.send(data) {
            tracing::error!("Failed to send finalized block to processing task: {}", e);
        }
    }
}
