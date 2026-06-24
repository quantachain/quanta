use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::RwLock;
use async_trait::async_trait;
use aleph_bft::DataProvider;

use crate::core::block::Block;
use crate::consensus::blockchain::Blockchain;

/// Target block slot duration in seconds.
const SLOT_SECONDS: i64 = 6;

/// How long a non-designated validator waits before acting as backup proposer.
///
/// When the designated (round-robin) proposer for the current slot does not
/// produce a block within this window, other validators step in to keep the
/// chain moving.  Slightly longer than SLOT_SECONDS so the designated proposer
/// always gets first priority when it is online.
const PROPOSER_TIMEOUT_SECS: i64 = 8;

/// AlephBFT data provider — proposes the next block once per 6-second slot.
///
/// # Block-time fix (2026-06-24)
///
/// Previous implementations gated proposals on `last_real_time_proposal`
/// (when *this node* last proposed).  This was wrong for two reasons:
///
///  1. AlephBFT is leaderless — any of the 7 validators can propose.
///     The slot gate must fire relative to when the last block was
///     **finalized**, not when this node last proposed.  When the
///     `QuantaDataProvider` is recreated each session (every 60 blocks),
///     `last_real_time_proposal` was `None`, causing an immediate proposal
///     flood right after session rotation, spiking block time to 25s.
///
///  2. `last_finalized_ts` was wired up in `bft_proposer.rs` precisely to
///     solve this, but was then accidentally neutered (prefixed with `_`).
///     This restores it as the primary timing control.
///
/// # Round-robin fix (2026-06-24)
///
/// Without explicit turn assignment, validators with lower latency or better
/// connectivity win the AlephBFT DAG race on every block, causing some
/// validators to receive zero transactions.  We now use `get_proposer()` to
/// select a designated proposer for each slot.  Non-designated validators
/// hold back for PROPOSER_TIMEOUT_SECS before acting as backup, giving the
/// designated proposer priority while still keeping the chain alive if it
/// goes offline.
pub struct QuantaDataProvider {
    blockchain: Arc<RwLock<Blockchain>>,
    /// This validator's own address.
    my_address: String,
    /// Sorted committee list — same order used by `get_proposer()`.
    committee: Vec<String>,
    /// Shared atomic written by the finalization consumer in `bft_proposer.rs`
    /// immediately after each block is applied.  `get_data()` reads this
    /// lock-free to implement the SLOT_SECONDS gate.
    last_finalized_ts: Arc<AtomicI64>,
    /// Wall-clock instant at which we first noticed the current slot was
    /// "open" (>= SLOT_SECONDS since last finalization) but we are not
    /// the designated proposer.  Used to measure the failover window.
    slot_overdue_since: Option<tokio::time::Instant>,
}

impl QuantaDataProvider {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        my_address: String,
        committee: Vec<String>,
        last_finalized_ts: Arc<AtomicI64>,
    ) -> Self {
        Self {
            blockchain,
            my_address,
            committee,
            last_finalized_ts,
            slot_overdue_since: None,
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
        // Only propose a block once at least SLOT_SECONDS of real time have
        // elapsed since the last *finalized* block.  This prevents the rapid-
        // fire proposal flood that occurred when QuantaDataProvider was
        // recreated on each session rotation with last_real_time_proposal=None.
        // -----------------------------------------------------------------------
        let now_unix = chrono::Utc::now().timestamp();
        let last_ts  = self.last_finalized_ts.load(Ordering::Acquire);
        let elapsed  = now_unix.saturating_sub(last_ts);

        if elapsed < SLOT_SECONDS {
            // Slot has not opened yet — reset the overdue timer and yield.
            self.slot_overdue_since = None;
            return None;
        }

        // -----------------------------------------------------------------------
        // ROUND-ROBIN PROPOSER SELECTION
        //
        // Compute the designated proposer for the current height:
        //   slot_idx         = current_height % committee.len()
        //   designated       = committee[slot_idx]
        //
        // If we ARE the designated proposer → propose immediately.
        // If we are NOT → start (or continue) the overdue timer.
        //   • Within PROPOSER_TIMEOUT_SECS: return None (let them go first).
        //   • After timeout: step in as backup (designated proposer is offline).
        // -----------------------------------------------------------------------
        if !self.committee.is_empty() {
            // Acquire a short-lived read lock only to get the current height.
            let current_height = {
                let bc = self.blockchain.read().await;
                bc.get_height()
            };

            let slot_idx   = (current_height as usize) % self.committee.len();
            let designated = &self.committee[slot_idx];

            if designated != &self.my_address {
                // We are NOT the designated proposer for this slot.
                let now_instant   = tokio::time::Instant::now();
                let overdue_since = self.slot_overdue_since.get_or_insert(now_instant);
                let waiting_secs  = now_instant
                    .duration_since(*overdue_since)
                    .as_secs() as i64;

                if waiting_secs < PROPOSER_TIMEOUT_SECS {
                    // Still within grace period — yield to the designated proposer.
                    return None;
                }

                // Timeout: designated proposer appears offline — step in as backup.
                tracing::warn!(
                    "BFT slot {}: designated proposer {} did not produce a block within {}s \
                     — stepping in as backup proposer",
                    current_height, designated, PROPOSER_TIMEOUT_SECS
                );
            } else {
                // We ARE the designated proposer — clear any stale overdue timer.
                self.slot_overdue_since = None;
                tracing::debug!(
                    "BFT slot {}: I am the designated proposer ({})",
                    current_height, self.my_address
                );
            }
        }

        // -----------------------------------------------------------------------
        // BUILD BLOCK TEMPLATE
        // -----------------------------------------------------------------------
        let bc = self.blockchain.read().await;
        match bc.create_block_template(self.my_address.clone()) {
            Ok(block) => {
                tracing::info!(
                    "BFT DataProvider: proposing block {} (proposer: {}, elapsed since last: {}s)",
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
