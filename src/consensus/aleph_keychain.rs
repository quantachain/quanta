use aleph_bft::{
    Index, Keychain, MultiKeychain, NodeCount, NodeIndex, PartialMultisignature, SignatureSet,
};
use crate::crypto::signatures::FalconKeypair;
use crate::crypto::wallet::QuantumWallet;
use crate::consensus::authorities::compute_committee;
use crate::core::transaction::AccountState;
use codec::{Decode, Encode};
use std::sync::Arc;
use tokio::sync::RwLock;

/// AlephBFT Signature Wrapper for Falcon-512 signatures
#[derive(Debug, Clone, PartialEq, Eq, codec::Encode, codec::Decode)]
pub struct FalconSignature {
    pub raw: Vec<u8>,
}


/// Bridges the local `QuantumWallet` and the epoch committee to the `aleph_bft::Keychain` traits.
#[derive(Clone)]
pub struct QuantaKeychain {
    /// Our local validator wallet
    wallet: Arc<QuantumWallet>,
    /// Our node index within the committee (0 to N-1)
    my_index: NodeIndex,
    /// Total number of nodes in the committee
    node_count: NodeCount,
    /// Ordered list of committee public keys (raw bytes or addresses)
    /// Used for signature verification of other nodes.
    committee_pubkeys: Vec<Vec<u8>>,
}

impl QuantaKeychain {
    pub fn new(
        wallet: Arc<QuantumWallet>,
        my_index: NodeIndex,
        node_count: NodeCount,
        committee_pubkeys: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            wallet,
            my_index,
            node_count,
            committee_pubkeys,
        }
    }
}

impl Index for QuantaKeychain {
    fn index(&self) -> NodeIndex {
        self.my_index
    }
}

impl Keychain for QuantaKeychain {
    type Signature = FalconSignature;

    fn node_count(&self) -> NodeCount {
        self.node_count
    }

    fn sign(&self, msg: &[u8]) -> Self::Signature {
        // We use the `sign_transaction_canonical` or a direct hash sign.
        // Wait, AlephBFT messages are already hashed or we hash them.
        // The `msg` passed here is the result of `Signable::hash()`.
        // So `msg` is the hash.
        let mut hash_arr = [0u8; 32];
        let len = msg.len().min(32);
        hash_arr[..len].copy_from_slice(&msg[..len]);
        let sig_bytes = self.wallet.keypair.sign_hash(&hash_arr);
        FalconSignature { raw: sig_bytes }
    }

    fn verify(&self, msg: &[u8], sgn: &Self::Signature, index: NodeIndex) -> bool {
        if index.0 >= self.committee_pubkeys.len() {
            return false;
        }
        let pubkey = &self.committee_pubkeys[index.0];
        
        let mut hash_arr = [0u8; 32];
        let len = msg.len().min(32);
        hash_arr[..len].copy_from_slice(&msg[..len]);
        
        crate::crypto::signatures::verify_hash_strict(&hash_arr, &sgn.raw, pubkey)
    }
}

impl MultiKeychain for QuantaKeychain {
    type PartialMultisignature = SignatureSet<Self::Signature>;

    fn bootstrap_multi(
        &self,
        signature: &Self::Signature,
        index: NodeIndex,
    ) -> Self::PartialMultisignature {
        SignatureSet::add_signature(
            SignatureSet::with_size(self.node_count()),
            signature,
            index,
        )
    }

    fn is_complete(&self, msg: &[u8], partial: &Self::PartialMultisignature) -> bool {
        let signature_count = partial.iter().count();
        // BFT threshold is 2/3 of nodes.
        let required = (self.node_count().0 * 2) / 3 + 1;
        if signature_count < required {
            return false;
        }
        partial
            .iter()
            .all(|(i, sgn)| self.verify(msg, sgn, i))
    }
}

/// A hasher implementation for AlephBFT.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantaHasher;

impl aleph_bft::Hasher for QuantaHasher {
    type Hash = [u8; 32];

    fn hash(s: &[u8]) -> Self::Hash {
        let mut hash_arr = [0u8; 32];
        let h = crate::crypto::sha3_hash(s);
        let len = h.len().min(32);
        hash_arr[..len].copy_from_slice(&h[..len]);
        hash_arr
    }
}
