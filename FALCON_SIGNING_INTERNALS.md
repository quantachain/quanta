# Falcon-512 Signing Libraries: pqcrypto-falcon vs falcon-rust

**Document Type:** Internal Developer Reference
**Relevant Files:** `src/crypto/signatures.rs`, `src/bin/distribute_faucet.rs`, `src/benchmark/network_bench.rs`, `quanta-wasm/src/lib.rs`

---

## Why Two Libraries Exist in This Codebase

QuantaChain uses two separate Rust crates that both implement Falcon-512, a NIST PQC Round 3 finalist lattice-based signature scheme. They coexist because each solves a different compilation and deployment constraint.

### pqcrypto-falcon

- **What it is:** Rust FFI bindings over the official NIST reference C implementation of Falcon-512.
- **How it works:** Compiles the C reference code and calls it from Rust via `unsafe` FFI. Produces production-grade, standards-compliant Falcon-512 key generation and signing.
- **Why it was chosen first:** The C reference implementation is the authoritative Falcon implementation. It is the basis of the NIST PQC submission and is trusted for correctness.
- **The limitation:** Because it compiles C code, it cannot be compiled to WebAssembly (WASM). The WASM target requires pure Rust. Any `cc` or `cmake` dependency in the build graph causes WASM compilation to fail.

### falcon-rust

- **What it is:** A pure Rust implementation of Falcon-512, with no C dependencies.
- **How it works:** Implements the full Falcon-512 specification in native Rust. Compiles cleanly to WASM, native Linux, macOS, and Windows targets without any C toolchain.
- **Why it was added:** The WASM wallet (`quanta-wasm`) must run in a browser. It cannot use `pqcrypto-falcon` because the WASM build fails when any C FFI crate is in the dependency graph. `falcon-rust` was chosen as the WASM-compatible alternative.
- **Critical consequence:** `falcon-rust` produces signatures in a different byte encoding than `pqcrypto-falcon`, even though both implement the same Falcon-512 cryptographic algorithm.

---

## The Signature Format Difference

This is the most important thing to understand. Both libraries are correct implementations of Falcon-512. However, their output byte representations are not identical.

`pqcrypto-falcon` wraps the C reference implementation. When you call `falcon512::sign(message, sk)`, it returns a `SignedMessage` struct whose bytes are laid out as:

```
pqcrypto output: [ signature_bytes ] [ message_bytes ]
```

The `SignedMessage::as_bytes()` call returns the concatenation of the signature and the original message.

`falcon-rust` exposes a lower-level interface. When you call `fr::sign(hash, sk)`, it returns a `Signature` struct, and `sig.to_bytes()` returns only the raw signature bytes with no appended message.

The QuantaChain node's canonical verification function (`verify_signature_strict` in `src/crypto/signatures.rs`) was designed around the format that `falcon-rust` produces. Its expected blob format is:

```
verify_signature_strict expects: [ raw_sig_bytes ] [ 32-byte hash ]
```

Where the last 32 bytes are the SHA3-256 domain-separated hash that was signed, appended manually by the caller. This format was chosen because it is what the WASM wallet produces, and the WASM wallet is the primary transaction signing path.

---

## Which Library Is Used Where

| Location | Library | Reason |
|---|---|---|
| `src/crypto/signatures.rs` (key generation, `FalconKeypair::generate`) | pqcrypto-falcon | Authoritative key generation on native node |
| `src/crypto/signatures.rs` (`sign_raw`, `sign_transaction_canonical`) | pqcrypto-falcon | Existing signing path for native tools |
| `src/crypto/signatures.rs` (`verify_signature_strict`) | falcon-rust | Verifies the format produced by the WASM wallet |
| `quanta-wasm/src/lib.rs` (browser wallet) | falcon-rust | WASM compilation requires pure Rust |
| `src/bin/distribute_faucet.rs` | falcon-rust | Must produce signatures the node accepts |
| `src/benchmark/network_bench.rs` | falcon-rust | Must produce signatures the node accepts |

---

## The Compatibility Problem and Why It Matters

`FalconKeypair::sign_transaction_canonical()` (in `signatures.rs`) uses `pqcrypto-falcon` internally. The signature blob it produces does not match what `verify_signature_strict()` expects, because `verify_signature_strict()` was written to accept the `falcon-rust` format used by the WASM wallet.

This means: **any code that signs with `FalconKeypair::sign_transaction_canonical()` and then submits to the live node will get `invalid_signature` rejections**, even though the local `tx.verify()` call also uses `verify_signature_strict()` and will also return false.

The only correct way to sign a transaction for submission to the live node is to use `falcon-rust` directly, following the pattern in `distribute_faucet.rs`:

```rust
use falcon_rust::falcon512::{self as fr, SecretKey as FrSK};
use sha3::{Sha3_256, Digest};

const SIGNING_DOMAIN: &[u8] = b"QUANTA_TX_V1:";

fn sign_for_node(sk_bytes: &[u8], signing_data: &[u8]) -> Vec<u8> {
    // 1. Domain-separated SHA3-256 hash
    let mut h = Sha3_256::new();
    h.update(SIGNING_DOMAIN);
    h.update(signing_data);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&h.finalize());

    // 2. Sign with falcon-rust
    let sk = FrSK::from_bytes(sk_bytes).expect("invalid SK");
    let sig = fr::sign(&hash, &sk);
    let sig_bytes = sig.to_bytes();

    // 3. Append hash to form the blob verify_signature_strict expects
    let mut blob = Vec::with_capacity(sig_bytes.len() + 32);
    blob.extend_from_slice(&sig_bytes);
    blob.extend_from_slice(&hash);
    blob
}
```

Then set `tx.signature = sign_for_node(&sk_bytes, &tx.get_signing_bytes())`.

---

## Key Compatibility: Public Keys Are Interoperable

While signatures are not directly interchangeable, **public keys are**. Both `pqcrypto-falcon` and `falcon-rust` use the standard Falcon-512 public key format: 897 bytes representing the polynomial coefficients in the ring used for verification. A key pair generated with `pqcrypto-falcon` can be verified against a signature produced by `falcon-rust`, and vice versa, as long as the signature blob is in the correct format.

This means:
- Key generation can use either library.
- The wallet or tool that signs determines which library must be used for verification.
- Because the node's `verify_signature_strict` is written for the `falcon-rust` format, all signing paths must use `falcon-rust` when targeting the live node.

---

## Current Status and Recommended Path Forward

The cleanest long-term resolution is to unify signing under `falcon-rust` across the entire codebase, since `falcon-rust` is both WASM-compatible and produces the format the node verifies. `pqcrypto-falcon` can then be kept only for key generation if needed, or removed entirely once `falcon-rust` key generation is validated to produce correctly sized and formatted key material.

Until that refactor is done:

- Use `FalconKeypair` for key management (generation, storage, address derivation).
- Use the `sign_for_node` pattern above (or the equivalent in `distribute_faucet.rs`) for any signing that must be verified by the live node.
- Do not use `FalconKeypair::sign_transaction_canonical` for transactions that will be submitted to the node. It produces pqcrypto format, which the node rejects.
