/// Quanta 2.0 BFT Authority Registry
///
/// During the PoW→BFT transition the authority set is entirely DYNAMIC —
/// validators register by submitting a `Stake` transaction on-chain and their
/// Falcon-512 public keys are stored in `AccountState::validators`.
///
/// This module provides helpers that load the live authority set from an
/// `AccountState` snapshot.  The old approach of hardcoding placeholder hex
/// strings is intentionally removed; no off-chain static keys are trusted.
///
/// Usage (inside `validate_block_consensus`):
/// ```rust
/// let authorities_map = base_state.get_validators();
/// let threshold = (authorities_map.len() * 2) / 3 + 1;
/// ```
/// That is already the approach taken in blockchain.rs; this module is kept
/// as a documentation anchor and may house helper types in the future.

use crate::core::transaction::AccountState;
use falcon_rust::falcon512::PublicKey;

/// Return all currently-registered Falcon-512 public keys from `state`.
///
/// The order is deterministic (sorted by validator address) so that every node
/// builds the same `validator_pks` vec when constructing a `FalconKeychain`.
pub fn get_authority_pks_from_state(state: &AccountState) -> Vec<PublicKey> {
    let mut entries: Vec<_> = state.get_validators().iter().collect();
    entries.sort_by_key(|(addr, _)| addr.as_str());

    entries
        .iter()
        .filter_map(|(_, pk_bytes)| {
            PublicKey::from_bytes(pk_bytes).ok()
        })
        .collect()
}

/// Return the addresses in the same sorted order as `get_authority_pks_from_state`.
/// Used to build the `FalconKeychain` index ↔ address mapping.
pub fn get_authority_addresses_sorted(state: &AccountState) -> Vec<String> {
    let mut entries: Vec<_> = state.get_validators().keys().cloned().collect();
    entries.sort();
    entries
}
