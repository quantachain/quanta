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
        };
        block.hash = block.calculate_hash();
        block
    }

    /// Create the genesis block (first block in chain)
    pub fn genesis() -> Self {
        let mut genesis = Self {
            index: 0,
            timestamp: 1640000000, // Fixed timestamp
            transactions: vec![],
            previous_hash: "0".repeat(64),
            nonce: 0,
            hash: String::new(),
            difficulty: 4,
            merkle_root: "0".repeat(64),
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
            "{}:{}:{}:{}:{}:{}:{}",
            self.index,
            self.timestamp,
            transactions_str,
            self.previous_hash,
            self.nonce,
            self.difficulty,
            self.merkle_root
        );

        double_sha3(data.as_bytes())
    }

    /// Check if block hash meets difficulty target
    pub fn has_valid_hash(&self) -> bool {
        let target = "0".repeat(self.difficulty as usize);
        self.hash.starts_with(&target)
    }

    /// Mine the block by finding a valid nonce
    pub fn mine(&mut self) {
        println!(
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
                let hashrate = hash_count as f64 / elapsed;
                println!(
                    "Block mined! Nonce: {}, Hashes: {}, Time: {:.2}s, Hashrate: {:.0} H/s",
                    self.nonce, hash_count, elapsed, hashrate
                );
                break;
            }
            
            self.nonce += 1;
            
            // Progress indicator every 100k hashes
            if hash_count % 100_000 == 0 {
                print!("\rHashes: {}k", hash_count / 1000);
                use std::io::{self, Write};
                io::stdout().flush().unwrap();
            }
        }
    }

    /// Validate block structure and hash
    pub fn is_valid(&self, previous_block: Option<&Block>) -> bool {
        // Check hash is correct
        if self.hash != self.calculate_hash() {
            println!("Invalid hash calculation");
            return false;
        }

        // Check proof-of-work
        if !self.has_valid_hash() {
            println!("Invalid proof-of-work");
            return false;
        }

        // CRITICAL: Validate merkle root (prevents merkle root lying)
        let tree = MerkleTree::from_transactions(&self.transactions);
        let computed_root = tree.root_hash().unwrap_or_else(|| "0".repeat(64));
        if self.merkle_root != computed_root {
            println!("Invalid merkle root: expected {}, got {}", computed_root, self.merkle_root);
            return false;
        }

        // Check previous hash linkage
        if let Some(prev) = previous_block {
            if self.previous_hash != prev.hash {
                println!("Invalid previous hash linkage");
                return false;
            }
            if self.index != prev.index + 1 {
                println!("Invalid block index");
                return false;
            }
        }

        // Verify all transaction signatures
        for tx in &self.transactions {
            if !tx.is_coinbase() && !tx.verify() {
                println!("Invalid transaction signature");
                return false;
            }
        }

        true
    }

    /// Get total transaction fees in block (u64 microunits)
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
    fn test_genesis_block() {
        let genesis = Block::genesis();
        assert_eq!(genesis.index, 0);
        assert_eq!(genesis.previous_hash.len(), 64);
    }

    #[test]
    fn test_block_hashing() {
        let block = Block::new(1, vec![], "previous_hash".to_string(), 1);
        let hash1 = block.calculate_hash();
        let hash2 = block.calculate_hash();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_mining() {
        let mut block = Block::new(1, vec![], "0".repeat(64), 2);
        block.mine();
        assert!(block.has_valid_hash());
        assert!(block.hash.starts_with("00"));
    }
}
