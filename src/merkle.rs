use crate::crypto::sha3_hash;
use crate::transaction::Transaction;
use serde::{Deserialize, Serialize};

/// Merkle tree node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MerkleNode {
    Leaf { hash: String, data: Vec<u8> },
    Branch { hash: String, left: Box<MerkleNode>, right: Box<MerkleNode> },
}

impl MerkleNode {
    pub fn hash(&self) -> &str {
        match self {
            MerkleNode::Leaf { hash, .. } => hash,
            MerkleNode::Branch { hash, .. } => hash,
        }
    }
}

/// Merkle tree for efficient verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleTree {
    root: Option<MerkleNode>,
    leaves: Vec<String>,
}

impl MerkleTree {
    /// Create a new Merkle tree from transactions
    pub fn from_transactions(transactions: &[Transaction]) -> Self {
        let leaves: Vec<String> = transactions
            .iter()
            .map(|tx| tx.hash())
            .collect();

        if leaves.is_empty() {
            return Self {
                root: None,
                leaves: Vec::new(),
            };
        }

        let root = Self::build_tree(&leaves);
        Self {
            root: Some(root),
            leaves,
        }
    }

    /// Create from transaction hashes directly
    pub fn from_hashes(hashes: Vec<String>) -> Self {
        if hashes.is_empty() {
            return Self {
                root: None,
                leaves: Vec::new(),
            };
        }

        let root = Self::build_tree(&hashes);
        Self {
            root: Some(root),
            leaves: hashes,
        }
    }

    /// Build the tree recursively
    fn build_tree(hashes: &[String]) -> MerkleNode {
        if hashes.len() == 1 {
            return MerkleNode::Leaf {
                hash: hashes[0].clone(),
                data: hashes[0].as_bytes().to_vec(),
            };
        }

        let mid = (hashes.len() + 1) / 2;
        let left_hashes = &hashes[..mid];
        let right_hashes = if mid < hashes.len() {
            &hashes[mid..]
        } else {
            &hashes[mid - 1..mid] // Duplicate last if odd
        };

        let left = Self::build_tree(left_hashes);
        let right = Self::build_tree(right_hashes);

        let combined = format!("{}{}", left.hash(), right.hash());
        let hash = hex::encode(sha3_hash(combined.as_bytes()));

        MerkleNode::Branch {
            hash,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Get the root hash
    pub fn root_hash(&self) -> Option<String> {
        self.root.as_ref().map(|node| node.hash().to_string())
    }

    /// Generate a Merkle proof for a transaction
    pub fn generate_proof(&self, tx_hash: &str) -> Option<MerkleProof> {
        let index = self.leaves.iter().position(|h| h == tx_hash)?;
        let mut proof = Vec::new();
        
        self.collect_proof(self.root.as_ref()?, index, 0, self.leaves.len(), &mut proof);
        
        Some(MerkleProof {
            tx_hash: tx_hash.to_string(),
            proof,
            index,
            total_leaves: self.leaves.len(),
        })
    }

    /// Recursively collect proof nodes
    fn collect_proof(
        &self,
        node: &MerkleNode,
        target_index: usize,
        start: usize,
        end: usize,
        proof: &mut Vec<(String, bool)>,
    ) {
        match node {
            MerkleNode::Leaf { .. } => {},
            MerkleNode::Branch { left, right, .. } => {
                let mid = (start + end) / 2;
                
                if target_index < mid {
                    // Target is in left subtree, add right sibling
                    proof.push((right.hash().to_string(), false)); // false = right
                    self.collect_proof(left, target_index, start, mid, proof);
                } else {
                    // Target is in right subtree, add left sibling
                    proof.push((left.hash().to_string(), true)); // true = left
                    self.collect_proof(right, target_index, mid, end, proof);
                }
            }
        }
    }

    /// Verify the entire tree
    pub fn verify(&self) -> bool {
        self.root.is_some()
    }
}

/// Merkle proof for SPV (Simplified Payment Verification)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub tx_hash: String,
    pub proof: Vec<(String, bool)>, // (hash, is_left)
    pub index: usize,
    pub total_leaves: usize,
}

impl MerkleProof {
    /// Verify the proof against a root hash
    pub fn verify(&self, root_hash: &str) -> bool {
        let mut current_hash = self.tx_hash.clone();
        
        for (sibling_hash, is_left) in &self.proof {
            let combined = if *is_left {
                format!("{}{}", sibling_hash, current_hash)
            } else {
                format!("{}{}", current_hash, sibling_hash)
            };
            current_hash = hex::encode(sha3_hash(combined.as_bytes()));
        }
        
        current_hash == root_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_creation() {
        let hashes = vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
            "hash4".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes);
        assert!(tree.root_hash().is_some());
    }

    #[test]
    fn test_merkle_proof() {
        let hashes = vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
            "tx4".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes.clone());
        let root = tree.root_hash().unwrap();
        
        for hash in &hashes {
            let proof = tree.generate_proof(hash).unwrap();
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn test_odd_number_of_transactions() {
        let hashes = vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
        ];
        
        let tree = MerkleTree::from_hashes(hashes);
        assert!(tree.root_hash().is_some());
    }
}
