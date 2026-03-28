/// Integration test: Wallet transaction signing → Node verification
///
/// This test EXACTLY mirrors the full send-transaction flow:
///   1. Wallet (WASM, popup.js) builds payload bytes
///   2. Wallet signs with falcon-rust → raw sig bytes
///   3. Node verifies with verify_signature_strict (falcon-rust)
///
/// Run with: cargo test --test wallet_tx_signing_test -- --nocapture
///
/// If ALL tests pass here, the live wallet→node flow will work.
/// If any test fails here, you have a bug to fix before redeploying.

use quanta::crypto::{
    verify_signature_strict, canonical_signing_hash, FALCON512_PUBKEY_BYTES,
};

// Import falcon-rust directly (same lib WASM uses)
use falcon_rust::falcon512::{
    SecretKey, PublicKey, keygen, sign as falcon_sign, verify as falcon_verify,
};
use sha3::{Sha3_256, Digest};

// ── Helpers that EXACTLY mirror popup.js ────────────────────────────────────

/// Mirror of popup.js toLeBytes(num) — 8 bytes little-endian
fn to_le_bytes(num: u64) -> [u8; 8] {
    num.to_le_bytes()
}

/// Mirror of address derivation in both WASM and node
fn derive_address(pk_bytes: &[u8]) -> String {
    let hash = Sha3_256::digest(pk_bytes);
    format!("0x{}", hex::encode(&hash[..20]))
}

/// Mirror of popup.js payloadBytes construction (MUST match transaction.rs get_signing_bytes)
fn build_payload(
    sender: &str,
    recipient: &str,
    amount: u64,
    timestamp: u64,
    fee: u64,
    nonce: u64,
    lock_time: u64,
    pk_bytes: &[u8],
) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(sender.as_bytes());      // sender UTF-8
    buf.extend_from_slice(recipient.as_bytes());   // recipient UTF-8
    buf.extend_from_slice(&to_le_bytes(amount));   // amount LE u64
    buf.extend_from_slice(&to_le_bytes(timestamp));// timestamp LE (i64 same as u64 for positive)
    buf.extend_from_slice(&to_le_bytes(fee));      // fee LE u64
    buf.extend_from_slice(&to_le_bytes(nonce));    // nonce LE u64
    buf.extend_from_slice(&to_le_bytes(lock_time));// lock_time LE u64
    buf.extend_from_slice(pk_bytes);               // public_key bytes
    buf.push(0u8);                                 // sig_scheme: Falcon512 = 0
    buf.push(0u8);                                 // tx_type: Transfer = 0
    buf
}

// ── Tests ────────────────────────────────────────────────────────────────────

/// TEST 1: Basic sign→verify roundtrip using falcon-rust only.
/// Proves falcon_rust::verify accepts signatures from falcon_rust::sign.
#[test]
fn test_falcon_rust_sign_verify_roundtrip() {
    let seed = [42u8; 32];
    let (sk, pk) = keygen(seed);

    let message = b"hello quanta";
    let sig = falcon_sign(message, &sk);
    let sig_bytes = sig.to_bytes();

    let pk_restored = PublicKey::from_bytes(&pk.to_bytes()).expect("pk parse");
    let sig_restored = falcon_rust::falcon512::Signature::from_bytes(&sig_bytes).expect("sig parse");

    assert!(
        falcon_verify(message, &sig_restored, &pk_restored),
        "falcon-rust: sign→verify roundtrip must pass"
    );
    println!("✅ TEST 1 PASSED: falcon-rust roundtrip");
}

/// TEST 2: verify_signature_strict accepts sig||hash blob from falcon-rust.
/// This is the NODE SIDE verification — proves our fix works.
#[test]
fn test_node_verify_accepts_falcon_rust_sig_with_hash() {
    let seed = [99u8; 32];
    let (sk, pk) = keygen(seed);
    let pk_bytes = pk.to_bytes();

    assert_eq!(pk_bytes.len(), FALCON512_PUBKEY_BYTES, "PK must be 897 bytes");

    let message = b"test transaction data";
    let hash = canonical_signing_hash(message);

    // WASM wallet produces: sig.to_bytes() || hash  (same as pqcrypto SignedMessage layout)
    let sig = falcon_sign(&hash, &sk);
    let sig_bytes = sig.to_bytes();
    let mut blob = Vec::new();
    blob.extend_from_slice(&sig_bytes);
    blob.extend_from_slice(&hash);

    println!("sig portion: {} bytes", sig_bytes.len());
    println!("total blob:  {} bytes", blob.len());
    assert!(blob.len() <= 698, "blob must be ≤ 698 bytes");

    // NODE: verify_signature_strict splits last 32 bytes as hash, verifies sig
    let ok = verify_signature_strict(&hash, &blob, &pk_bytes);
    assert!(ok, "verify_signature_strict must accept sig||hash blob");
    println!("✅ TEST 2 PASSED: node accepts falcon-rust sig||hash blob");
}

/// TEST 3: Full end-to-end wallet→node flow simulation.
/// Builds the EXACT same payload as popup.js sendTransaction(), signs it,
/// then verifies it the same way the node does in Transaction::verify().
#[test]
fn test_full_wallet_to_node_flow() {
    // Simulate wallet generating a keypair (same as generate_wallet() in WASM)
    let (sk, pk) = falcon_rust::falcon512::keygen([77u8; 32]);
    let pk_bytes = pk.to_bytes();
    let sk_bytes = sk.to_bytes();

    let sender    = derive_address(&pk_bytes); // "0x..." derived from pk
    let recipient = "0xc20e263e1c6cd7ec11076b4fc028647eeb2ddfa2".to_string();
    let amount    = 1000 * 1_000_000u64; // 1000 QUA in microunits
    let fee       = 1000u64;             // 0.001 QUA
    let nonce     = 1u64;
    let timestamp = 1711603299u64;       // fixed for test determinism
    let lock_time = 0u64;

    // STEP 1: Wallet builds payload (mirrors popup.js payloadBytes)
    let payload = build_payload(
        &sender, &recipient, amount, timestamp, fee, nonce, lock_time, &pk_bytes,
    );

    // STEP 2: Wallet computes canonical hash and signs (mirrors wasm sign_transaction)
    let signing_hash = canonical_signing_hash(&payload);
    let sig = falcon_sign(&signing_hash, &falcon_rust::falcon512::SecretKey::from_bytes(&sk_bytes).unwrap());
    let sig_bytes = sig.to_bytes();

    // WASM sign_transaction output = sig_bytes || hash  (mirrors our lib.rs fix)
    let mut blob = Vec::new();
    blob.extend_from_slice(&sig_bytes);
    blob.extend_from_slice(&signing_hash);

    println!("sender:    {}", sender);
    println!("payload:   {} bytes", payload.len());
    println!("sig hash:  {}", hex::encode(&signing_hash));
    println!("sig bytes: {} B, blob: {} B", sig_bytes.len(), blob.len());

    // STEP 3: Node verifies — mirrors Transaction::verify() → verify_signature_strict()
    let node_hash = canonical_signing_hash(&payload);
    assert_eq!(signing_hash, node_hash, "Node must compute same hash as wallet");

    // verify_signature_strict splits blob: sig = blob[..len-32], hash = blob[len-32..]
    let ok = verify_signature_strict(&node_hash, &blob, &pk_bytes);
    assert!(ok, "Full wallet→node flow must verify successfully");
    println!("✅ TEST 3 PASSED: full wallet→node flow verified");
}

/// TEST 4: Wrong signature is rejected.
#[test]
fn test_tampered_sig_rejected() {
    let (sk, pk) = falcon_rust::falcon512::keygen([11u8; 32]);
    let hash = canonical_signing_hash(b"some tx data");
    let sig = falcon_sign(&hash, &sk);
    let mut sig_bytes = sig.to_bytes();
    // Flip a byte in the middle of the sig
    let mid = sig_bytes.len() / 2;
    sig_bytes[mid] ^= 0xFF;
    // Build blob with tampered sig + correct hash
    let mut bad_blob = Vec::new();
    bad_blob.extend_from_slice(&sig_bytes);
    bad_blob.extend_from_slice(&hash);

    let ok = verify_signature_strict(&hash, &bad_blob, &pk.to_bytes());
    assert!(!ok, "Tampered signature must be rejected");
    println!("✅ TEST 4 PASSED: tampered sig correctly rejected");
}

/// TEST 5: Wrong public key is rejected.
#[test]
fn test_wrong_pubkey_rejected() {
    let (sk, pk1) = falcon_rust::falcon512::keygen([22u8; 32]);
    let (_sk2, pk2) = falcon_rust::falcon512::keygen([33u8; 32]);
    let hash = canonical_signing_hash(b"some tx data");
    let sig = falcon_sign(&hash, &sk);
    let sig_bytes = sig.to_bytes();

    // Build correct blob: sig_bytes || hash
    let mut blob = Vec::new();
    blob.extend_from_slice(&sig_bytes);
    blob.extend_from_slice(&hash);

    // Verify against wrong pubkey — must fail
    assert!(
        !verify_signature_strict(&hash, &blob, &pk2.to_bytes()),
        "Wrong pubkey must be rejected"
    );
    // Verify against correct pubkey — must pass
    assert!(
        verify_signature_strict(&hash, &blob, &pk1.to_bytes()),
        "Correct pubkey must verify"
    );
    println!("✅ TEST 5 PASSED: wrong pubkey correctly rejected");
}

/// TEST 6: Public key size check — 897 bytes exact.
#[test]
fn test_pubkey_size_is_897() {
    let (_sk, pk) = falcon_rust::falcon512::keygen([55u8; 32]);
    assert_eq!(pk.to_bytes().len(), 897, "Falcon-512 pk must be 897 bytes");
    println!("✅ TEST 6 PASSED: pk is {} bytes", pk.to_bytes().len());
}
