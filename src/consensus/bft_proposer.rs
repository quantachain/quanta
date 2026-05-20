/// Quanta 2.0 BFT Block Proposer
///
/// Replaces PoW mining after `QUANTA_V2_FORK_HEIGHT`.  Each validator node
/// runs this loop:
///
///   1. Wait until our slot starts (slot = (now - genesis_ts) / SLOT_SECONDS).
///   2. Propose a block by building a template and broadcasting it.
///   3. Collect `VoteMsg` from ≥ 2f+1 validators (each a Falcon-512 sig over the
///      block hash).
///   4. Assemble the `bft_signatures` vec and call `Blockchain::add_bft_block`.
///   5. Broadcast the finalised block to the network.
///
/// The design intentionally keeps AlephBFT as an optional async layer that can
/// be added later; for the initial production release the simpler single-leader
/// per-slot model below is sufficient for the small validator set.
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use sha3::{Digest, Sha3_256};
use chrono::Utc;
use tracing::{info, warn, error};

use crate::consensus::blockchain::{Blockchain, BlockchainError, QUANTA_V2_FORK_HEIGHT};
use crate::crypto::wallet::QuantumWallet;
use crate::network::Network;

/// Duration of a single BFT slot in seconds.
/// Validators propose one block per slot; unused slots are skipped.
pub const SLOT_SECONDS: u64 = 30;

/// Unix timestamp of block 0 on Testnet (used for slot calculation).
/// Must match Block::genesis(Testnet).timestamp.
const TESTNET_GENESIS_TS: i64 = 1_775_001_600;

/// A vote message from one validator: their Falcon-512 signature over the
/// canonical "QUANTA_BFT_V1:" || chain_id || block_hash bytes.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct VoteMsg {
    pub block_hash: String,
    pub height: u64,
    pub validator_address: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum BftProtocolMsg {
    Vote(VoteMsg),
    Proposal(crate::core::block::Block),
}

/// Return the current BFT slot number relative to genesis.
pub fn current_slot(genesis_ts: i64) -> u64 {
    let elapsed = (Utc::now().timestamp() - genesis_ts).max(0) as u64;
    elapsed / SLOT_SECONDS
}

/// Sign a block hash with the validator's Falcon-512 key, using the same
/// domain prefix as `FalconKeychain::sign` in bft.rs.
pub fn sign_block_hash(wallet: &QuantumWallet, block_hash: &str, chain_id: u32) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    hasher.update(b"QUANTA_BFT_V1:");
    hasher.update(&chain_id.to_le_bytes());
    hasher.update(block_hash.as_bytes());
    let digest = hasher.finalize();
    wallet.keypair.sign_transaction_canonical(&digest)
}

/// Verify a vote signature from a known validator public key.
pub fn verify_vote(
    block_hash: &str,
    chain_id: u32,
    sig_bytes: &[u8],
    pk_bytes: &[u8],
) -> bool {
    let mut hasher = Sha3_256::new();
    hasher.update(b"QUANTA_BFT_V1:");
    hasher.update(&chain_id.to_le_bytes());
    hasher.update(block_hash.as_bytes());
    let digest = hasher.finalize();

    let pk = match falcon_rust::falcon512::PublicKey::from_bytes(pk_bytes) {
        Ok(pk) => pk,
        Err(_) => return false,
    };
    let sig = match falcon_rust::falcon512::Signature::from_bytes(sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    falcon_rust::falcon512::verify(&digest, &sig, &pk)
}

/// The main BFT proposer task.  Spawn this with `tokio::spawn` after the chain
/// reaches `QUANTA_V2_FORK_HEIGHT - 1`.
///
/// # Parameters
/// - `blockchain` — shared blockchain state
/// - `wallet`     — this validator's signing wallet
/// - `network`    — P2P network handle for broadcasting
/// - `chain_id`   — 1 = Mainnet, 0 = Testnet
/// - `new_block_rx` — watch channel from `Blockchain::subscribe_new_blocks()`
pub async fn run_bft_proposer(
    blockchain: Arc<RwLock<Blockchain>>,
    wallet: Arc<QuantumWallet>,
    network: Option<Arc<Network>>,
    chain_id: u32,
    mut new_block_rx: watch::Receiver<u64>,
) {
    info!("BFT Proposer: starting (chain_id={})", chain_id);

    // Derive genesis timestamp from the first block on disk.
    let genesis_ts = {
        let bc = blockchain.read().await;
        let g = bc.get_latest_block(); // returns genesis when height == 1
        // For robustness fall back to the hardcoded testnet constant.
        if g.index == 0 { g.timestamp } else { TESTNET_GENESIS_TS }
    };

    loop {
        // ── Wait for our chain to be at or past the fork height ────────────
        let height = blockchain.read().await.get_height();
        if height < QUANTA_V2_FORK_HEIGHT {
            let blocks_left = QUANTA_V2_FORK_HEIGHT - height;
            info!(
                "BFT Proposer: chain at {}, fork at {} ({} blocks remaining via PoW)",
                height, QUANTA_V2_FORK_HEIGHT, blocks_left
            );
            // Sleep for one slot then re-check.
            tokio::time::sleep(tokio::time::Duration::from_secs(SLOT_SECONDS)).await;
            continue;
        }

        // ── Compute our slot and how long until it starts ──────────────────
        let slot = current_slot(genesis_ts);
        let slot_start_ts = genesis_ts + (slot as i64 * SLOT_SECONDS as i64);
        let now = Utc::now().timestamp();
        let wait_ms = ((slot_start_ts - now).max(0) as u64) * 1000;

        if wait_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms)).await;
        }

        // ── Am I the leader for this slot? ─────────────────────────────────
        // Leader = validator_index % n_validators == slot % n_validators
        // We determine our index from position in the sorted validator map.
        let (am_leader, n_validators) = {
            let bc = blockchain.read().await;
            let validators = bc.get_account_state_read().get_validators().clone();
            let n = validators.len();
            if n == 0 {
                warn!("BFT Proposer: no validators registered yet, skipping slot {}", slot);
                (false, 0usize)
            } else {
                // Sort by address for deterministic ordering across all nodes.
                let mut addrs: Vec<&String> = validators.keys().collect();
                addrs.sort();
                let my_idx = addrs.iter().position(|a| *a == &wallet.address);
                let leader_idx = (slot as usize) % n;
                (my_idx == Some(leader_idx), n)
            }
        };

        if !am_leader {
            // Not our slot — just wait for the next new_block notification
            // or timeout after one slot.
            tokio::select! {
                _ = new_block_rx.changed() => {}
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(SLOT_SECONDS)) => {}
            }
            continue;
        }

        info!("BFT Proposer: I am leader for slot {} (height {})", slot, height);

        // ── Build block template ───────────────────────────────────────────
        let mut block = match blockchain.read().await.create_block_template(wallet.address.clone()) {
            Ok(b) => b,
            Err(e) => {
                error!("BFT Proposer: failed to build block template: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        // Recalculate the hash after template creation (nonce = 0, difficulty = 0 for BFT)
        block.difficulty = 0; // BFT blocks carry no PoW difficulty
        block.hash = block.calculate_hash();

        // ── Self-sign the block (leader's own vote) ─────────────────────────
        let my_sig = sign_block_hash(&wallet, &block.hash, chain_id);
        block.bft_signatures.push(my_sig.clone());

        // ── Broadcast the proposal and collect votes ────────────────────────
        // In the full AlephBFT integration, this would call aleph_bft::run().
        // For the initial production release, we broadcast a "BlockProposal"
        // P2P message and collect VoteMsg replies within a 2/3 of slot_time window.
        //
        // For now we collect votes via the network gossip layer.
        // Votes arrive as `VoteMsg` in the `VoteMessages` P2P channel.

        let threshold = (n_validators * 2) / 3 + 1;
        let collection_deadline = tokio::time::Instant::now()
            + tokio::time::Duration::from_secs(SLOT_SECONDS * 2 / 3);

        // Broadcast our proposal to peers so they can vote.
        if let Some(ref net) = network {
            net.broadcast_bft_proposal(block.clone()).await;
        }

        // Collect remote votes.  The P2P layer delivers them via a channel.
        // We wait until we have a super-majority or the deadline passes.
        // (The actual vote-collection channel will be wired in Network v2.)
        // For now we finalize with just our own signature if no peers are present
        // (single-validator testnet scenario).

        // Poll until deadline.
        while tokio::time::Instant::now() < collection_deadline {
            let current_sigs = block.bft_signatures.len();
            if current_sigs >= threshold {
                break;
            }

            // Try to get votes from the network layer (non-blocking peek)
            if let Some(ref net) = network {
                let votes = net.drain_vote_messages(&block.hash).await;
                for vote in votes {
                    // Verify the vote is from a known validator with valid signature
                    let bc = blockchain.read().await;
                    let validators = bc.get_account_state_read().get_validators().clone();
                    drop(bc);

                    if let Some(pk_bytes) = validators.get(&vote.validator_address) {
                        if verify_vote(&block.hash, chain_id, &vote.signature, pk_bytes) {
                            // De-duplicate by checking we don't already have a sig
                            // from this validator (compare the first 8 bytes as a cheap key)
                            let already_seen = block.bft_signatures.iter().any(|s| {
                                s.len() >= 8 && vote.signature.len() >= 8 && s[..8] == vote.signature[..8]
                            });
                            if !already_seen {
                                block.bft_signatures.push(vote.signature);
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        let collected = block.bft_signatures.len();
        if collected < threshold {
            warn!(
                "BFT Proposer: slot {} timed out with only {}/{} signatures — skipping block",
                slot, collected, threshold
            );
            continue;
        }

        info!(
            "BFT Proposer: slot {} — assembled certificate with {}/{} signatures",
            slot, collected, n_validators
        );

        // ── Submit the finalised BFT block ─────────────────────────────────
        match blockchain.write().await.add_network_block(block.clone()) {
            Ok(_) => {
                info!(
                    "✓ BFT block {} finalised at slot {} ({} validator signatures)",
                    block.index, slot, block.bft_signatures.len()
                );
                // Broadcast the fully-certified block to peers.
                if let Some(ref net) = network {
                    net.broadcast_block(block.clone()).await;
                }
            }
            Err(e) => {
                error!("BFT Proposer: failed to add BFT block {}: {}", block.index, e);
            }
        }

        // Brief pause before next slot calculation
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
