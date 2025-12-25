use pqcrypto_falcon::falcon512::*;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use sha3::{Digest, Sha3_256};

/// Falcon-512 wrapper for quantum-resistant signatures
/// Public key: ~897 bytes, Private key: ~1281 bytes, Signature: ~666 bytes
#[derive(Clone, Debug)]
pub struct FalconKeypair {
    pub public_key: Vec<u8>,
    pub secret_key: Vec<u8>,
}

impl FalconKeypair {
    /// Generate a new Falcon-512 keypair
    pub fn generate() -> Self {
        let (pk, sk) = keypair();
        Self {
            public_key: pk.as_bytes().to_vec(),
            secret_key: sk.as_bytes().to_vec(),
        }
    }

    /// Sign a message with Falcon private key
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let sk = SecretKey::from_bytes(&self.secret_key)
            .expect("Invalid secret key");
        let signed = sign(message, &sk);
        signed.as_bytes().to_vec()
    }

    /// Derive quantum-resistant address from public key
    /// Uses first 20 bytes of SHA3-256 hash (like Ethereum)
    pub fn get_address(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.public_key);
        let hash = hasher.finalize();
        hex::encode(&hash[..20])
    }
}

/// Verify a Falcon signature
pub fn verify_signature(message: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
    match PublicKey::from_bytes(public_key) {
        Ok(pk) => {
            match SignedMessage::from_bytes(signature) {
                Ok(sm) => {
                    match open(&sm, &pk) {
                        Ok(verified_msg) => verified_msg == message,
                        Err(_) => false,
                    }
                }
                Err(_) => false,
            }
        }
        Err(_) => false,
    }
}

/// Calculate SHA3-256 hash (quantum-resistant)
pub fn sha3_hash(data: &[u8]) -> Vec<u8> {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    hasher.finalize().to_vec()
}

/// Calculate double SHA3-256 hash for block hashing
pub fn double_sha3(data: &[u8]) -> String {
    let hash1 = sha3_hash(data);
    let hash2 = sha3_hash(&hash1);
    hex::encode(hash2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let keypair = FalconKeypair::generate();
        assert!(keypair.public_key.len() > 800); // ~897 bytes
        assert!(keypair.secret_key.len() > 1200); // ~1281 bytes
    }

    #[test]
    fn test_signature_verification() {
        let keypair = FalconKeypair::generate();
        let message = b"Quantum-resistant transaction";
        let signature = keypair.sign(message);
        
        assert!(signature.len() > 600); // ~666 bytes signature
        assert!(verify_signature(message, &signature, &keypair.public_key));
    }

    #[test]
    fn test_address_generation() {
        let keypair = FalconKeypair::generate();
        let address = keypair.get_address();
        assert_eq!(address.len(), 40); // 20 bytes = 40 hex chars
    }
}
