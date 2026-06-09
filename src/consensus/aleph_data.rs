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
    /// Shared atomic: timestamp (Unix seconds) of the last finalized block.
    /// Written by the finalization consumer; read here without a lock.
    last_finalized_ts: Arc<AtomicI64>,
    /// Local tracking: timestamp of the last block we proposed.
    /// Prevents the node from spamming proposals while waiting for
    /// AlephBFT to finalize the block we just proposed.
    last_proposed_ts: i64,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        proposer_address: String,
        last_finalized_ts: Arc<AtomicI64>,
    ) -> Self {
        Self {
            blockchain,
            proposer_address,
            last_finalized_ts,
            last_proposed_ts: 0,
        }
    }
}

#[async_trait]
impl DataProvider for QuantaDataProvider {
    type Output = Block;

    async fn get_data(&mut self) -> Option<Self::Output> {
        // ---------------------------------------------------------------------------
        // HOT PATH: check the 6-second slot gate WITHOUT acquiring any lock.
        //
        // `last_finalized_ts` is an `Arc<AtomicI64>` updated by the finalization
        // consumer each time a new block is applied.  A single atomic load is
        // orders of magnitude cheaper than acquiring a tokio RwLock, which matters
        // because AlephBFT calls get_data() at sub-millisecond intervals.
        // ---------------------------------------------------------------------------
        // 6-SECOND SLOT GATE
        // Block production is rate-limited to once per SLOT_SECONDS (6 s).
        // Await until the window has elapsed — returning None would tell
        // AlephBFT that the data provider is permanently closed.
        loop {
            let last_ts = self.last_finalized_ts.load(Ordering::Acquire);
            let current_time = chrono::Utc::now().timestamp();
            
            // We must wait at least 6s from the last finalized block AND
            // 6s from our OWN last proposal (to prevent spamming blocks
            // during the brief window before consensus finalizes our block).
            let target_ts = std::cmp::max(last_ts + 6, self.last_proposed_ts + 6);
            
            if current_time >= target_ts {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // ---------------------------------------------------------------------------
        // SLOW PATH: we're past the 6-second window, so build a block template.
        // Only NOW do we acquire the blockchain read-lock.
        // ---------------------------------------------------------------------------
        let bc = self.blockchain.read().await;
        match bc.create_block_template(self.proposer_address.clone()) {
            Ok(block) => {
                self.last_proposed_ts = chrono::Utc::now().timestamp();
                Some(block)
            },
            Err(e) => {
                tracing::error!("Failed to create block template: {:?}", e);
                // In BFT, if we can't create a block, we shouldn't kill the node
                // by returning None. However, this is an edge case.
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
