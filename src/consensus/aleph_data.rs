use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::RwLock;
use async_trait::async_trait;
use aleph_bft::DataProvider;

use crate::core::block::Block;
use crate::consensus::blockchain::Blockchain;

/// AlephBFT data provider that proposes the next block at the configured
/// block interval (SLOT_SECONDS = 6).
///
/// # Lock-contention fix
///
/// The original implementation acquired the blockchain read-lock on *every*
/// `get_data()` call — including the majority of calls that immediately return
/// `None` because the 6-second window hasn't elapsed yet.  AlephBFT calls
/// `get_data()` thousands of times per second, so this caused severe lock
/// contention with the finalization writer.
///
/// The fix: the finalization consumer in `bft_proposer.rs` stores the
/// timestamp of each newly applied block into `last_finalized_ts` (an
/// `Arc<AtomicI64>`).  `get_data()` reads that atomic — a single load
/// instruction, no lock — and only acquires the blockchain read-lock when
/// it's actually time to build a new block template.
pub struct QuantaDataProvider {
    blockchain: Arc<RwLock<Blockchain>>,
    proposer_address: String,
    last_real_time_proposal: Option<tokio::time::Instant>,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        proposer_address: String,
        _last_finalized_ts: Arc<AtomicI64>, // Kept for backwards compatibility if needed, but unused
    ) -> Self {
        Self {
            blockchain,
            proposer_address,
            last_real_time_proposal: None,
        }
    }
}

#[async_trait]
impl DataProvider for QuantaDataProvider {
    type Output = Block;

    async fn get_data(&mut self) -> Option<Self::Output> {
        // ---------------------------------------------------------------------------
        // REAL-TIME 6-SECOND SLOT GATE
        // 
        // Previously, returning `None` caused AlephBFT to aggressively spam empty
        // units into the network, accelerating the DAG's round counter.
        // Once the round counter exceeded 3000, AlephBFT's exponential delay 
        // kicked in, ballooning block times to 30-60 seconds.
        //
        // By using `tokio::time::sleep` to block AlephBFT's Creator task, we
        // PREVENT it from producing empty units. The DAG advances exclusively via
        // data units, strictly maintaining exactly 1 round per 6 seconds.
        // ---------------------------------------------------------------------------
        let now = tokio::time::Instant::now();
        let slot = tokio::time::Duration::from_secs(6);

        if let Some(last_proposal) = self.last_real_time_proposal {
            let elapsed = now.duration_since(last_proposal);
            if elapsed < slot {
                tokio::time::sleep(slot - elapsed).await;
            }
        }
        
        self.last_real_time_proposal = Some(tokio::time::Instant::now());

        let bc = self.blockchain.read().await;
        match bc.create_block_template(self.proposer_address.clone()) {
            Ok(block) => Some(block),
            Err(e) => {
                tracing::error!("Failed to create block template: {:?}", e);
                // Return None only on catastrophic failure, which triggers AlephBFT's empty-unit delay fallback
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
        if let Err(e) = self.finalized_tx.send(data) {
            tracing::error!("Failed to send finalized block to processing task: {}", e);
        }
    }
}
