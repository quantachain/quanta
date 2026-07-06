use aleph_bft::DataProvider;
use async_trait::async_trait;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::consensus::blockchain::Blockchain;
use crate::core::block::Block;

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
    /// Wallet used to sign the proposed block
    wallet: Arc<crate::crypto::wallet::QuantumWallet>,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        my_address: String,
        last_finalized_ts: Arc<AtomicI64>,
        wallet: Arc<crate::crypto::wallet::QuantumWallet>,
    ) -> Self {
        Self {
            blockchain,
            my_address,
            last_finalized_ts,
            wallet,
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
            let last_ts = self.last_finalized_ts.load(Ordering::Acquire);
            let elapsed = now_unix.saturating_sub(last_ts);

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
        let elapsed = chrono::Utc::now()
            .timestamp()
            .saturating_sub(self.last_finalized_ts.load(Ordering::Acquire));
        match bc.create_block_template(self.my_address.clone()) {
            Ok(mut block) => {
                // SECURITY FIX: Sign the block so network syncing nodes can cryptographically verify it.
                // Since AlephBFT does not export standard BFT certificates, the Proposer's signature
                // acts as a verifiable proof of origin during P2P sync.
                let payload = block.bft_signing_payload();
                let sig = self.wallet.keypair.sign_hash(&payload);
                block.bft_signatures.push(sig);
                block.bft_signers.push(self.my_address.clone());
                block.finalize_hash(); // Re-finalize since we mutated it

                tracing::info!(
                    "BFT DataProvider: proposing signed block {} (proposer: {}, elapsed since last finalized: {}s)",
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
