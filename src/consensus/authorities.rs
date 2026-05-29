/// Quanta v2 — BFT Authority Helpers
///
/// Thin wrappers over `AccountState` that the BFT engine uses to build
/// the per-epoch committee and resolve Falcon-512 public keys for
/// signature verification.
///
/// Validators register via `Stake` transactions; the committee for each
/// epoch is derived deterministically from the on-chain state — no static
/// key list is needed.

use crate::core::transaction::AccountState;
use falcon_rust::falcon512::PublicKey;

/// Maximum active validators per epoch committee.
/// Testnet QUA7: 7 (permissioned, known validators — CONFIRMED)
/// Mainnet genesis: 21 (Cosmos/Tendermint standard, more decentralized — CONFIRMED)
pub const MAX_COMMITTEE_SIZE: usize = 7; // Change to 21 at mainnet genesis

/// Epochs a deregistered validator must wait before staked QUA is returned.
/// v3: 60 epochs = 60,000 blocks ≈ 4.2 days at 6s/block (SLOT_SECONDS).
/// Provides real economic commitment without locking validators in indefinitely.
pub const UNBONDING_EPOCHS: u64 = 60;

/// Number of blocks per epoch.
pub const EPOCH_SIZE: u64 = 1000;

/// Return the epoch number for a given block height.
#[inline]
pub fn epoch_for_height(height: u64) -> u64 {
    height / EPOCH_SIZE
}

/// Return the first block height of an epoch.
#[inline]
pub fn epoch_start(epoch: u64) -> u64 {
    epoch * EPOCH_SIZE
}

/// Compute the deterministic proposer address for a given slot within an epoch.
///
/// Rotation: `slot = height - epoch_start(epoch)`, then round-robin over
/// the sorted committee.  Consistent across all nodes because the committee
/// list is sorted by address (secondary key after stake).
pub fn get_proposer(epoch: u64, height: u64, committee: &[String]) -> Option<String> {
    if committee.is_empty() {
        return None;
    }
    let epoch_start = epoch_start(epoch);
    let slot = (height - epoch_start) as usize;
    Some(committee[slot % committee.len()].clone())
}

/// Compute the epoch committee from the current `AccountState`.
///
/// Returns a sorted list of up to `MAX_COMMITTEE_SIZE` validator addresses.
/// The list is deterministic: top validators by stake, tie-broken by address.
pub fn compute_committee(state: &AccountState) -> Vec<String> {
    state.compute_epoch_committee(MAX_COMMITTEE_SIZE)
}

/// Resolve the Falcon-512 public keys for a list of committee addresses.
///
/// Addresses whose validator record is missing or whose key is malformed
/// are silently skipped. The returned vec is parallel to `committee`
/// (same index = same validator).
pub fn resolve_committee_keys(
    committee: &[String],
    state: &AccountState,
) -> Vec<(String, PublicKey)> {
    committee
        .iter()
        .filter_map(|addr| {
            let info = state.get_validator_info(addr)?;
            let pk = PublicKey::from_bytes(&info.falcon_pk).ok()?;
            Some((addr.clone(), pk))
        })
        .collect()
}
