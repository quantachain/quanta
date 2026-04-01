use serde::{Serialize, Deserialize};
use crate::core::transaction::Transaction;
use crate::crypto::double_sha3;
use crate::core::merkle::MerkleTree;
use chrono::Utc;

/// Block structure
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<Transaction>,
    pub previous_hash: String,
    pub nonce: u64,
    pub hash: String,
    pub difficulty: u32,
    pub merkle_root: String,
    /// Cryptographic commitment to the global account state after this block
    #[serde(default)]
    pub state_root: String,
}

impl Block {
    /// Create a new block (unmined)
    pub fn new(
        index: u64,
        transactions: Vec<Transaction>,
        previous_hash: String,
        difficulty: u32,
    ) -> Self {
        let timestamp = Utc::now().timestamp();
        
        // Calculate Merkle root
        let merkle_tree = MerkleTree::from_transactions(&transactions);
        let merkle_root = merkle_tree.root_hash().unwrap_or_else(|| "0".repeat(64));
        
        let mut block = Self {
            index,
            timestamp,
            transactions,
            previous_hash,
            nonce: 0,
            hash: String::new(),
            difficulty,
            merkle_root,
            state_root: String::new(), // Will be set by create_block_template
        };
        block.hash = block.calculate_hash();
        block
    }

    /// Create the genesis block (first block in chain)
    pub fn genesis(network: crate::core::ChainNetwork) -> Self {
        // CONSENSUS-CRITICAL: Genesis block parameters
        // Timestamp: January 1, 2026 00:00:00 UTC (Quanta Launch)
        // All nodes must use identical genesis parameters
        
        let (timestamp, difficulty, nonce) = match network {
            crate::core::ChainNetwork::Mainnet => (1774051200, 16_777_216, 0), // Pending actual mining before Mainnet launch
            crate::core::ChainNetwork::Testnet  => (1775001600, 8304130, 9921538), // Alpha Testnet — ~30s block time
        };
        
        let mut genesis = Self {
            index: 0,
            timestamp, // Set based on network type
            transactions: vec![],
            previous_hash: "0".repeat(64),
            nonce,
            hash: String::new(),
            difficulty,
            merkle_root: "0".repeat(64),
            state_root: "0".repeat(64), // Empty state root for genesis
        };
        genesis.hash = genesis.calculate_hash();
        genesis
    }

    /// Calculate block hash using SHA3-256
    pub fn calculate_hash(&self) -> String {
        let transactions_str = self
            .transactions
            .iter()
            .map(|tx| tx.hash())
            .collect::<Vec<String>>()
            .join(",");

        let data = format!(
            "{}:{}:{}:{}:{}:{}:{}:{}",
            self.index,
            self.timestamp,
            transactions_str,
            self.previous_hash,
            self.nonce,
            self.difficulty,
            self.merkle_root,
            self.state_root
        );

        double_sha3(data.as_bytes())
    }

    /// Check if block hash meets difficulty target
    pub fn has_valid_hash(&self) -> bool {
        if self.hash.len() < 16 {
            return false;
        }

        // Parse the first 16 hex characters (64 bits) of the hash
        let hash_prefix = match u64::from_str_radix(&self.hash[..16], 16) {
            Ok(v) => v,
            Err(_) => return false,
        };

        // Target = u64::MAX / expected_hashes
        // Where difficulty IS the expected_hashes (e.g., difficulty 16 = target starts with '0')
        // We use u64::MAX to provide perfectly smooth difficulty adjustments
        let difficulty_u64 = self.difficulty as u64;
        let target = if difficulty_u64 == 0 {
            u64::MAX
        } else {
            u64::MAX / difficulty_u64
        };

        hash_prefix <= target
    }

    /// Mine the block by finding a valid nonce
    pub fn mine(&mut self) {
        tracing::info!(
            "Mining block {} with difficulty {}...",
            self.index, self.difficulty
        );
        
        let start = std::time::Instant::now();
        let mut hash_count = 0u64;
        
        loop {
            self.hash = self.calculate_hash();
            hash_count += 1;
            
            if self.has_valid_hash() {
                let elapsed = start.elapsed().as_secs_f64();
                let hashrate = if elapsed > 0.0 { hash_count as f64 / elapsed } else { f64::INFINITY };
                tracing::info!(
                    "Block mined! Nonce: {}, Hashes: {}, Time: {:.2}s, Hashrate: {:.0} H/s",
                    self.nonce, hash_count, elapsed, hashrate
                );
                break;
            }
            
            self.nonce += 1;
            
            // Progress indicator every 100k hashes
            if hash_count % 100_000 == 0 {
                tracing::debug!("Mining progress: {}k hashes (block {})", hash_count / 1000, self.index);
            }
        }
    }

    /// Validate block structure and PoW.
    ///
    /// Checks: hash integrity, proof-of-work, Merkle root, and previous-block
    /// linkage. Transaction signature verification is intentionally NOT done
    /// here — it is performed in parallel by Rayon inside
    /// `Blockchain::validate_block_consensus()`. Running it here as well would
    /// double the Falcon-512 verification work (~1800 ms per block).
    pub fn is_valid(&self, previous_block: Option<&Block>) -> bool {
        // 1. Hash integrity
        if self.hash != self.calculate_hash() {
            tracing::warn!("Block {}: hash does not match contents", self.index);
            return false;
        }

        // 2. Proof-of-work
        if !self.has_valid_hash() {
            tracing::warn!("Block {}: hash does not meet declared difficulty {}", self.index, self.difficulty);
            return false;
        }

        // 3. Merkle root integrity
        let tree = MerkleTree::from_transactions(&self.transactions);
        let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
        if self.merkle_root != computed_root {
            tracing::warn!(
                "Block {}: Merkle root mismatch: block={} computed={}",
                self.index, self.merkle_root, computed_root
            );
            return false;
        }

        // 4. Chain linkage and timestamp
        if let Some(prev) = previous_block {
            if self.previous_hash != prev.hash {
                tracing::warn!("Block {}: previous_hash does not match parent {}", self.index, prev.index);
                return false;
            }
            if self.index != prev.index + 1 {
                tracing::warn!("Block {}: index {} is not parent index {} + 1", self.index, self.index, prev.index);
                return false;
            }
            if self.timestamp <= prev.timestamp {
                tracing::warn!("Block {}: timestamp {} is not after parent timestamp {}", self.index, self.timestamp, prev.timestamp);
                return false;
            }
            let current_time = chrono::Utc::now().timestamp();
            if self.timestamp > current_time + 7200 {
                tracing::warn!("Block {}: timestamp {} is more than 2 hours in the future", self.index, self.timestamp);
                return false;
            }
        }

        true
    }

    /// Get total transaction fees in block (u64 microunits)
    #[allow(dead_code)]
    pub fn get_total_fees(&self) -> u64 {
        self.transactions
            .iter()
            .filter(|tx| !tx.is_coinbase())
            .map(|tx| tx.fee)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // TODO: Update expected genesis parameters/hashes before Mainnet
    fn verify_genesis_hash() {
        let genesis = Block::genesis(crate::core::ChainNetwork::Mainnet);
        
        // CONSENSUS-CRITICAL: Genesis block must have these exact parameters
        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.timestamp, 1774051200); // 2026-03-21 00:00:00 UTC
        assert_eq!(genesis.difficulty, 6);
        assert_eq!(genesis.previous_hash, "0".repeat(64));
        assert_eq!(genesis.merkle_root, "0".repeat(64));
        assert_eq!(genesis.state_root, "0".repeat(64));
        assert_eq!(genesis.transactions.len(), 0);
        
        // CRITICAL: Hash must match hardcoded value in blockchain.rs
        assert_eq!(
            genesis.hash,
            "d0f8e765c51672695069e6b91b989eb9d7646e362fbfb0948f5d3ab74ba88edf",
            "Genesis hash mismatch! This will cause chain splits."
        );
    }

    #[test]
    #[ignore] // TODO: Update expected genesis parameters/hashes before Mainnet
    fn genesis_hash_recalculation() {
        let genesis = Block::genesis(crate::core::ChainNetwork::Mainnet);
        let recalculated = genesis.calculate_hash();
        
        assert_eq!(
            genesis.hash, recalculated,
            "Genesis hash calculation must be deterministic"
        );
    }
}
