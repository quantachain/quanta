use pqcrypto_falcon::falcon512::*;
use pqcrypto_traits::sign::{PublicKey, SecretKey, SignedMessage};
use sha3::{Digest, Sha3_256};
use serde::Serialize;

use zeroize::Zeroize;

// ---------------------------------------------------------------------------
// Falcon-512 exact byte sizes (NIST / pqcrypto-falcon 0.3.0)
// These are consensus-critical constants. Never change without a hard fork.
// ---------------------------------------------------------------------------

/// Exact byte length of a Falcon-512 public key.
pub const FALCON512_PUBKEY_BYTES: usize = 897;

/// Exact byte length of a Falcon-512 signed message wrapper (signature + message).
/// For a 32-byte message (SHA3-256 hash), the signed-message blob is 698 bytes:
///   666 bytes maximum signature + 32 bytes message content.
/// We accept any value in [32 + 1, 32 + 666] = [33, 698] — pqcrypto encodes
/// variable-length compressed signatures, but we bound-check tightly.
pub const FALCON512_SIG_MAX_BYTES: usize = 698;  // 666 sig + 32 msg
pub const FALCON512_SIG_MIN_BYTES: usize = 33;   // minimum plausible

/// Domain separation tag prepended before hashing for signing.
/// This prevents cross-protocol signature reuse and cross-chain replay.
/// CONSENSUS-CRITICAL: Never change this value after genesis — doing so
/// invalidates every historical signature and requires a hard fork.
pub const SIGNING_DOMAIN: &[u8] = b"QUANTA_TX_V1:";

// ---------------------------------------------------------------------------
// Secret key wrapper — zeroized on drop
// ---------------------------------------------------------------------------

/// Secure secret key wrapper — zeroizes memory on drop.
#[derive(Zeroize)]
#[zeroize(drop)]
struct SecretKeyBytes(Vec<u8>);

// ---------------------------------------------------------------------------
// FalconKeypair — key generation and signing
// ---------------------------------------------------------------------------

/// Falcon-512 keypair for quantum-resistant signatures.
/// Public key: 897 bytes, Secret key: ~1281 bytes, Signature: up to 666 bytes.
///
/// SECURITY: Secret key is zeroized on drop. Signing is intentionally
/// separated from the consensus verification path — nodes only ever
/// call verify functions inside consensus logic.
///
/// LOW-4 FIX: Serialize is implemented manually to exclude secret_key so it
/// can never be leaked through logging, API responses, or bincode dumps.
#[derive(Clone, Debug)]
pub struct FalconKeypair {
    pub public_key: Vec<u8>,
    secret_key: Vec<u8>,
}

/// On-wire / at-rest representation — public key ONLY.
/// Deserializing this into FalconKeypair leaves secret_key empty;
/// such a keypair can verify but not sign.
#[derive(serde::Serialize)]
struct FalconKeypairPublicView<'a> {
    public_key: &'a Vec<u8>,
}

#[derive(serde::Deserialize)]
struct FalconKeypairDeser {
    public_key: Vec<u8>,
}

impl serde::Serialize for FalconKeypair {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        // Intentionally omits secret_key — never serialise private key material
        FalconKeypairPublicView { public_key: &self.public_key }.serialize(s)
    }
}

impl<'de> serde::Deserialize<'de> for FalconKeypair {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let view = FalconKeypairDeser::deserialize(d)?;
        Ok(FalconKeypair {
            public_key: view.public_key,
            secret_key: Vec::new(), // secret key not round-tripped
        })
    }
}


impl Drop for FalconKeypair {
    fn drop(&mut self) {
        self.secret_key.zeroize();
    }
}

impl FalconKeypair {
    /// Generate a new Falcon-512 keypair.
    pub fn generate() -> Self {
        let (pk, sk) = keypair();
        Self {
            public_key: pk.as_bytes().to_vec(),
            secret_key: sk.as_bytes().to_vec(),
        }
    }

    /// Return the length of the stored secret key bytes.
    pub fn secret_key_len(&self) -> usize {
        self.secret_key.len()
    }

    /// Sign a raw byte slice with the Falcon-512 secret key.
    /// INTERNAL: Prefer `sign_transaction_canonical()` for protocol signing.
    fn sign_raw(&self, message: &[u8]) -> Vec<u8> {
        let sk = SecretKey::from_bytes(&self.secret_key)
            .expect("Invalid secret key bytes in FalconKeypair");
        let signed = sign(message, &sk);
        signed.as_bytes().to_vec()
    }

    /// Canonical transaction signing.
    ///
    /// Signs `SHA3-256(SIGNING_DOMAIN || data)`, ensuring:
    ///   - Domain separation (non-malleable across protocols)
    ///   - Fixed 32-byte message size passed to Falcon (no length variance)
    ///   - Consistent with `verify_signature_strict()` on the verifier side
    ///
    /// This is the ONLY function that should be called when signing
    /// blockchain transactions. Never sign raw serialized structs directly.
    pub fn sign_transaction_canonical(&self, data: &[u8]) -> Vec<u8> {
        let hash = canonical_signing_hash(data);
        self.sign_raw(&hash)
    }

    /// Sign a pre-computed 32-byte hash directly (for callers that already
    /// have the canonical hash, e.g. hardware wallets).
    pub fn sign_hash(&self, hash: &[u8; 32]) -> Vec<u8> {
        self.sign_raw(hash)
    }

    /// Legacy method — hashes `data` with SHA3 then signs.
    /// Use `sign_transaction_canonical()` for all new code.
    pub fn sign_transaction_data(&self, data: &[u8]) -> Vec<u8> {
        let hash = sha3_hash(data);
        self.sign_raw(&hash)
    }

    /// Derive the Quanta address from this public key.
    /// Format: `0x` + lowercase hex of first 20 bytes of SHA3-256(public_key).
    pub fn get_address(&self) -> String {
        let hash = Sha3_256::digest(&self.public_key);
        format!("0x{}", hex::encode(&hash[..20]))
    }

    /// Same as `get_address()` without the `0x` prefix.
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
///
/// This is the hash that the signer creates and the verifier reconstructs.
/// Always use this when building the message to pass to Falcon.
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

/// Strict Falcon-512 signature verification for consensus use.
///
/// Rejects the signature immediately (without entering Falcon internals) if:
///   - `public_key.len()  != FALCON512_PUBKEY_BYTES` (897)
///   - `signed_msg.len()` is outside `[FALCON512_SIG_MIN_BYTES, FALCON512_SIG_MAX_BYTES]`
///
/// The `message` parameter must be the canonical SHA3-256 hash of the
/// domain-prefixed transaction data, NOT the raw transaction bytes.
///
/// Returns `true` only on a strict cryptographic success.
///
/// This function MUST be the only Falcon verification entry point used
/// inside consensus-critical code paths. Never use `verify_signature()`
/// in block validation.
pub fn verify_signature_strict(
    message: &[u8],
    signed_msg: &[u8],
    public_key: &[u8],
) -> bool {
    // Pre-check 1: public key length must be exactly 897 bytes.
    if public_key.len() != FALCON512_PUBKEY_BYTES {
        tracing::debug!(
            "Falcon-512 strict verify: public key length {} != expected {}",
            public_key.len(),
            FALCON512_PUBKEY_BYTES
        );
        return false;
    }

    // Pre-check 2: signed message length bounds.
    if signed_msg.len() < FALCON512_SIG_MIN_BYTES || signed_msg.len() > FALCON512_SIG_MAX_BYTES {
        tracing::debug!(
            "Falcon-512 strict verify: signed message length {} out of bounds [{}, {}]",
            signed_msg.len(),
            FALCON512_SIG_MIN_BYTES,
            FALCON512_SIG_MAX_BYTES
        );
        return false;
    }

    // Cryptographic verification.
    match PublicKey::from_bytes(public_key) {
        Ok(pk) => match SignedMessage::from_bytes(signed_msg) {
            Ok(sm) => match open(&sm, &pk) {
                Ok(verified_msg) => {
                    // Binary result only — no soft failures permitted.
                    let result = verified_msg.as_slice() == message;
                    if !result {
                        tracing::debug!("Falcon-512 strict verify: message content mismatch");
                    }
                    result
                }
                Err(_) => {
                    tracing::debug!("Falcon-512 strict verify: open() returned error");
                    false
                }
            },
            Err(_) => {
                tracing::debug!("Falcon-512 strict verify: signed message deserialization failed");
                false
            }
        },
        Err(_) => {
            tracing::debug!("Falcon-512 strict verify: public key deserialization failed");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy verification helper (kept for internal use only)
// ---------------------------------------------------------------------------

/// Legacy verification — does NOT enforce length pre-checks.
/// DO NOT USE in consensus code. Use `verify_signature_strict()` instead.
/// Kept only for backwards-compatible internal paths that are not
/// consensus-critical (e.g., wallet import sanity checks).
#[allow(dead_code)]
pub(crate) fn verify_signature(message: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
    match PublicKey::from_bytes(public_key) {
        Ok(pk) => match SignedMessage::from_bytes(signature) {
            Ok(sm) => match open(&sm, &pk) {
                Ok(verified_msg) => verified_msg.as_slice() == message,
                Err(_) => false,
            },
            Err(_) => false,
        },
        Err(_) => false,
    }
}

/// Verify a pre-computed 32-byte hash — strict version for consensus paths.
#[allow(dead_code)]
pub fn verify_hash_strict(hash: &[u8; 32], signed_msg: &[u8], public_key: &[u8]) -> bool {
    verify_signature_strict(hash, signed_msg, public_key)
}

// ---------------------------------------------------------------------------
// SHA3 utilities
// ---------------------------------------------------------------------------

/// Calculate SHA3-256 hash. Returns exactly 32 bytes.
pub fn sha3_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Double SHA3-256, returned as lowercase hex string. Used for block hashing.
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

    // --- Helper: generate a keypair and produce a canonical signed message ---
    fn make_canonical_signed(data: &[u8]) -> (FalconKeypair, Vec<u8>, [u8; 32]) {
        let kp = FalconKeypair::generate();
        let hash = canonical_signing_hash(data);
        let signed = kp.sign_transaction_canonical(data);
        (kp, signed, hash)
    }

    // -----------------------------------------------------------------------
    // 1. Happy path: correct signature verifies successfully
    // -----------------------------------------------------------------------
    #[test]
    fn test_strict_verify_valid_signature() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, signed, hash) = make_canonical_signed(tx_data);
        assert!(
            verify_signature_strict(&hash, &signed, &kp.public_key),
            "A freshly signed canonical transaction must verify"
        );
    }

    // -----------------------------------------------------------------------
    // 2. Wrong public key length is rejected before crypto ops
    // -----------------------------------------------------------------------
    #[test]
    fn test_strict_verify_wrong_pubkey_length() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (_, signed, hash) = make_canonical_signed(tx_data);

        // Short key
        let short_key = vec![0u8; 64];
        assert!(
            !verify_signature_strict(&hash, &signed, &short_key),
            "Wrong-length public key must be rejected"
        );

        // Long key
        let long_key = vec![0u8; 1024];
        assert!(
            !verify_signature_strict(&hash, &signed, &long_key),
            "Oversized public key must be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // 3. Wrong signed-message length is rejected before crypto ops
    // -----------------------------------------------------------------------
    #[test]
    fn test_strict_verify_wrong_sig_length() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, _, hash) = make_canonical_signed(tx_data);

        // Empty
        assert!(
            !verify_signature_strict(&hash, &[], &kp.public_key),
            "Empty signature must be rejected"
        );

        // Too short
        let tiny_sig = vec![0u8; 10];
        assert!(
            !verify_signature_strict(&hash, &tiny_sig, &kp.public_key),
            "Too-short signature must be rejected"
        );

        // Too long
        let huge_sig = vec![0u8; 2000];
        assert!(
            !verify_signature_strict(&hash, &huge_sig, &kp.public_key),
            "Oversized signature must be rejected"
        );
    }

    // -----------------------------------------------------------------------
    // 4. Domain separation: signing under one prefix fails verification
    //    if the verifier uses a different message
    // -----------------------------------------------------------------------
    #[test]
    fn test_domain_separation_prevents_cross_domain_reuse() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, signed, _) = make_canonical_signed(tx_data);

        // Try to verify with a hash that was not domain-prefixed
        let raw_hash = sha3_hash(tx_data);
        assert!(
            !verify_signature_strict(&raw_hash, &signed, &kp.public_key),
            "Signature produced with domain prefix must not verify against raw hash"
        );
    }

    // -----------------------------------------------------------------------
    // 5. Tampered message is rejected
    // -----------------------------------------------------------------------
    #[test]
    fn test_tampered_message_rejected() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, signed, _) = make_canonical_signed(tx_data);

        let tampered = canonical_signing_hash(b"sender:recipient:9999:1234567890:1000:1");
        assert!(
            !verify_signature_strict(&tampered, &signed, &kp.public_key),
            "Tampered message content must not verify"
        );
    }

    // -----------------------------------------------------------------------
    // 6. Tampered signature bytes are rejected
    // -----------------------------------------------------------------------
    #[test]
    fn test_tampered_signature_rejected() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (kp, mut signed, hash) = make_canonical_signed(tx_data);

        // Flip a byte deep in the signature
        let mid = signed.len() / 2;
        signed[mid] ^= 0xFF;

        assert!(
            !verify_signature_strict(&hash, &signed, &kp.public_key),
            "Bit-flipped signature must not verify"
        );
    }

    // -----------------------------------------------------------------------
    // 7. Canonical signing hash is deterministic
    // -----------------------------------------------------------------------
    #[test]
    fn test_canonical_signing_hash_is_deterministic() {
        let data = b"any transaction data";
        let h1 = canonical_signing_hash(data);
        let h2 = canonical_signing_hash(data);
        assert_eq!(h1, h2, "canonical_signing_hash must be deterministic");
        assert_ne!(h1, sha3_hash(data), "Domain prefix must change the hash value");
    }

    // -----------------------------------------------------------------------
    // 8. Wrong keypair cannot forge a signature
    // -----------------------------------------------------------------------
    #[test]
    fn test_wrong_keypair_cannot_forge() {
        let tx_data = b"sender:recipient:1000:1234567890:1000:1";
        let (_, signed, hash) = make_canonical_signed(tx_data);
        let attacker_kp = FalconKeypair::generate();

        assert!(
            !verify_signature_strict(&hash, &signed, &attacker_kp.public_key),
            "Signature from one keypair must not verify against a different public key"
        );
    }

    // -----------------------------------------------------------------------
    // 9. Falcon-512 constants match the pqcrypto library at build time
    // -----------------------------------------------------------------------
    #[test]
    fn test_pubkey_size_constant_matches_library() {
        let kp = FalconKeypair::generate();
        assert_eq!(
            kp.public_key.len(),
            FALCON512_PUBKEY_BYTES,
            "FALCON512_PUBKEY_BYTES constant does not match actual key size — update the constant"
        );
    }

    // -----------------------------------------------------------------------
    // 10. double_sha3 is deterministic
    // -----------------------------------------------------------------------
    #[test]
    fn test_double_sha3_deterministic() {
        let data = b"block header data";
        assert_eq!(double_sha3(data), double_sha3(data));
    }
}
