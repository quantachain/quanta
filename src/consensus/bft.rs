use serde::{Serialize, Deserialize};
use std::sync::Arc;
use aleph_bft::{Keychain as AlephKeychain, NodeIndex, MultiKeychain, NodeCount};
use async_trait::async_trait;
use falcon_rust::falcon512::{SecretKey as PrivateKey, PublicKey, Signature};
use sha3::{Digest, Sha3_256};

/// A Quanta BFT Signable Data — represents the commitment to a specific block.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash, parity_scale_codec::Encode, parity_scale_codec::Decode)]
pub struct BftData {
    pub chain_id: u32,
    pub height: u64,
    pub block_hash: [u8; 32],
}



/// FalconKeychain: Bridges AlephBFT consensus with Quanta's Falcon-512 cryptography.
/// 
/// This implementation ensures that every consensus message is signed using 
/// NIST-standardized Post-Quantum signatures.
#[derive(Clone)]
pub struct FalconKeychain {
    node_index: NodeIndex,
    private_key: Arc<PrivateKey>,
    validator_pks: Vec<PublicKey>,
    chain_id: u32,
}

impl FalconKeychain {
    pub fn new(
        node_index: NodeIndex,
        private_key: PrivateKey,
        validator_pks: Vec<PublicKey>,
        chain_id: u32,
    ) -> Self {
        Self {
            node_index,
            private_key: Arc::new(private_key),
            validator_pks,
            chain_id,
        }
    }
}

impl aleph_bft::Index for FalconKeychain {
    fn index(&self) -> NodeIndex {
        self.node_index
    }
}

#[async_trait]
impl AlephKeychain for FalconKeychain {
    type Signature = Vec<u8>;

    fn node_count(&self) -> NodeCount {
        NodeCount(self.validator_pks.len())
    }

    fn sign(&self, msg: &[u8]) -> Self::Signature {
        // Domain separation and chain ID binding for replay protection
        let mut hasher = Sha3_256::new();
        hasher.update(b"QUANTA_BFT_V1:");
        hasher.update(&self.chain_id.to_le_bytes());
        hasher.update(msg);
        let digest = hasher.finalize();

        // Sign the digest with Falcon-512
        let sig = falcon_rust::falcon512::sign(&digest, &self.private_key);
        sig.to_bytes().to_vec()
    }

    fn verify(&self, msg: &[u8], sig: &Self::Signature, index: NodeIndex) -> bool {
        let pk = match self.validator_pks.get(index.0) {
            Some(pk) => pk,
            None => return false,
        };

        let falcon_sig = match Signature::from_bytes(sig) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let mut hasher = Sha3_256::new();
        hasher.update(b"QUANTA_BFT_V1:");
        hasher.update(&self.chain_id.to_le_bytes());
        hasher.update(msg);
        let digest = hasher.finalize();

        // Verify the signature against the validator's Falcon public key
        falcon_rust::falcon512::verify(&digest, &falcon_sig, pk)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, parity_scale_codec::Encode, parity_scale_codec::Decode)]
pub struct FalconPartialMultisig {
    pub signatures: Vec<(NodeIndex, Vec<u8>)>,
}

impl aleph_bft::PartialMultisignature for FalconPartialMultisig {
    type Signature = Vec<u8>;
    fn add_signature(self, signature: &Self::Signature, index: NodeIndex) -> Self {
        let mut new = self;
        new.signatures.push((index, signature.clone()));
        new
    }
}

/// MultiKeychain implementation to support AlephBFT's multi-signature verification.
impl MultiKeychain for FalconKeychain {
    type PartialMultisignature = FalconPartialMultisig;

    fn bootstrap_multi(&self, sig: &Self::Signature, index: NodeIndex) -> Self::PartialMultisignature {
        FalconPartialMultisig {
            signatures: vec![(index, sig.clone())]
        }
    }

    fn is_complete(&self, msg: &[u8], partial: &Self::PartialMultisignature) -> bool {
        // In AlephBFT, 2/3+1 majority is required.
        let threshold = (self.validator_pks.len() * 2) / 3 + 1;
        
        if partial.signatures.len() < threshold {
            return false;
        }

        // Verify each signature in the partial set
        let mut seen = std::collections::HashSet::new();
        for (idx, sig) in &partial.signatures {
            if !seen.insert(idx.0) || !self.verify(msg, sig, *idx) {
                return false;
            }
        }
        true
    }
}

/// BFT Config for Quanta Mainnet
pub struct QuantaBftConfig {
    pub n_validators: usize,
    pub epoch_id: u64,
}

impl QuantaBftConfig {
    pub fn mainnet_v1() -> Self {
        Self {
            n_validators: 21,
            epoch_id: 1,
        }
    }
}
