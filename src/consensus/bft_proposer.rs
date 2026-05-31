/// Quanta v2 — BFT Block Proposer Loop
///
/// Each validator node runs this task in the background.
///
/// # Protocol (per slot)
///
///   1. Determine if we are the proposer for this block height.
///   2. Build a block template from the mempool.
///   3. Broadcast `BftProposal` to all peers.
///   4. Wait up to 2/3 of the slot window for `BftPrecommit` messages.
///   5. Once ≥ ⌈2/3⌉ committee members have signed, assemble the BFT
///      certificate and call `Blockchain::add_network_block()`.
///   6. Broadcast the certified block to all peers.
///
/// Non-proposer validators verify incoming proposals and broadcast their
/// own precommit via the `handle_bft_proposal()` function.

use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use tracing::{info, warn, error};
use chrono::Utc;

use crate::consensus::authorities::{
    compute_committee, epoch_for_height, epoch_start, get_proposer, EPOCH_SIZE, MAX_COMMITTEE_SIZE,
};
use crate::consensus::bft::{verify_bft_certificate, BftVoteCollector, sign_bft_vote};
use crate::consensus::Blockchain;
use crate::core::block::Block;
use crate::crypto::wallet::QuantumWallet;
use crate::network::Network;

/// Duration of a single BFT slot (= one block target time).
pub const SLOT_SECONDS: u64 = 6;

/// A BFT precommit vote broadcast over the P2P network.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct VoteMsg {
    /// Block height this vote is for.
    pub height: u64,
    /// BFT round (Tendermint).
    pub round: u32,
    /// Epoch.
    pub epoch: u64,
    /// Hash of the block being voted on.
    pub block_hash: String,
    /// Address of the signing validator.
    pub validator: String,
    /// Falcon-512 signature over bft_signing_payload().
    pub signature: Vec<u8>,
}

/// Wire-level BFT protocol message (serialised with bincode into P2PMessage::BftMessage).
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub enum BftProtocolMsg {
    /// A validator broadcasts its precommit vote.
    Vote(VoteMsg),
    /// The proposer broadcasts its proposed block (unsigned certificate).
    Proposal(Block),
}

/// How long the proposer waits for votes before giving up (2/3 of slot time).
const VOTE_COLLECTION_MS: u64 = (SLOT_SECONDS * 1000 * 2) / 3;

// ---------------------------------------------------------------------------
// Non-proposer: handle an incoming block proposal
// ---------------------------------------------------------------------------

/// Called by a validator node when it receives a `BftProposal` from the
/// current proposer.
///
/// Validates the proposal and — if valid — broadcasts a `BftPrecommit`
/// signed with the local wallet key.
///
/// Returns `Some(sig)` if we signed and it should be broadcast, `None`
/// if the proposal was rejected.
pub async fn handle_bft_proposal(
    block: &Block,
    blockchain: &Arc<RwLock<Blockchain>>,
    wallet: &QuantumWallet,
) -> Option<Vec<u8>> {
    let bc = blockchain.read().await;

    // Basic structural validation.
    let prev = bc.get_block_by_index(block.index.saturating_sub(1));
    if !block.is_valid(prev.as_ref()) {
        warn!("BFT: received invalid proposal for height {}", block.index);
        return None;
    }

    // Committee check.
    let state = bc.get_account_state_snapshot();
    let committee = compute_committee(&state);
    if !committee.contains(&block.proposer) {
        warn!("BFT: proposer {} not in committee", block.proposer);
        return None;
    }

    // Sign the BFT payload.
    let payload = block.bft_signing_payload();
    let sig = sign_bft_vote(&wallet.keypair, &payload);

    info!(
        "BFT: signed precommit for height {} (proposer={})",
        block.index, block.proposer
    );
    Some(sig)
}

// ---------------------------------------------------------------------------
// Proposer loop
// ---------------------------------------------------------------------------

/// Main BFT proposer loop.  Spawn this with `tokio::spawn` for every
/// validator node.  Observers (non-validators) do NOT run this.
///
/// # Parameters
/// - `blockchain`   — shared chain state
/// - `wallet`       — this validator's signing key
/// - `network`      — P2P handle for broadcasting proposals and blocks
/// - `new_block_rx` — notified whenever a new block is added to the chain
pub async fn run_bft_proposer(
    blockchain: Arc<RwLock<Blockchain>>,
    wallet: Arc<QuantumWallet>,
    network: Option<Arc<Network>>,
    mut new_block_rx: watch::Receiver<u64>,
) {
    info!("BFT Proposer: starting (validator={})", wallet.address);

    loop {
        // ── Get current chain state ─────────────────────────────────────────
        let (height, committee, state_snapshot) = {
            let bc = blockchain.read().await;
            let h = bc.get_height();
            let snap = bc.get_account_state_snapshot();
            let c = compute_committee(&snap);
            (h, c, snap)
        };

        let next_height = height;
        let epoch = epoch_for_height(next_height);

        // ── Are we the proposer for next_height? ────────────────────────────
        let my_address = wallet.address.clone();
        let proposer = get_proposer(epoch, next_height, &committee);

        let am_proposer = proposer.as_deref() == Some(my_address.as_str());

        if committee.is_empty() {
            warn!("BFT Proposer: no active validators — waiting...");
            tokio::time::sleep(tokio::time::Duration::from_secs(SLOT_SECONDS)).await;
            continue;
        }

        if !am_proposer {
            // Not our slot — wait for a new block or timeout.
            tokio::select! {
                _ = new_block_rx.changed() => {}
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(SLOT_SECONDS)) => {}
            }
            continue;
        }

        info!(
            "BFT Proposer: I am proposer for height {} (epoch={}, committee_size={})",
            next_height, epoch, committee.len()
        );

        // ── Build block template ────────────────────────────────────────────
        let mut block = match blockchain.read().await
            .create_bft_block_template(next_height, my_address.clone(), epoch, 0)
        {
            Ok(b) => b,
            Err(e) => {
                error!("BFT Proposer: failed to build block template: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Self-sign (proposer also votes).
        let payload = block.bft_signing_payload();
        let my_sig = sign_bft_vote(&wallet.keypair, &payload);

        // ── Broadcast proposal ──────────────────────────────────────────────
        if let Some(ref net) = network {
            net.broadcast_bft_proposal(block.clone()).await;
        }

        // ── Collect votes (precommits) ──────────────────────────────────────
        let mut collector = BftVoteCollector::new(
            next_height,
            0, // round 0
            epoch,
            committee.len(),
        );
        // Include our own vote.
        collector.add_precommit(my_address.clone(), my_sig);

        let deadline = tokio::time::Instant::now()
            + tokio::time::Duration::from_millis(VOTE_COLLECTION_MS);

        while tokio::time::Instant::now() < deadline && !collector.has_precommit_quorum() {
            if let Some(ref net) = network {
                let votes = net.drain_vote_messages(&block.hash).await;
                for vote in votes {
                    // Only accept votes from committee members with valid sigs.
                    if !committee.contains(&vote.validator) {
                        continue;
                    }
                    let pk_opt = state_snapshot
                        .get_validator_info(&vote.validator)
                        .map(|v| v.falcon_pk.clone());

                    if let Some(pk) = pk_opt {
                        let vote_payload = block.bft_signing_payload();
                        if crate::crypto::verify_signature_strict(&vote_payload, &vote.signature, &pk) {
                            collector.add_precommit(vote.validator, vote.signature);
                        }
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // ── Assemble certificate or bail ────────────────────────────────────
        if !collector.has_precommit_quorum() {
            warn!(
                "BFT Proposer: height {} — timed out without quorum, skipping",
                next_height
            );
            continue;
        }

        let (sigs, signers) = collector.extract_certificate();
        block.bft_signatures = sigs;
        block.bft_signers = signers;
        block.finalize_hash(); // rehash after adding certificate

        // ── Submit ──────────────────────────────────────────────────────────
        match blockchain.write().await.add_network_block(block.clone()) {
            Ok(_) => {
                info!(
                    "✓ BFT block {} finalised ({} sigs)",
                    block.index, block.bft_signatures.len()
                );
                if let Some(ref net) = network {
                    net.broadcast_block(block).await;
                }
            }
            Err(e) => {
                error!("BFT Proposer: failed to add block {}: {}", block.index, e);
            }
        }

        // Yield briefly so the chain state can update before the next slot.
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
