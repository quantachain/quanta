// Utility to compute genesis block hash
// Run with: cargo run --bin genesis_hash

use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {
    pub index: u64,
    pub timestamp: i64,
    pub transactions: Vec<String>,
    pub previous_hash: String,
    pub nonce: u64,
    pub hash: String,
    pub difficulty: u32,
    pub merkle_root: String,
}

fn double_sha3(data: &[u8]) -> String {
    use sha3::{Sha3_256, Digest};
    let first_hash = Sha3_256::digest(data);
    let second_hash = Sha3_256::digest(&first_hash);
    format!("{:x}", second_hash)
}

impl Block {
    pub fn genesis() -> Self {
        let mut genesis = Self {
            index: 0,
            timestamp: 1735689600, // 2025-01-01 00:00:00 UTC
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

    pub fn calculate_hash(&self) -> String {
        let transactions_str = self.transactions.join(",");
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
}

fn main() {
    let genesis = Block::genesis();
    println!("Genesis Block Details:");
    println!("  Timestamp: {} (2025-01-01 00:00:00 UTC)", genesis.timestamp);
    println!("  Index: {}", genesis.index);
    println!("  Difficulty: {}", genesis.difficulty);
    println!("  Hash: {}", genesis.hash);
    println!("\n✅ Update blockchain.rs with this:");
    println!("const GENESIS_HASH: &str = \"{}\";", genesis.hash);
}
