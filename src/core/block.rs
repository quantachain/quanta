use serde::{Serialize, Deserialize};
use crate::core::transaction::Transaction;
use crate::crypto::{double_sha3, FALCON512_SIG_MAX_BYTES, FALCON512_SIG_MIN_BYTES};
use crate::core::merkle::MerkleTree;
use chrono::Utc;

// ---------------------------------------------------------------------------
// BFT Block — Quanta v2
//
// No proof-of-work. Every block is proposed by a validator in the current
// epoch committee and committed when ≥ ⌈2/3⌉ of the committee has signed.
// ---------------------------------------------------------------------------

/// A Quanta v2 block.
///
/// Consensus is BFT (Tendermint-style) from genesis.
/// All integrity is provided by Falcon-512 signatures from the epoch committee,
/// NOT by hash-puzzle PoW.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, std::hash::Hash, codec::Encode, codec::Decode)]
pub struct Block {
    // ---- Chain structure ----
    /// Block height (0 = genesis).
    pub index: u64,
    /// Unix timestamp (seconds) when block was proposed.
    pub timestamp: i64,
    /// Transactions included in this block.
    pub transactions: Vec<Transaction>,
    /// Hash of the previous block (links the chain).
    pub previous_hash: String,
    /// SHA3-256 double-hash over all block fields.
    pub hash: String,
    /// SHA3-256 Merkle root of `transactions`.
    pub merkle_root: String,
    /// SHA3-256 commitment to the global account state after applying this block.
    pub state_root: String,

    // ---- BFT consensus ----
    /// Epoch number this block belongs to. Epoch N covers heights
    /// [N * EPOCH_SIZE, (N+1) * EPOCH_SIZE).
    pub epoch: u64,
    /// Tendermint voting round in which 2/3+ agreement was reached (0-indexed).
    pub bft_round: u32,
    /// Address of the validator that proposed this block.
    pub proposer: String,
    /// Falcon-512 BFT certificate: one `raw_sig || hash` blob per signing
    /// committee member. Format matches `verify_signature_strict()`.
    /// Must contain ≥ ⌈2/3 * committee_size⌉ valid entries.
    pub bft_signatures: Vec<Vec<u8>>,
    /// Addresses of the validators whose signatures are in `bft_signatures`,
    /// in the same order. Stored explicitly so verifiers don't need to brute-
    /// force which key belongs to which signature.
    pub bft_signers: Vec<String>,
}

impl Block {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Create an unsigned, uncommitted BFT block template.
    ///
    /// The caller must:
    /// 1. Set `state_root` after applying transactions.
    /// 2. Collect `bft_signatures` + `bft_signers` via the voting round.
    /// 3. Call `finalize_hash()` once all fields are set.
    pub fn new_bft(
        index: u64,
        transactions: Vec<Transaction>,
        previous_hash: String,
        epoch: u64,
        bft_round: u32,
        proposer: String,
    ) -> Self {
        let timestamp = Utc::now().timestamp();
        let merkle_tree = MerkleTree::from_transactions(&transactions);
        let merkle_root = merkle_tree.root_hash().unwrap_or_else(|| "0".repeat(64));

        let mut block = Self {
            index,
            timestamp,
            transactions,
            previous_hash,
            hash: String::new(),
            merkle_root,
            state_root: String::new(),
            epoch,
            bft_round,
            proposer,
            bft_signatures: vec![],
            bft_signers: vec![],
        };
        block.hash = block.calculate_hash();
        block
    }

    /// Create the Quanta v2 genesis block.
    ///
    /// The genesis block has no proposer, no BFT signatures (it is
    /// hardcoded and trusted by all nodes), and an empty state.
    ///
    /// CONSENSUS-CRITICAL: The genesis hash is hardcoded in blockchain.rs.
    /// Any change to these fields requires a new hash to be computed and
    /// burned into the code.
    pub fn genesis() -> Self {
        // 2026-06-06 00:00:01 UTC — Quanta v2 testnet reset (validator wallet replacement)
        let timestamp = 1780704001i64;

        let mut genesis = Self {
            index: 0,
            timestamp,
            transactions: vec![],
            previous_hash: "0".repeat(64),
            hash: String::new(),
            merkle_root: "0".repeat(64),
            state_root: "0".repeat(64),
            epoch: 0,
            bft_round: 0,
            proposer: "GENESIS".to_string(),
            bft_signatures: vec![],
            bft_signers: vec![],
        };
        genesis.hash = genesis.calculate_hash();
        genesis
    }

    // -----------------------------------------------------------------------
    // Hashing
    // -----------------------------------------------------------------------

    /// Compute the canonical block hash.
    ///
    /// CONSENSUS RULES (FROZEN):
    /// - All integers little-endian.
    /// - Transactions represented by their individual hashes, comma-joined.
    /// - BFT signatures hex-encoded, comma-joined (in order).
    /// - BFT signers comma-joined (in order, same order as signatures).
    /// - `proposer`, `epoch`, `bft_round`, `state_root` all included.
    pub fn calculate_hash(&self) -> String {
        let transactions_str = self
            .transactions
            .iter()
            .map(|tx| tx.hash())
            .collect::<Vec<String>>()
            .join(",");

        let signatures_str = self
            .bft_signatures
            .iter()
            .map(|sig| hex::encode(sig))
            .collect::<Vec<String>>()
            .join(",");

        let signers_str = self.bft_signers.join(",");

        let data = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.index,
            self.timestamp,
            transactions_str,
            self.previous_hash,
            self.merkle_root,
            self.state_root,
            self.epoch,
            self.bft_round,
            self.proposer,
            signatures_str,
            signers_str,
        );

        double_sha3(data.as_bytes())
    }

    /// Recompute and store `self.hash` after all fields are finalised.
    pub fn finalize_hash(&mut self) {
        self.hash = self.calculate_hash();
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validate block structure (NOT BFT certificate).
    ///
    /// This checks:
    /// 1. Hash integrity — `self.hash` matches recalculated value.
    /// 2. Merkle root integrity.
    /// 3. Chain linkage (previous_hash, index, timestamp).
    /// 4. Signature blob sizes are within Falcon-512 bounds.
    ///
    /// BFT certificate verification (2/3 majority check) is done separately
    /// in `Blockchain::validate_bft_certificate()` because it requires access
    /// to the epoch committee which is not stored on the block itself.
    pub fn is_valid(&self, previous_block: Option<&Block>) -> bool {
        // 1. Hash integrity
        if self.hash != self.calculate_hash() {
            tracing::warn!("Block {}: hash does not match contents", self.index);
            return false;
        }

        // 2. Merkle root integrity
        let tree = MerkleTree::from_transactions(&self.transactions);
        let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
        if self.merkle_root != computed_root {
            tracing::warn!(
                "Block {}: Merkle root mismatch: block={} computed={}",
                self.index, self.merkle_root, computed_root
            );
            return false;
        }

        // 3. BFT signature blob size pre-check.
        // Full cryptographic verification is in validate_bft_certificate().
        // We just confirm each blob is within the legal Falcon-512 bounds so
        // that downstream parsing can never panic.
        for (i, sig) in self.bft_signatures.iter().enumerate() {
            if sig.len() < FALCON512_SIG_MIN_BYTES || sig.len() > FALCON512_SIG_MAX_BYTES {
                tracing::warn!(
                    "Block {}: bft_signatures[{}] length {} is outside Falcon-512 bounds [{}, {}]",
                    self.index, i, sig.len(), FALCON512_SIG_MIN_BYTES, FALCON512_SIG_MAX_BYTES
                );
                return false;
            }
        }

        // 4. Signer list length must match signature list length.
        if self.bft_signatures.len() != self.bft_signers.len() {
            tracing::warn!(
                "Block {}: bft_signatures.len() ({}) != bft_signers.len() ({})",
                self.index, self.bft_signatures.len(), self.bft_signers.len()
            );
            return false;
        }

        // 5. Chain linkage and timestamp checks (skip for genesis).
        if let Some(prev) = previous_block {
            if self.previous_hash != prev.hash {
                tracing::warn!(
                    "Block {}: previous_hash mismatch (expected {})",
                    self.index, prev.hash
                );
                return false;
            }
            if self.index != prev.index + 1 {
                tracing::warn!(
                    "Block {}: index {} is not parent {} + 1",
                    self.index, self.index, prev.index
                );
                return false;
            }
            if self.timestamp <= prev.timestamp {
                tracing::warn!(
                    "Block {}: timestamp {} not after parent timestamp {}",
                    self.index, self.timestamp, prev.timestamp
                );
                return false;
            }
            let now = Utc::now().timestamp();
            if self.timestamp > now + 7200 {
                tracing::warn!(
                    "Block {}: timestamp {} is more than 2 hours in the future",
                    self.index, self.timestamp
                );
                return false;
            }
        }

        true
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Payload that validators sign during the BFT voting round.
    ///
    /// CONSENSUS RULE: every validator signs exactly this 32-byte hash.
    /// Format: SHA3-256("QUANTA_BFT_V2:" || block_hash || epoch_le || round_le)
    pub fn bft_signing_payload(&self) -> [u8; 32] {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        hasher.update(b"QUANTA_BFT_V2:");
        hasher.update(self.hash.as_bytes());
        hasher.update(&self.epoch.to_le_bytes());
        hasher.update(&self.bft_round.to_le_bytes());
        let result = hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&result);
        out
    }

    /// Total transaction fees in this block (microunits).
    pub fn get_total_fees(&self) -> u64 {
        self.transactions
            .iter()
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.fee)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn genesis_hash_is_deterministic() {
        let g1 = Block::genesis();
        let g2 = Block::genesis();
        assert_eq!(g1.hash, g2.hash, "Genesis hash must be deterministic");
        assert!(!g1.hash.is_empty());
    }

    #[test]
    fn genesis_is_valid() {
        let genesis = Block::genesis();
        assert!(genesis.is_valid(None), "Genesis block must pass is_valid(None)");
    }

    #[test]
    fn bft_block_chain_linkage() {
        let genesis = Block::genesis();
        let mut block1 = Block::new_bft(
            1,
            vec![],
            genesis.hash.clone(),
            0,
            0,
            "0xproposer".to_string(),
        );
        block1.state_root = "0".repeat(64);
        block1.finalize_hash();

        assert!(block1.is_valid(Some(&genesis)));
    }

    #[test]
    fn tampered_block_fails_validation() {
        let genesis = Block::genesis();
        let mut block1 = Block::new_bft(
            1,
            vec![],
            genesis.hash.clone(),
            0,
            0,
            "0xproposer".to_string(),
        );
        block1.state_root = "0".repeat(64);
        block1.finalize_hash();

        // Tamper after hashing
        block1.epoch = 99;
        assert!(
            !block1.is_valid(Some(&genesis)),
            "Tampered block must fail is_valid()"
        );
    }

    #[test]
    fn bft_signing_payload_is_deterministic() {
        let genesis = Block::genesis();
        assert_eq!(
            genesis.bft_signing_payload(),
            genesis.bft_signing_payload(),
            "BFT signing payload must be deterministic"
        );
    }
}
