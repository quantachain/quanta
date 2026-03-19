//! quanta-wasm — Post-Quantum browser wallet for QuantaChain
//!
//! Compiled to WebAssembly with `wasm-pack build --target web`.
//! Exposes Falcon-512 key generation, signing, and address derivation
//! to JavaScript without any C FFI (pure Rust throughout).
//!
//! CONSENSUS CONTRACT: SIGNING_DOMAIN and address derivation must stay
//! identical to the chain's `src/crypto/signatures.rs` and `wallet.rs`.

use wasm_bindgen::prelude::*;
use sha3::{Sha3_256, Digest};
use hmac::{Hmac, Mac};
use bip39::{Mnemonic, Language};
use serde::{Serialize, Deserialize};
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha3_256>;

/// Domain separation tag — MUST match `SIGNING_DOMAIN` in signatures.rs.
const SIGNING_DOMAIN: &[u8] = b"QUANTA_TX_V1:";

// ---------------------------------------------------------------------------
// Panic hook — surfaces Rust panics in the browser console
// ---------------------------------------------------------------------------
#[wasm_bindgen(start)]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

// ---------------------------------------------------------------------------
// JS-friendly data structures
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct WalletInfo {
    pub mnemonic:   String,
    pub address:    String,
    pub public_key: String, // hex
    pub secret_key: String, // hex — caller must zeroize after storing
}

#[derive(Serialize, Deserialize)]
pub struct KeypairInfo {
    pub address:    String,
    pub public_key: String, // hex
    pub secret_key: String, // hex
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// SHA3-256 wrapper — returns 32 bytes.
fn sha3_256(data: &[u8]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&Sha3_256::digest(data));
    out
}

/// Canonical signing hash: `SHA3-256(SIGNING_DOMAIN || data)`.
/// Must match `canonical_signing_hash()` in the chain's signatures.rs.
fn canonical_signing_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(SIGNING_DOMAIN);
    hasher.update(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Derive a Quanta address from a raw Falcon-512 public key byte slice.
/// Formula: `"0x" + hex(SHA3-256(pubkey)[..20])`
fn address_from_pubkey(pubkey: &[u8]) -> String {
    let hash = sha3_256(pubkey);
    format!("0x{}", hex::encode(&hash[..20]))
}

/// HD account key: `HMAC-SHA3-256(master_key, index_be_bytes)`
fn derive_account_key(master_key: &[u8], index: u32) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(master_key)
        .expect("HMAC key init");
    mac.update(&index.to_be_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

/// HD master key: `HMAC-SHA3-256("Quanta HD Wallet Master Key", seed)`
fn derive_master_key(seed: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(b"Quanta HD Wallet Master Key")
        .expect("HMAC key init");
    mac.update(seed);
    let mut out = [0u8; 32];
    out.copy_from_slice(&mac.finalize().into_bytes());
    out
}

// ---------------------------------------------------------------------------
// falcon-rust 0.1.2 confirmed API (from source grep):
//   SecretKey::generate()                         -> SecretKey
//   SecretKey::generate_from_seed(seed: [u8;32])  -> SecretKey
//   SecretKey::from_bytes(&[u8])                  -> Result<SecretKey, _>
//   SecretKey::to_bytes(&self)                    -> Vec<u8>
//   PublicKey::from_secret_key(&SecretKey)        -> PublicKey  ← get pk from sk
//   PublicKey::from_bytes(&[u8])                  -> Result<PublicKey, _>
//   PublicKey::to_bytes(&self)                    -> Vec<u8>
//   Signature::from_bytes(&[u8])                  -> Result<Signature, _>
//   Signature::to_bytes(&self)                    -> Vec<u8>
//   keygen(seed: [u8;32])                         -> (SecretKey, PublicKey)  free fn
//   sign(m: &[u8], sk: &SecretKey)               -> Signature               free fn
//   verify(m: &[u8], sig: &Signature, pk: &PK)   -> bool                    free fn
// ---------------------------------------------------------------------------

use falcon_rust::falcon512::{SecretKey, PublicKey, Signature, sign as falcon_sign, verify as falcon_verify, keygen};

// ---------------------------------------------------------------------------
// Public WASM API
// ---------------------------------------------------------------------------

/// Generate a fresh random Falcon-512 keypair and a fresh BIP39 mnemonic.
///
/// Returns a JSON object: `{ mnemonic, address, public_key, secret_key }`
/// where `public_key` and `secret_key` are lowercase hex strings.
///
/// The caller is responsible for:
/// 1. Displaying `mnemonic` ONCE and asking the user to back it up.
/// 2. Encrypting `secret_key` before storing (use `encrypt_secret_key()`).
/// 3. Zeroizing `secret_key` from JS memory as soon as possible.
#[wasm_bindgen]
pub fn generate_wallet() -> Result<JsValue, JsValue> {
    // Generate BIP39 mnemonic (24 words, 256-bit entropy)
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Generate Falcon-512 keypair (random)
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let sk = SecretKey::generate();
    let pk = PublicKey::from_secret_key(&sk);

    let pk_bytes = pk.to_bytes();
    let sk_bytes = sk.to_bytes();
    let address  = address_from_pubkey(&pk_bytes);

    let info = WalletInfo {
        mnemonic:   mnemonic.to_string(),
        address,
        public_key: hex::encode(&pk_bytes),
        secret_key: hex::encode(&sk_bytes),
    };

    serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Import an existing wallet from a BIP39 mnemonic phrase.
///
/// Generates the `index`-th account keypair deterministically from the mnemonic.
/// Returns `{ address, public_key, secret_key }`.
///
/// IMPORTANT: `secret_key` must be encrypted before storing.
#[wasm_bindgen]
pub fn import_wallet(mnemonic_phrase: &str, passphrase: &str, index: u32) -> Result<JsValue, JsValue> {
    let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic_phrase)
        .map_err(|e| JsValue::from_str(&format!("Invalid mnemonic: {}", e)))?;

    let seed = mnemonic.to_seed(passphrase);
    let master = derive_master_key(&seed);
    let account_key = derive_account_key(&master, index);

    // Deterministic Falcon-512 keypair from HD account key.
    // keygen(seed) -> (SecretKey, PublicKey) — same seed always gives same keypair.
    let (sk, pk) = keygen(account_key);

    let pk_bytes = pk.to_bytes();
    let sk_bytes = sk.to_bytes();
    let address  = address_from_pubkey(&pk_bytes);

    let info = KeypairInfo {
        address,
        public_key: hex::encode(&pk_bytes),
        secret_key: hex::encode(&sk_bytes),
    };

    serde_wasm_bindgen::to_value(&info).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Sign transaction data with a Falcon-512 secret key.
///
/// `tx_data_hex`  — hex-encoded raw transaction bytes to sign.
/// `secret_key_hex` — hex-encoded Falcon-512 secret key.
///
/// Returns the hex-encoded signed-message blob (signature || message),
/// ready to be sent as `signature` in the transaction JSON.
///
/// The signing hash is `SHA3-256(SIGNING_DOMAIN || tx_data)`, matching
/// `sign_transaction_canonical()` in the chain's signatures.rs.
#[wasm_bindgen]
pub fn sign_transaction(tx_data_hex: &str, secret_key_hex: &str) -> Result<String, JsValue> {
    let tx_bytes = hex::decode(tx_data_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad tx_data hex: {}", e)))?;
    let mut sk_bytes = hex::decode(secret_key_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad secret_key hex: {}", e)))?;

    let sk = SecretKey::from_bytes(&sk_bytes)
        .map_err(|_| JsValue::from_str("Invalid Falcon-512 secret key"))?;
    sk_bytes.zeroize();

    // Compute canonical signing hash (matches chain consensus)
    let hash = canonical_signing_hash(&tx_bytes);

    // Sign with Falcon-512 via WASM — free function sign(message, &sk)
    let sig = falcon_sign(&hash, &sk);
    Ok(hex::encode(sig.to_bytes()))
}

/// Verify a Falcon-512 signature.
///
/// `hash_hex`      — hex of the 32-byte canonical signing hash.
/// `signature_hex` — hex of the signed-message blob.
/// `pubkey_hex`    — hex of the 897-byte Falcon-512 public key.
///
/// Returns `true` only on a strict cryptographic success.
#[wasm_bindgen]
pub fn verify_signature(hash_hex: &str, signature_hex: &str, pubkey_hex: &str) -> bool {
    let Ok(hash_bytes)  = hex::decode(hash_hex)      else { return false; };
    let Ok(sig_bytes)   = hex::decode(signature_hex) else { return false; };
    let Ok(pk_bytes)    = hex::decode(pubkey_hex)    else { return false; };

    let Ok(pk)  = PublicKey::from_bytes(&pk_bytes)  else { return false; };
    let Ok(sig) = Signature::from_bytes(&sig_bytes) else { return false; };

    // Use free function to avoid naming conflict with hmac::Mac::verify
    falcon_verify(&hash_bytes, &sig, &pk)
}

/// Derive a Quanta address from a hex-encoded Falcon-512 public key.
/// Formula: `"0x" + hex(SHA3-256(pubkey)[..20])`
#[wasm_bindgen]
pub fn get_address(pubkey_hex: &str) -> Result<String, JsValue> {
    let pk_bytes = hex::decode(pubkey_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad pubkey hex: {}", e)))?;
    Ok(address_from_pubkey(&pk_bytes))
}

/// Generate a fresh BIP39 24-word mnemonic (for display before key generation).
#[wasm_bindgen]
pub fn generate_mnemonic() -> Result<String, JsValue> {
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(mnemonic.to_string())
}

/// Validate a BIP39 mnemonic phrase. Returns `true` if valid.
#[wasm_bindgen]
pub fn validate_mnemonic(phrase: &str) -> bool {
    Mnemonic::parse_in_normalized(Language::English, phrase).is_ok()
}

/// Compute the canonical signing hash for a given transaction payload.
/// Useful for hardware wallet integrations.
/// Returns 32-byte hash as hex.
#[wasm_bindgen]
pub fn compute_signing_hash(tx_data_hex: &str) -> Result<String, JsValue> {
    let tx_bytes = hex::decode(tx_data_hex)
        .map_err(|e| JsValue::from_str(&format!("Bad hex: {}", e)))?;
    Ok(hex::encode(canonical_signing_hash(&tx_bytes)))
}
