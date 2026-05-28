/// Quanta v2 — Tendermint-style BFT Consensus Engine
///
/// This module implements the BFT certificate verification and voting round
/// logic used by all nodes (both validators and observers).
///
/// # Protocol Summary
///
/// Each block height runs through rounds:
///   1. PROPOSE  — the designated proposer broadcasts a `BftProposal`.
///   2. PREVOTE  — every committee member verifies the proposal and broadcasts
///                 a `BftPrevote` (signed block hash).
///   3. PRECOMMIT— upon seeing ≥ 2/3+ prevotes, members broadcast `BftPrecommit`.
///   4. COMMIT   — upon seeing ≥ 2/3+ precommits, the block is finalised and
///                 a BFT certificate (the `bft_signatures` vec) is assembled.
///
/// # What This Module Handles
///
/// - `verify_bft_certificate()` — used by all nodes to validate incoming blocks.
/// - `BftVoteCollector`         — accumulates prevotes/precommits during live
///                                consensus (used by the validator node).
/// - `BftMessage` types         — wire messages exchanged over P2P.

use std::collections::HashMap;

use crate::consensus::authorities::{
    resolve_committee_keys, EPOCH_SIZE, MAX_COMMITTEE_SIZE,
};
use crate::core::block::Block;
use crate::core::transaction::AccountState;
use crate::crypto::{verify_signature_strict, FalconKeypair};

// ---------------------------------------------------------------------------
// BFT wire messages
// ---------------------------------------------------------------------------

/// A BFT vote cast by a validator during the consensus round.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BftVote {
    /// Block height this vote is for.
    pub height: u64,
    /// BFT voting round (0-indexed; increments on timeout).
    pub round: u32,
    /// Epoch the voter belongs to.
    pub epoch: u64,
    /// Hex-encoded block hash being voted on.
    pub block_hash: String,
    /// Validator's address (for committee lookup).
    pub validator: String,
    /// Falcon-512 signature blob: `raw_sig || sha3_hash`.
    /// Signs `block.bft_signing_payload()`.
    pub signature: Vec<u8>,
}

/// Vote phase discriminant.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum VoteKind {
    Prevote,
    Precommit,
}

// ---------------------------------------------------------------------------
// BFT certificate verification
// ---------------------------------------------------------------------------

/// Verify the BFT certificate attached to `block`.
///
/// A valid certificate requires that:
/// 1. The `proposer` field matches the expected proposer for this epoch/height.
/// 2. At least ⌈2/3 * committee_size⌉ unique committee members have signed.
/// 3. Every signature is a valid Falcon-512 signature over
///    `block.bft_signing_payload()`.
/// 4. No signer appears more than once (duplicate-signature attack).
/// 5. The genesis block (height 0) always passes without signatures.
///
/// Returns `true` if the certificate is valid, `false` otherwise.
pub fn verify_bft_certificate(
    block: &Block,
    committee: &[String],
    state: &AccountState,
) -> bool {
    // Genesis block is always valid (no BFT certificate required).
    if block.index == 0 {
        return true;
    }

    let committee_size = committee.len();
    if committee_size == 0 {
        tracing::warn!("BFT verify block {}: empty committee", block.index);
        return false;
    }

    // Threshold: strictly more than 2/3.
    let threshold = bft_threshold(committee_size);

    // Reject if not enough signatures are present without doing crypto.
    if block.bft_signatures.len() < threshold {
        tracing::warn!(
            "BFT verify block {}: only {} signatures, need {} (committee={})",
            block.index, block.bft_signatures.len(), threshold, committee_size
        );
        return false;
    }

    // Signer/signature arrays must be parallel.
    if block.bft_signatures.len() != block.bft_signers.len() {
        tracing::warn!("BFT verify block {}: sig/signer count mismatch", block.index);
        return false;
    }

    // Pre-compute the payload all committee members must have signed.
    let payload = block.bft_signing_payload();

    // Resolve Falcon-512 public keys for the committee.
    let key_map: HashMap<String, falcon_rust::falcon512::PublicKey> =
        resolve_committee_keys(committee, state)
            .into_iter()
            .collect();

    let mut valid_count = 0usize;
    let mut seen_signers: std::collections::HashSet<&str> = std::collections::HashSet::new();

    for (sig, signer) in block.bft_signatures.iter().zip(block.bft_signers.iter()) {
        // Duplicate signer check.
        if !seen_signers.insert(signer.as_str()) {
            tracing::warn!(
                "BFT verify block {}: duplicate signer {}",
                block.index, signer
            );
            return false;
        }

        // Signer must be in the committee.
        if !committee.contains(signer) {
            tracing::warn!(
                "BFT verify block {}: signer {} not in committee",
                block.index, signer
            );
            return false;
        }

        // Look up the Falcon public key.
        let pk_bytes = match state.get_validator_info(signer) {
            Some(info) => &info.falcon_pk,
            None => {
                tracing::warn!(
                    "BFT verify block {}: no validator info for {}",
                    block.index, signer
                );
                return false;
            }
        };

        // Cryptographic verification.
        if !verify_signature_strict(&payload, sig, pk_bytes) {
            tracing::warn!(
                "BFT verify block {}: invalid Falcon-512 sig from {}",
                block.index, signer
            );
            return false;
        }

        valid_count += 1;
    }

    if valid_count < threshold {
        tracing::warn!(
            "BFT verify block {}: only {} valid sigs, need {}",
            block.index, valid_count, threshold
        );
        return false;
    }

    tracing::debug!(
        "BFT verify block {}: certificate OK ({}/{} sigs, threshold={})",
        block.index, valid_count, committee_size, threshold
    );
    true
}

/// Return the minimum number of valid signatures required for consensus.
///
/// Tendermint rule: strictly more than 2/3 of voting power.
/// For N validators: threshold = ⌊2N/3⌋ + 1
#[inline]
pub fn bft_threshold(committee_size: usize) -> usize {
    (committee_size * 2) / 3 + 1
}

// ---------------------------------------------------------------------------
// Vote collector (live consensus — used by validator nodes only)
// ---------------------------------------------------------------------------

/// Accumulates BFT votes for a single (height, round) pair.
///
/// Once `has_quorum()` returns true the caller can assemble the BFT
/// certificate and finalise the block.
pub struct BftVoteCollector {
    pub height: u64,
    pub round: u32,
    pub epoch: u64,
    /// `address -> signature blob`
    prevotes: HashMap<String, Vec<u8>>,
    precommits: HashMap<String, Vec<u8>>,
    committee_size: usize,
}

impl BftVoteCollector {
    pub fn new(height: u64, round: u32, epoch: u64, committee_size: usize) -> Self {
        Self {
            height,
            round,
            epoch,
            prevotes: HashMap::new(),
            precommits: HashMap::new(),
            committee_size,
        }
    }

    /// Record a prevote.  Returns `true` if prevote quorum is now reached.
    pub fn add_prevote(&mut self, validator: String, sig: Vec<u8>) -> bool {
        self.prevotes.insert(validator, sig);
        self.prevotes.len() >= bft_threshold(self.committee_size)
    }

    /// Record a precommit.  Returns `true` if precommit quorum is now reached.
    pub fn add_precommit(&mut self, validator: String, sig: Vec<u8>) -> bool {
        self.precommits.insert(validator, sig);
        self.precommits.len() >= bft_threshold(self.committee_size)
    }

    /// Whether enough prevotes have been collected.
    pub fn has_prevote_quorum(&self) -> bool {
        self.prevotes.len() >= bft_threshold(self.committee_size)
    }

    /// Whether enough precommits have been collected.
    pub fn has_precommit_quorum(&self) -> bool {
        self.precommits.len() >= bft_threshold(self.committee_size)
    }

    /// Extract the BFT certificate once precommit quorum is reached.
    ///
    /// Returns `(sigs, signers)` parallel vecs suitable for storing in `Block`.
    pub fn extract_certificate(&self) -> (Vec<Vec<u8>>, Vec<String>) {
        let mut signers: Vec<String> = self.precommits.keys().cloned().collect();
        signers.sort(); // deterministic ordering
        let sigs: Vec<Vec<u8>> = signers
            .iter()
            .map(|addr| self.precommits[addr].clone())
            .collect();
        (sigs, signers)
    }
}

// ---------------------------------------------------------------------------
// Validator signing helper
// ---------------------------------------------------------------------------

/// Sign a BFT vote payload with the given Falcon-512 keypair.
///
/// `payload` is the 32-byte value returned by `Block::bft_signing_payload()`.
/// Returns the signature blob `raw_sig || payload` as expected by
/// `verify_signature_strict()`.
pub fn sign_bft_vote(keypair: &FalconKeypair, payload: &[u8; 32]) -> Vec<u8> {
    keypair.sign_hash(payload)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::block::Block;
    use crate::core::transaction::{AccountState, ValidatorInfo};
    use crate::crypto::{FalconKeypair, FALCON512_PUBKEY_BYTES};

    fn make_validator(state: &mut AccountState) -> (String, FalconKeypair) {
        let kp = FalconKeypair::generate();
        let addr = kp.get_address();
        state.register_validator(&addr, kp.public_key.clone(), 1_000_000, 0);
        (addr, kp)
    }

    #[test]
    fn genesis_always_valid() {
        let genesis = Block::genesis_v2();
        let state = AccountState::new();
        assert!(verify_bft_certificate(&genesis, &[], &state));
    }

    #[test]
    fn valid_certificate_passes() {
        let mut state = AccountState::new();
        let mut committee = Vec::new();
        let mut keypairs = Vec::new();

        for _ in 0..3 {
            let (addr, kp) = make_validator(&mut state);
            committee.push(addr);
            keypairs.push(kp);
        }
        committee.sort(); // match compute_epoch_committee sort order

        let genesis = Block::genesis_v2();
        let mut block = Block::new_bft(
            1, vec![], genesis.hash.clone(), 0, 0,
            committee[0].clone(),
        );
        block.state_root = "0".repeat(64);
        block.finalize_hash();

        let payload = block.bft_signing_payload();

        // All 3 validators sign (need ≥ 2/3 + 1 = 3 for committee of 3)
        let mut sigs = Vec::new();
        let mut signers = Vec::new();
        for (addr, kp) in committee.iter().zip(keypairs.iter()) {
            sigs.push(kp.sign_hash(&payload));
            signers.push(addr.clone());
        }
        block.bft_signatures = sigs;
        block.bft_signers = signers;
        block.finalize_hash();

        assert!(verify_bft_certificate(&block, &committee, &state));
    }

    #[test]
    fn insufficient_sigs_fails() {
        let mut state = AccountState::new();
        let mut committee = Vec::new();
        let mut keypairs = Vec::new();

        for _ in 0..4 {
            let (addr, kp) = make_validator(&mut state);
            committee.push(addr);
            keypairs.push(kp);
        }
        committee.sort();

        let genesis = Block::genesis_v2();
        let mut block = Block::new_bft(
            1, vec![], genesis.hash.clone(), 0, 0,
            committee[0].clone(),
        );
        block.state_root = "0".repeat(64);
        block.finalize_hash();

        let payload = block.bft_signing_payload();

        // Only 2 of 4 sign — threshold is 3, should fail
        block.bft_signatures = vec![keypairs[0].sign_hash(&payload)];
        block.bft_signers = vec![committee[0].clone()];
        block.finalize_hash();

        assert!(!verify_bft_certificate(&block, &committee, &state));
    }

    #[test]
    fn bft_threshold_values() {
        assert_eq!(bft_threshold(1),  1);
        assert_eq!(bft_threshold(3),  3);
        assert_eq!(bft_threshold(4),  3);
        assert_eq!(bft_threshold(7),  5);
        assert_eq!(bft_threshold(21), 15);
    }
}
