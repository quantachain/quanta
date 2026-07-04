// pqcrypto-falcon: C FFI bindings to the NIST reference Falcon-512 implementation.
// Used ONLY for key generation (keypair()) and secret key parsing — NOT for signing.
// Key generation stays here because the NIST reference C impl is the authoritative
// source for correctness. Public keys are byte-compatible with falcon-rust.
use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::{PublicKey as PqPublicKey, SecretKey as PqSecretKey};
use sha3::{Digest, Sha3_256};
use zeroize::Zeroize;

// falcon-rust: pure-Rust Falcon-512 — used for ALL signing and verification.
// WASM-compatible (no C FFI). Produces the canonical blob format:
//   raw_sig_bytes (≤666 B) || sha3_hash_bytes (32 B)
// which verify_signature_strict() and the browser WASM wallet both expect.
use falcon_rust::falcon512 as fr;
use falcon_rust::falcon512::{PublicKey as FrPublicKey, Signature as FrSignature};


// ---------------------------------------------------------------------------
// Falcon-512 exact byte sizes
// ---------------------------------------------------------------------------

/// Exact byte length of a Falcon-512 public key (same in both pqcrypto and falcon-rust).
pub const FALCON512_PUBKEY_BYTES: usize = 897;

/// Byte length of the signature field in a Transaction.
/// Format: raw_falcon_sig_bytes (variable, ≤666) || canonical_hash_bytes (32).
/// Max: 666 + 32 = 698. Min: at least 1 sig byte + 32 hash bytes = 33.
/// This format is identical to the original pqcrypto SignedMessage layout,
/// so transaction sizes are unchanged. Verification now uses falcon-rust
/// to avoid the C FFI incompatibility with the WASM wallet.
pub const FALCON512_SIG_MAX_BYTES: usize = 698; // 666 sig + 32 hash
pub const FALCON512_SIG_MIN_BYTES: usize = 33;  // at least 1 sig byte + 32 hash bytes

/// Domain separation tag — MUST match `SIGNING_DOMAIN` in quanta-wasm/src/lib.rs.
pub const SIGNING_DOMAIN: &[u8] = b"QUANTA_TX_V1:";

// ---------------------------------------------------------------------------
// Secret key wrapper — zeroized on drop
// ---------------------------------------------------------------------------

#[derive(Zeroize)]
#[zeroize(drop)]
#[allow(dead_code)] // Exists for its Zeroize drop guard; zeroes secret key bytes on drop.
struct SecretKeyBytes(Vec<u8>);

// ---------------------------------------------------------------------------
// FalconKeypair
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct FalconKeypair {
    pub public_key: Vec<u8>,
    secret_key: Vec<u8>,
}

impl Drop for FalconKeypair {
    fn drop(&mut self) {
        self.secret_key.zeroize();
    }
}

impl FalconKeypair {
    /// Generate a new Falcon-512 keypair.
    pub fn generate() -> Self {
        let (pk, sk) = falcon512::keypair();
        Self {
            public_key: pk.as_bytes().to_vec(),
            secret_key: sk.as_bytes().to_vec(),
        }
    }

    /// Generate a new Falcon-512 keypair deterministically from a 32-byte seed.
    /// This uses `falcon-rust` because `pqcrypto-falcon` does not expose a deterministic keygen.
    pub fn generate_from_seed(seed: [u8; 32]) -> Self {
        let (sk, pk) = fr::keygen(seed);
        Self {
            public_key: pk.to_bytes().to_vec(),
            secret_key: sk.to_bytes().to_vec(),
        }
    }

    pub fn secret_key_len(&self) -> usize {
        self.secret_key.len()
    }

    pub fn secret_key_bytes(&self) -> &[u8] {
        &self.secret_key
    }

    #[allow(dead_code)]
    pub fn from_secret_key_bytes(sk_bytes: &[u8], pk_bytes: &[u8]) -> Result<Self, String> {
        if pk_bytes.len() != FALCON512_PUBKEY_BYTES {
            return Err(format!(
                "Invalid Falcon-512 public key: {} bytes (expected {})",
                pk_bytes.len(), FALCON512_PUBKEY_BYTES
            ));
        }
        if sk_bytes.is_empty() || sk_bytes.len() > 2048 {
            return Err(format!(
                "Invalid Falcon-512 secret key length: {} bytes", sk_bytes.len()
            ));
        }
        Ok(Self {
            public_key: pk_bytes.to_vec(),
            secret_key: sk_bytes.to_vec(),
        })
    }

    /// Sign a message hash using falcon-rust (pure Rust, WASM-compatible format).
    ///
    /// SIGNING FORMAT CONTRACT (frozen forever):
    ///   output = raw_falcon512_sig_bytes (≤666 B) || sha3_hash_bytes (32 B)
    ///
    /// This is the ONLY format accepted by `verify_signature_strict()` and by
    /// the browser WASM wallet. Using falcon-rust here (instead of pqcrypto)
    /// ensures that ALL signing paths — native CLI, faucet, test harness, WASM —
    /// produce identical byte blobs without relying on pqcrypto's internal layout.
    ///
    /// Key generation still uses pqcrypto (authoritative NIST reference C impl).
    /// Public keys are byte-compatible between the two libraries (both use the
    /// standard 897-byte Falcon-512 polynomial encoding).
    fn sign_raw(&self, message: &[u8]) -> Vec<u8> {
        use falcon_rust::falcon512::{self as fr_sign, SecretKey as FrSK};

        // Reconstruct the falcon-rust SecretKey from stored bytes.
        // pqcrypto and falcon-rust use the same underlying key format,
        // so the bytes generated by pqcrypto::keypair() are valid here.
        let sk = FrSK::from_bytes(&self.secret_key)
            .expect("Invalid secret key bytes in FalconKeypair (falcon-rust)");

        // Sign the message (expected: a 32-byte canonical hash).
        let sig = fr_sign::sign(message, &sk);
        let sig_bytes = sig.to_bytes();

        // Build the canonical blob: raw_sig || message
        // verify_signature_strict() splits at len-32 to recover both.
        let mut blob = Vec::with_capacity(sig_bytes.len() + message.len());
        blob.extend_from_slice(&sig_bytes);
        blob.extend_from_slice(message);
        blob
    }

    /// Canonical transaction signing — the ONLY signing function for protocol use.
    ///
    /// Steps:
    ///   1. Compute `SHA3-256(SIGNING_DOMAIN || data)` → 32-byte hash
    ///   2. Sign the hash with falcon-rust → raw_sig_bytes
    ///   3. Return `raw_sig_bytes || hash` (the format `verify_signature_strict` expects)
    pub fn sign_transaction_canonical(&self, data: &[u8]) -> Vec<u8> {
        let hash = canonical_signing_hash(data);
        self.sign_raw(&hash)
    }

    pub fn sign_hash(&self, hash: &[u8; 32]) -> Vec<u8> {
        self.sign_raw(hash)
    }

    /// Legacy — kept for compatibility.
    #[allow(dead_code)]
    pub fn sign_transaction_data(&self, data: &[u8]) -> Vec<u8> {
        let hash = sha3_hash(data);
        self.sign_raw(&hash)
    }

    pub fn get_address(&self) -> String {
        let hash = Sha3_256::digest(&self.public_key);
        format!("0x{}", hex::encode(&hash[..20]))
    }

    #[allow(dead_code)]
    pub fn get_address_raw(&self) -> String {
        let hash = Sha3_256::digest(&self.public_key);
        hex::encode(&hash[..20])
    }
}

// ---------------------------------------------------------------------------
// Canonical hash helper
// ---------------------------------------------------------------------------

/// Compute `SHA3-256(SIGNING_DOMAIN || data)`.
pub fn canonical_signing_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(SIGNING_DOMAIN);
    hasher.update(data);
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

// ---------------------------------------------------------------------------
// verify_signature_strict — THE consensus verification function
// ---------------------------------------------------------------------------

/// Strict Falcon-512 signature verification using falcon-rust (raw signature format).
///
/// `message`    — the 32-byte canonical signing hash.
/// `signed_msg` — raw Falcon-512 signature bytes as produced by falcon-rust's sign().
///                The browser WASM wallet sends this format.
///                NOT a pqcrypto SignedMessage blob.
/// `public_key` — 897-byte Falcon-512 public key.
pub fn verify_signature_strict(
    message: &[u8],
    signed_msg: &[u8],
    public_key: &[u8],
) -> bool {
    // Pre-check 1: public key must be exactly 897 bytes.
    if public_key.len() != FALCON512_PUBKEY_BYTES {
        tracing::debug!("Falcon-512 verify: pubkey len {} != {}", public_key.len(), FALCON512_PUBKEY_BYTES);
        return false;
    }
    // Pre-check 2: signed_msg = sig_bytes || hash_bytes (32).
    // Must be at least 33 bytes (1 sig byte + 32 hash) and at most 698 (666 + 32).
    if signed_msg.len() < FALCON512_SIG_MIN_BYTES || signed_msg.len() > FALCON512_SIG_MAX_BYTES {
        tracing::debug!("Falcon-512 verify: blob len {} out of bounds [{}, {}]",
            signed_msg.len(), FALCON512_SIG_MIN_BYTES, FALCON512_SIG_MAX_BYTES);
        return false;
    }

    // Split: last 32 bytes = embedded hash, the rest = raw Falcon-512 sig.
    let (raw_sig, embedded_hash) = signed_msg.split_at(signed_msg.len() - 32);

    // Pre-check 3: embedded hash must match the expected message.
    // This catches any blob where the message portion was tampered.
    if embedded_hash != message {
        tracing::debug!("Falcon-512 verify: embedded hash mismatch");
        return false;
    }

    // Cryptographic verification using falcon-rust.
    // Both the WASM wallet and FalconKeypair::sign_raw produce:
    //   sig_bytes = pqcrypto::sign(hash).as_bytes()[..len-32]
    // which is the raw Falcon-512 compressed signature.
    let pk = match FrPublicKey::from_bytes(public_key) {
        Ok(pk) => pk,
        Err(_) => { tracing::debug!("Falcon-512 verify: pk parse failed"); return false; }
    };
    let sig = match FrSignature::from_bytes(raw_sig) {
        Ok(s) => s,
        Err(_) => { tracing::debug!("Falcon-512 verify: sig parse failed"); return false; }
    };

    let ok = fr::verify(message, &sig, &pk);
    if !ok { tracing::debug!("Falcon-512 verify: cryptographic check failed"); }
    ok
}

// ---------------------------------------------------------------------------
// Legacy verification (non-consensus paths only)
// ---------------------------------------------------------------------------

#[allow(dead_code)]
pub(crate) fn verify_signature(message: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
    verify_signature_strict(message, signature, public_key)
}

#[allow(dead_code)]
pub fn verify_hash_strict(hash: &[u8; 32], signed_msg: &[u8], public_key: &[u8]) -> bool {
    verify_signature_strict(hash, signed_msg, public_key)
}

// ---------------------------------------------------------------------------
// SHA3 utilities
// ---------------------------------------------------------------------------

pub fn sha3_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

pub fn double_sha3(data: &[u8]) -> String {
    let hash1 = sha3_hash(data);
    let hash2 = sha3_hash(&hash1);
    hex::encode(hash2)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_canonical_signed(data: &[u8]) -> (FalconKeypair, Vec<u8>, [u8; 32]) {
        let kp = FalconKeypair::generate();
        let hash = canonical_signing_hash(data);
        let signed = kp.sign_transaction_canonical(data);
        (kp, signed, hash)
    }

    #[test]
    fn test_strict_verify_valid_signature() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, signed, hash) = make_canonical_signed(tx_data);
        assert!(
            verify_signature_strict(&hash, &signed, &kp.public_key),
            "A freshly signed canonical transaction must verify"
        );
    }

    #[test]
    fn test_strict_verify_wrong_pubkey_length() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (_, signed, hash) = make_canonical_signed(tx_data);
        assert!(!verify_signature_strict(&hash, &signed, &vec![0u8; 64]));
        assert!(!verify_signature_strict(&hash, &signed, &vec![0u8; 1024]));
    }

    #[test]
    fn test_tampered_message_rejected() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, signed, _) = make_canonical_signed(tx_data);
        let tampered = canonical_signing_hash(b"sender:recipient:9999:1234567890:1000:1");
        assert!(!verify_signature_strict(&tampered, &signed, &kp.public_key));
    }

    #[test]
    fn test_wrong_keypair_cannot_forge() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (_, signed, hash) = make_canonical_signed(tx_data);
        let attacker_kp = FalconKeypair::generate();
        assert!(!verify_signature_strict(&hash, &signed, &attacker_kp.public_key));
    }

    #[test]
    fn test_canonical_signing_hash_is_deterministic() {
        let data = b"any transaction data";
        assert_eq!(canonical_signing_hash(data), canonical_signing_hash(data));
        assert_ne!(canonical_signing_hash(data), sha3_hash(data));
    }

    #[test]
    fn test_double_sha3_deterministic() {
        let data = b"block header data";
        assert_eq!(double_sha3(data), double_sha3(data));
    }
}
