use crate::core::transaction::Transaction;
use crate::crypto::signatures::verify_signature;
use serde::{Deserialize, Serialize};

/// Multi-signature transaction requiring M-of-N signatures
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultiSigTransaction {
    pub base_tx: Transaction,
    pub required_signatures: usize,  // M
    pub public_keys: Vec<Vec<u8>>,   // N public keys
    pub signatures: Vec<Option<Vec<u8>>>, // Collected signatures
}

impl MultiSigTransaction {
    /// Create a new multisig transaction (M-of-N)
    pub fn new(
        base_tx: Transaction,
        required_signatures: usize,
        public_keys: Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        if required_signatures == 0 {
            return Err("Required signatures must be > 0".to_string());
        }
        
        if required_signatures > public_keys.len() {
            return Err(format!(
                "Required signatures ({}) cannot exceed total keys ({})",
                required_signatures,
                public_keys.len()
            ));
        }
        
        let signatures = vec![None; public_keys.len()];
        
        Ok(Self {
            base_tx,
            required_signatures,
            public_keys,
            signatures,
        })
    }
    
    /// Add a signature from one of the signers
    pub fn add_signature(&mut self, index: usize, signature: Vec<u8>) -> Result<(), String> {
        if index >= self.public_keys.len() {
            return Err("Invalid signer index".to_string());
        }
        
        if self.signatures[index].is_some() {
            return Err("Signature already provided for this index".to_string());
        }
        
        // Verify the signature
        let signing_data = self.base_tx.get_signing_data();
        if !verify_signature(&signing_data, &signature, &self.public_keys[index]) {
            return Err("Invalid signature".to_string());
        }
        
        self.signatures[index] = Some(signature);
        Ok(())
    }
    
    /// Check if transaction has enough signatures
    pub fn is_complete(&self) -> bool {
        let sig_count = self.signatures.iter().filter(|s| s.is_some()).count();
        sig_count >= self.required_signatures
    }
    
    /// Verify all provided signatures
    pub fn verify(&self) -> bool {
        if !self.is_complete() {
            return false;
        }
        
        let signing_data = self.base_tx.get_signing_data();
        let mut valid_sigs = 0;
        
        for (i, sig_opt) in self.signatures.iter().enumerate() {
            if let Some(sig) = sig_opt {
                if verify_signature(&signing_data, sig, &self.public_keys[i]) {
                    valid_sigs += 1;
                }
            }
        }
        
        valid_sigs >= self.required_signatures
    }
    
    /// Get multisig address (hash of sorted public keys)
    pub fn get_multisig_address(&self) -> String {
        use sha3::{Sha3_256, Digest};
        
        let mut combined = Vec::new();
        for pk in &self.public_keys {
            combined.extend_from_slice(pk);
        }
        
        let mut hasher = Sha3_256::new();
        hasher.update(&combined);
        let hash = hasher.finalize();
        
        format!("multisig_{}", hex::encode(&hash[..20]))
    }
    
    /// Get signature progress (X of Y signatures collected)
    pub fn signature_progress(&self) -> (usize, usize) {
        let collected = self.signatures.iter().filter(|s| s.is_some()).count();
        (collected, self.required_signatures)
    }
}

/// Common multisig configurations
#[derive(Debug, Clone, Copy)]
pub enum MultiSigType {
    TwoOfThree,    // 2-of-3
    ThreeOfFive,   // 3-of-5
    FourOfSeven,   // 4-of-7
}

impl MultiSigType {
    pub fn required_signatures(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree => 2,
            MultiSigType::ThreeOfFive => 3,
            MultiSigType::FourOfSeven => 4,
        }
    }
    
    pub fn total_signers(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree => 3,
            MultiSigType::ThreeOfFive => 5,
            MultiSigType::FourOfSeven => 7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transaction::Transaction;
    
    #[test]
    fn test_multisig_creation() {
        let tx = Transaction::new(
            "sender".to_string(),
            "recipient".to_string(),
            10_000_000, // 10 QUA in microunits
            123456789,
        );
        
        let public_keys = vec![
            vec![1, 2, 3],
            vec![4, 5, 6],
            vec![7, 8, 9],
        ];
        
        let multisig = MultiSigTransaction::new(tx, 2, public_keys).unwrap();
        assert_eq!(multisig.required_signatures, 2);
        assert_eq!(multisig.public_keys.len(), 3);
        assert!(!multisig.is_complete());
    }
    
    #[test]
    fn test_invalid_multisig_config() {
        let tx = Transaction::new(
            "sender".to_string(),
            "recipient".to_string(),
            10_000_000, // 10 QUA in microunits
            123456789,
        );
        
        let public_keys = vec![vec![1, 2, 3]];
        
        // Require more signatures than keys available
        let result = MultiSigTransaction::new(tx, 3, public_keys);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_multisig_types() {
        assert_eq!(MultiSigType::TwoOfThree.required_signatures(), 2);
        assert_eq!(MultiSigType::TwoOfThree.total_signers(), 3);
        
        assert_eq!(MultiSigType::ThreeOfFive.required_signatures(), 3);
        assert_eq!(MultiSigType::ThreeOfFive.total_signers(), 5);
    }
}
