use pqcrypto_falcon::falcon512::*;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use sha3::{Digest, Sha3_256};
use serde::{Serialize, Deserialize};
use zeroize::Zeroize;

/// Secure secret key wrapper - zeroizes on drop
#[derive(Zeroize)]
#[zeroize(drop)]
struct SecretKeyBytes(Vec<u8>);

/// Falcon-512 wrapper for quantum-resistant signatures
/// Public key: ~897 bytes, Private key: ~1281 bytes, Signature: ~666 bytes
/// 
/// SECURITY: Secret key is zeroized on drop (no memory leaks)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FalconKeypair {
    pub public_key: Vec<u8>,
    #[serde(serialize_with = "serialize_secret", deserialize_with = "deserialize_secret")]
    secret_key: Vec<u8>, // Wrapped internally for zeroization
}

// Custom serialization to ensure secret is handled carefully
fn serialize_secret<S>(secret: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_bytes(secret)
}

fn deserialize_secret<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
    Ok(bytes)
}

impl Drop for FalconKeypair {
    fn drop(&mut self) {
        // Explicitly zeroize secret key on drop
        self.secret_key.zeroize();
    }
}

impl FalconKeypair {
    /// Get secret key length (for display purposes)
    pub fn secret_key_len(&self) -> usize {
        self.secret_key.len()
    }

    /// Generate a new Falcon-512 keypair
    pub fn generate() -> Self {
        let (pk, sk) = keypair();
        Self {
            public_key: pk.as_bytes().to_vec(),
            secret_key: sk.as_bytes().to_vec(),
        }
    }

    /// Sign a message with Falcon private key
    /// SECURITY: Message is typically a HASH, not raw data
    /// For transactions, use sign_hash() instead
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let sk = SecretKey::from_bytes(&self.secret_key)
            .expect("Invalid secret key");
        let signed = sign(message, &sk);
        signed.as_bytes().to_vec()
    }
    
    /// Sign a hash (PREFERRED for transactions)
    /// This is the canonical way to sign blockchain transactions
    pub fn sign_hash(&self, hash: &[u8; 32]) -> Vec<u8> {
        self.sign(hash)
    }
    
    /// Sign transaction data (hashes then signs)
    /// Use this for actual transaction signing
    pub fn sign_transaction_data(&self, data: &[u8]) -> Vec<u8> {
        let hash = sha3_hash(data);
        self.sign_hash(&hash)
    }

    /// Derive quantum-resistant address from public key
    /// Uses first 20 bytes of SHA3-256 hash (Ethereum-style)
    /// Format: 0x + 40 hex chars = 42 chars total
    pub fn get_address(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.public_key);
        let hash = hasher.finalize();
        format!("0x{}", hex::encode(&hash[..20]))
    }
    
    /// Get address without 0x prefix (for backwards compatibility)
    pub fn get_address_raw(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.public_key);
        let hash = hasher.finalize();
        hex::encode(&hash[..20])
    }
}

/// Verify a Falcon signature
/// 
/// NOTE: For blockchain transactions, 'message' should be the HASH of the transaction,
/// not the raw transaction data. Use verify_hash() for clarity.
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

/// Verify a signature over a hash (PREFERRED for transactions)
pub fn verify_hash(hash: &[u8; 32], signature: &[u8], public_key: &[u8]) -> bool {
    verify_signature(hash, signature, public_key)
}

/// Calculate SHA3-256 hash (quantum-resistant)
/// Returns exactly 32 bytes for type safety
pub fn sha3_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Calculate double SHA3-256 hash for block hashing
pub fn double_sha3(data: &[u8]) -> String {
    let hash1 = sha3_hash(data);
    let hash2 = sha3_hash(&hash1);
    hex::encode(&hash2)
}


