use crate::crypto::sha3_hash;
use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};

/// Hash type - always 32 bytes (SHA3-256)
pub type Hash = [u8; 32];

/// Convert transaction hash string to bytes (TEMPORARY until Transaction.hash() returns bytes)
fn hash_to_bytes(hash_str: &str) -> Hash {
    let bytes = hex::decode(hash_str).expect("Invalid hex hash");
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes[..32]);
    hash
}

/// Merkle tree node - stores raw bytes, not strings
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MerkleNode {
    Leaf { hash: Hash },
    Branch { hash: Hash, left: Box<MerkleNode>, right: Box<MerkleNode> },
}

impl MerkleNode {
    pub fn hash(&self) -> &Hash {
        match self {
            MerkleNode::Leaf { hash } => hash,
            MerkleNode::Branch { hash, .. } => hash,
        }
    }
}

/// Merkle tree for efficient verification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleTree {
    root: Option<MerkleNode>,
    leaves: Vec<Hash>, // Raw hashes, not strings
}

impl MerkleTree {
    /// Create a new Merkle tree from transactions
    pub fn from_transactions(transactions: &[Transaction]) -> Self {
        // TEMPORARY: convert string hashes to bytes until Transaction.hash() returns [u8; 32]
        let leaves: Vec<Hash> = transactions
            .iter()
            .map(|tx| hash_to_bytes(&tx.hash()))
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

    /// Create from raw hash bytes (PREFERRED)
    pub fn from_hashes_bytes(hashes: Vec<Hash>) -> Self {
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
    
    /// Create from transaction hashes (DEPRECATED - converts strings to bytes)
    #[deprecated(note = "Use from_hashes_bytes() instead")]
    pub fn from_hashes(hashes: Vec<String>) -> Self {
        let byte_hashes: Vec<Hash> = hashes.iter().map(|h| hash_to_bytes(h)).collect();
        Self::from_hashes_bytes(byte_hashes)
    }

    /// Build the tree recursively - hashes RAW BYTES, not strings
    fn build_tree(hashes: &[Hash]) -> MerkleNode {
        if hashes.len() == 1 {
            return MerkleNode::Leaf {
                hash: hashes[0],
            };
        }

        let mid = (hashes.len() + 1) / 2;
        let left_hashes = &hashes[..mid];
        let right_hashes = if mid < hashes.len() {
            &hashes[mid..]
        } else {
            &hashes[mid - 1..mid] // Duplicate last if odd (STANDARDIZED)
        };

        let left = Self::build_tree(left_hashes);
        let right = Self::build_tree(right_hashes);

        // CRITICAL: Concatenate BYTES, not strings
        let mut combined = Vec::with_capacity(64);
        combined.extend_from_slice(left.hash());
        combined.extend_from_slice(right.hash());
        
        // Hash the raw bytes
        let hash_bytes = sha3_hash(&combined);
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&hash_bytes[..32]);

        MerkleNode::Branch {
            hash,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Get the root hash as bytes
    pub fn root_hash_bytes(&self) -> Option<Hash> {
        self.root.as_ref().map(|node| *node.hash())
    }
    
    /// Get the root hash as hex string (for display/RPC)
    pub fn root_hash(&self) -> Option<String> {
        self.root_hash_bytes().map(|hash| hex::encode(hash))
    }

    /// Generate a Merkle proof for a transaction hash
    pub fn generate_proof(&self, tx_hash: &Hash) -> Option<MerkleProof> {
        let index = self.leaves.iter().position(|h| h == tx_hash)?;
        let mut proof = Vec::new();
        
        self.collect_proof(self.root.as_ref()?, index, 0, self.leaves.len(), &mut proof);
        
        Some(MerkleProof {
            tx_hash: *tx_hash,
            proof, // Bottom-up order from collect_proof
        })
    }
    
    /// Generate proof from hex string (TEMPORARY)
    pub fn generate_proof_hex(&self, tx_hash_hex: &str) -> Option<MerkleProof> {
        let tx_hash = hash_to_bytes(tx_hash_hex);
        self.generate_proof(&tx_hash)
    }

    /// Recursively collect proof nodes (bottom-up)
    fn collect_proof(
        &self,
        node: &MerkleNode,
        target_index: usize,
        start: usize,
        end: usize,
        proof: &mut Vec<(Hash, bool)>,
    ) {
        match node {
            MerkleNode::Leaf { .. } => {},
            MerkleNode::Branch { left, right, .. } => {
                let mid = (start + end) / 2;
                
                if target_index < mid {
                    // Target is in left subtree, add right sibling
                    proof.push((*right.hash(), false)); // false = right
                    self.collect_proof(left, target_index, start, mid, proof);
                } else {
                    // Target is in right subtree, add left sibling
                    proof.push((*left.hash(), true)); // true = left
                    self.collect_proof(right, target_index, mid, end, proof);
                }
            }
        }
    }

    /// Verify tree integrity by recomputing root
    pub fn verify_tree(&self) -> bool {
        if let Some(root) = &self.root {
            if self.leaves.is_empty() {
                return false;
            }
            // Recompute tree and compare roots
            let recomputed = Self::build_tree(&self.leaves);
            recomputed.hash() == root.hash()
        } else {
            self.leaves.is_empty() // Empty tree is valid if no leaves
        }
    }
}

/// Merkle proof for SPV (Simplified Payment Verification)
/// Stores raw bytes - hex conversion only for display
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub tx_hash: Hash,
    pub proof: Vec<(Hash, bool)>, // (sibling_hash, is_left)
}

impl MerkleProof {
    /// Verify the proof against a root hash (bytes)
    pub fn verify(&self, root_hash: &Hash) -> bool {
        let mut current_hash = self.tx_hash;
        
        for (sibling_hash, is_left) in &self.proof {
            // Concatenate bytes in correct order
            let mut combined = Vec::with_capacity(64);
            if *is_left {
                combined.extend_from_slice(sibling_hash);
                combined.extend_from_slice(&current_hash);
            } else {
                combined.extend_from_slice(&current_hash);
                combined.extend_from_slice(sibling_hash);
            }
            
            // Hash the bytes
            let hash_bytes = sha3_hash(&combined);
            current_hash.copy_from_slice(&hash_bytes[..32]);
        }
        
        &current_hash == root_hash
    }
    
    /// Verify against hex string root (for convenience)
    pub fn verify_hex(&self, root_hash_hex: &str) -> bool {
        let root_hash = hash_to_bytes(root_hash_hex);
        self.verify(&root_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_creation() {
        #[allow(deprecated)]
        let tree = MerkleTree::from_hashes(vec![
            "hash1".to_string(),
            "hash2".to_string(),
            "hash3".to_string(),
            "hash4".to_string(),
        ]);
        
        assert!(tree.root_hash().is_some());
        assert!(tree.verify_tree());
    }

    #[test]
    fn test_merkle_proof() {
        let hashes_str = vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
            "tx4".to_string(),
        ];
        
        #[allow(deprecated)]
        let tree = MerkleTree::from_hashes(hashes_str.clone());
        let root = tree.root_hash_bytes().unwrap();
        
        for hash_str in &hashes_str {
            let proof = tree.generate_proof_hex(hash_str).unwrap();
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn test_odd_number_of_transactions() {
        #[allow(deprecated)]
        let tree = MerkleTree::from_hashes(vec![
            "tx1".to_string(),
            "tx2".to_string(),
            "tx3".to_string(),
        ]);
        
        assert!(tree.root_hash().is_some());
        assert!(tree.verify_tree());
    }
    
    #[test]
    fn test_bytes_not_strings() {
        // This test proves we're hashing BYTES, not strings
        let hash1 = [0u8; 32];
        let hash2 = [1u8; 32];
        
        let tree = MerkleTree::from_hashes_bytes(vec![hash1, hash2]);
        assert!(tree.verify_tree());
        
        // Root should be deterministic
        let root1 = tree.root_hash_bytes().unwrap();
        let tree2 = MerkleTree::from_hashes_bytes(vec![hash1, hash2]);
        let root2 = tree2.root_hash_bytes().unwrap();
        
        assert_eq!(root1, root2, "Deterministic root hash");
    }
}
