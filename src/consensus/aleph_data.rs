use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::RwLock;
use async_trait::async_trait;
use aleph_bft::DataProvider;

use crate::core::block::Block;
use crate::consensus::blockchain::Blockchain;

/// Target block slot duration in seconds.
const SLOT_SECONDS: i64 = 6;

/// AlephBFT data provider — proposes the next block once per 6-second slot.
///
/// # Block-time design (2026-07-02)
///
/// AlephBFT is leaderless by design — its DAG handles concurrent proposals
/// from multiple validators and picks one natively.  All validators propose
/// simultaneously once SLOT_SECONDS have elapsed since the last finalized
/// block.  There is no round-robin designator or proposer timeout; those
/// added up to 8s of extra latency per block and fought against AlephBFT's
/// own consensus mechanism.
///
/// The slot gate is anchored to `last_finalized_ts` (written atomically by
/// the finalization consumer in `bft_proposer.rs`) rather than to when this
/// node last proposed, so the gate works correctly across session rotations
/// and node restarts.
pub struct QuantaDataProvider {
    blockchain: Arc<RwLock<Blockchain>>,
    /// This validator's own address — used only for block template creation.
    my_address: String,
    /// Shared atomic written by the finalization consumer in `bft_proposer.rs`
    /// immediately after each block is applied.  `get_data()` reads this
    /// lock-free to implement the SLOT_SECONDS gate.
    last_finalized_ts: Arc<AtomicI64>,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        my_address: String,
        last_finalized_ts: Arc<AtomicI64>,
    ) -> Self {
        Self {
            blockchain,
            my_address,
            last_finalized_ts,
        }
    }
}

#[async_trait]
impl DataProvider for QuantaDataProvider {
    type Output = Block;

    async fn get_data(&mut self) -> Option<Self::Output> {
        // -----------------------------------------------------------------------
        // SLOT GATE — primary block-time control.
        //
        // Yield until at least SLOT_SECONDS of real time have elapsed since
        // the last finalized block.  All validators propose simultaneously
        // once the slot opens; AlephBFT's DAG selects one via consensus.
        // -----------------------------------------------------------------------
        loop {
            let now_unix = chrono::Utc::now().timestamp();
            let last_ts  = self.last_finalized_ts.load(Ordering::Acquire);
            let elapsed  = now_unix.saturating_sub(last_ts);

            if elapsed >= SLOT_SECONDS {
                break;
            }
            
            let delay = (SLOT_SECONDS - elapsed) as u64;
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }

        // -----------------------------------------------------------------------
        // BUILD BLOCK TEMPLATE
        // -----------------------------------------------------------------------
        let bc = self.blockchain.read().await;
        match bc.create_block_template(self.my_address.clone()) {
            Ok(block) => {
                tracing::info!(
                    "BFT DataProvider: proposing block {} (proposer: {}, elapsed since last finalized: {}s)",
                    block.index, self.my_address, elapsed
                );
                Some(block)
            }
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
        if let Err(e) = self.finalized_tx.send(data) {
            tracing::error!("Failed to send finalized block to processing task: {}", e);
        }
    }
}
