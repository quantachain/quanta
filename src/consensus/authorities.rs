use falcon_rust::PublicKey;
use std::sync::OnceLock;

/// The initial set of 21 Authority Nodes for the Quanta 2.0 Merge.
/// 
/// These public keys are hardcoded in the consensus layer to prevent 
/// authority-takeover attacks during the bootstrap phase.
pub const AUTHORITY_PUBKEYS: &[&str] = &[
    "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20", // Placeholder 1
    "2122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f40", // Placeholder 2
    // ... we would list all 21 here
];

static LOADED_AUTHORITIES: OnceLock<Vec<PublicKey>> = OnceLock::new();

/// Load and cache the Falcon-512 public keys for all consensus validators.
pub fn get_authority_pks() -> &'static Vec<PublicKey> {
    LOADED_AUTHORITIES.get_or_init(|| {
        AUTHORITY_PUBKEYS.iter()
            .map(|hex| {
                // In production, we'd hex-decode and use PublicKey::from_bytes
                // For now, returning a mock key or handling the empty set
                // (This needs real 897-byte Falcon public keys)
                PublicKey::from_bytes(&vec![0u8; 897]).unwrap()
            })
            .collect()
    })
}
