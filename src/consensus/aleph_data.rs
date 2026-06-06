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

        // 6-SECOND RATE-LIMIT GATE
        // AlephBFT calls get_data() as fast as its DAG allows (sub-second).
        // We return None until 6 wall-clock seconds have passed since the last
        // finalized block, so blocks are produced at ~6s intervals.
        //
        // NOTE: We no longer need to clamp last_block.timestamp here.
        // create_block_template now hard-caps block timestamps to wall-clock time,
        // so last_block.timestamp can never be ahead of real time. The old clamp
        // was a band-aid for the timestamp drift bug — the real fix is upstream.
        let last_block = bc.get_latest_block();
        let current_time = chrono::Utc::now().timestamp();

        if current_time < last_block.timestamp + 6 {
            return None;
        }

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
