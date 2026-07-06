#![allow(dead_code)]
use crate::core::transaction::Transaction;
use crate::crypto::signatures::{verify_signature_strict, FalconKeypair};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// Multi-signature transaction requiring M-of-N signatures
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultiSigTransaction {
    pub base_tx: Transaction,
    pub required_signatures: usize,       // M
    pub public_keys: Vec<Vec<u8>>,        // N public keys
    pub signatures: Vec<Option<Vec<u8>>>, // Collected signatures
}

impl MultiSigTransaction {
    /// Create a new multisig transaction (M-of-N)
    pub fn new(
        base_tx: Transaction,
        required_signatures: usize,
        public_keys: Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        if required_signatures == 0 {
            return Err("Required signatures must be > 0".to_string());
        }
        if required_signatures > public_keys.len() {
            return Err(format!(
                "Required signatures ({}) cannot exceed total keys ({})",
                required_signatures,
                public_keys.len()
            ));
        }
        let signatures = vec![None; public_keys.len()];
        Ok(Self {
            base_tx,
            required_signatures,
            public_keys,
            signatures,
        })
    }

    /// Add a signature from one of the signers (verifies immediately)
    pub fn add_signature(&mut self, index: usize, signature: Vec<u8>) -> Result<(), String> {
        if index >= self.public_keys.len() {
            return Err("Invalid signer index".to_string());
        }
        if self.signatures[index].is_some() {
            return Err("Signature already provided for this index".to_string());
        }
        let signing_hash = self.base_tx.get_signing_data();
        if !verify_signature_strict(&signing_hash, &signature, &self.public_keys[index]) {
            return Err("Invalid signature".to_string());
        }
        self.signatures[index] = Some(signature);
        Ok(())
    }

    /// Returns true if M valid signatures have been collected
    pub fn is_complete(&self) -> bool {
        self.signatures.iter().filter(|s| s.is_some()).count() >= self.required_signatures
    }

    /// Verify all collected signatures are valid and threshold is met
    pub fn verify(&self) -> bool {
        if !self.is_complete() {
            return false;
        }
        let signing_hash = self.base_tx.get_signing_data();
        let valid = self
            .signatures
            .iter()
            .enumerate()
            .filter_map(|(i, sig_opt)| sig_opt.as_ref().map(|sig| (i, sig)))
            .filter(|(i, sig)| verify_signature_strict(&signing_hash, sig, &self.public_keys[*i]))
            .count();
        valid >= self.required_signatures
    }

    /// Get the canonical multisig address for this keyset
    pub fn get_multisig_address(&self) -> String {
        multisig_address(
            &self.public_keys,
            self.required_signatures,
            self.public_keys.len(),
        )
    }

    /// Returns (collected, required) signature counts
    pub fn signature_progress(&self) -> (usize, usize) {
        let collected = self.signatures.iter().filter(|s| s.is_some()).count();
        (collected, self.required_signatures)
    }

    /// Serialize to JSON for offline signing ceremony
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from JSON file
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Invalid multisig JSON: {}", e))
    }
}

/// Compute the canonical multisig address for M-of-N keyset.
///
/// Address = "ms" + hex(SHA3-256(M_byte || N_byte || sorted_pk0 || ... || sorted_pkN)[..20])
///
/// Keys are sorted so the address is independent of key insertion order.
pub fn multisig_address(public_keys: &[Vec<u8>], required: usize, total: usize) -> String {
    let mut sorted_keys = public_keys.to_vec();
    sorted_keys.sort();

    let mut hasher = Sha3_256::new();
    hasher.update([required as u8, total as u8]);
    for pk in &sorted_keys {
        hasher.update(pk);
    }
    let hash = hasher.finalize();
    format!("ms{}", hex::encode(&hash[..20]))
}

// ---------------------------------------------------------------------------
// TreasuryMultisig — 2-of-3, all keys held by founder (pre-testnet)
//
// DEPRECATED: Use TreasuryMultisigV2 for all new setups.
// ---------------------------------------------------------------------------

/// Treasury setup for the pre-testnet phase.
///
/// **Deprecated** — use `TreasuryMultisigV2` instead, which supports a
/// configurable number of signers (3-of-N) for more decentralised governance.
///
/// Kept here to allow loading of existing treasury_setup.json files generated
/// before this upgrade. Do not create new instances.
#[deprecated(note = "Use TreasuryMultisigV2 for new treasury setups (3-of-N policy)")]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreasuryMultisig {
    /// The 3 Falcon-512 public keys (each stored in a separate wallet file)
    pub public_keys: Vec<Vec<u8>>,
    /// 2-of-3 threshold
    pub required: usize,
    /// The canonical 2-of-3 multisig address — use this as TREASURY_ADDRESS
    pub address: String,
}

#[allow(deprecated)]
impl TreasuryMultisig {
    /// Generate a new 2-of-3 treasury with 3 fresh Falcon-512 keypairs.
    ///
    /// Returns the setup (with public keys + address) and 3 private keypairs.
    /// Save each keypair to a separate encrypted wallet file immediately!
    pub fn generate() -> (Self, [FalconKeypair; 3]) {
        let k0 = FalconKeypair::generate();
        let k1 = FalconKeypair::generate();
        let k2 = FalconKeypair::generate();

        let public_keys = vec![
            k0.public_key.clone(),
            k1.public_key.clone(),
            k2.public_key.clone(),
        ];
        let address = multisig_address(&public_keys, 2, 3);
        let setup = TreasuryMultisig {
            public_keys,
            required: 2,
            address,
        };
        (setup, [k0, k1, k2])
    }

    /// Reconstruct from 3 known public keys (to verify address or re-display).
    pub fn from_public_keys(pk0: Vec<u8>, pk1: Vec<u8>, pk2: Vec<u8>) -> Self {
        let public_keys = vec![pk0, pk1, pk2];
        let address = multisig_address(&public_keys, 2, 3);
        TreasuryMultisig {
            public_keys,
            required: 2,
            address,
        }
    }

    /// Create a spending proposal (unsigned MultiSigTransaction) for the given transfer.
    /// Serializes to JSON for the offline signing ceremony.
    pub fn propose_spend(
        &self,
        to: String,
        amount_microunits: u64,
        fee: u64,
        nonce: u64,
        timestamp: i64,
    ) -> MultiSigTransaction {
        let base_tx = Transaction {
            sender: self.address.clone(),
            recipient: to,
            amount: amount_microunits,
            timestamp,
            signature: vec![],
            public_key: vec![], // Multisig — no single pubkey
            fee,
            nonce,
            lock_time: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            network_id: 0, // Default Testnet; caller should set to config.network_type.network_id()
            payload: vec![],
        };
        MultiSigTransaction {
            base_tx,
            required_signatures: self.required,
            public_keys: self.public_keys.clone(),
            signatures: vec![None; self.public_keys.len()],
        }
    }

    /// Sign a proposal with one of the 3 treasury keys.
    /// `key_index` is 0, 1, or 2 matching position in public_keys.
    pub fn sign_proposal(
        proposal: &mut MultiSigTransaction,
        key_index: usize,
        keypair: &FalconKeypair,
    ) -> Result<(), String> {
        let signing_bytes = proposal.base_tx.get_signing_bytes();
        let signature = keypair.sign_transaction_canonical(&signing_bytes);
        proposal.add_signature(key_index, signature)
    }

    /// Serialize setup to JSON (store alongside your wallet files).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Load setup from JSON.
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Invalid treasury JSON: {}", e))
    }
}

// ---------------------------------------------------------------------------
// TreasuryMultisigV2 — 3-of-N (configurable total signers ≥ 3)
//
// Replaces TreasuryMultisig for all new treasury setups.
// Threshold is fixed at 3 required signatures for strong security while
// allowing fault-tolerance with more total keyholders.
//
// # Recommended Configurations
//
//   3-of-5  (default):  2 keys can be lost/compromised with no risk
//   3-of-7          :  4 keys can be lost/compromised with no risk
//
// # Workflow
// ```
// # 1. One-time setup: generates N keypairs, prints treasury address
// quanta-wallet treasury-init --signers 5 --out treasury_setup.json
//
// # 2. Add treasury_address from the printout to quanta.toml:
// #    treasury_address = "ms..."
//
// # 3. Propose a spend
// quanta-wallet treasury-propose --setup treasury_setup.json \
//     --to 0xRecipient --amount 100 --nonce 1 --out proposal.json
//
// # 4. Sign with key 0
// quanta-wallet treasury-sign --proposal proposal.json \
//     --key treasury_key0.qua --index 0
//
// # 5. Sign with key 1
// quanta-wallet treasury-sign --proposal proposal.json \
//     --key treasury_key1.qua --index 1
//
// # 6. Sign with key 2  (3-of-N satisfied)
// quanta-wallet treasury-sign --proposal proposal.json \
//     --key treasury_key2.qua --index 2
//
// # 7. Broadcast
// quanta-wallet treasury-broadcast --proposal proposal.json \
//     --node http://localhost:3000
// ```
// ---------------------------------------------------------------------------

/// 3-of-N Falcon-512 treasury multisig.
///
/// `required` is always 3. `total_signers` (N) is set at generation time
/// and must satisfy N ≥ 3. Each of the N keyholders stores one `.qua`
/// wallet file; any 3 of them can authorize a spend.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreasuryMultisigV2 {
    /// N Falcon-512 public keys (one per keyholder)
    pub public_keys: Vec<Vec<u8>>,
    /// Always 3 — the minimum number of signatures required
    pub required: usize,
    /// Total number of keyholders (N ≥ 3)
    pub total_signers: usize,
    /// Canonical 3-of-N multisig address — set this as treasury_address in quanta.toml
    pub address: String,
}

impl TreasuryMultisigV2 {
    /// The fixed required-signature threshold for the V2 treasury policy.
    pub const REQUIRED: usize = 3;

    /// Generate a new 3-of-N treasury with `total` fresh Falcon-512 keypairs.
    ///
    /// # Panics
    /// Panics if `total < 3` (caller should validate first).
    ///
    /// # Returns
    /// `(setup, keypairs)` — the setup contains only public keys + the address.
    /// The keypairs hold the private keys; save each one to a **separate**
    /// encrypted wallet file before discarding this return value.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use quanta::crypto::TreasuryMultisigV2;
    /// let (setup, keypairs) = TreasuryMultisigV2::generate(5); // 3-of-5
    /// println!("Treasury address: {}", setup.address);
    /// ```
    pub fn generate(total: usize) -> (Self, Vec<FalconKeypair>) {
        assert!(
            total >= Self::REQUIRED,
            "total_signers must be >= {}",
            Self::REQUIRED
        );

        let keypairs: Vec<FalconKeypair> = (0..total).map(|_| FalconKeypair::generate()).collect();
        let public_keys: Vec<Vec<u8>> = keypairs.iter().map(|kp| kp.public_key.clone()).collect();
        let address = multisig_address(&public_keys, Self::REQUIRED, total);

        let setup = TreasuryMultisigV2 {
            public_keys,
            required: Self::REQUIRED,
            total_signers: total,
            address,
        };
        (setup, keypairs)
    }

    /// Reconstruct the setup from a known list of N public keys.
    ///
    /// Useful for re-deriving the address from backups or for auditing.
    /// Returns an error if fewer than `REQUIRED` keys are supplied.
    pub fn from_public_keys(public_keys: Vec<Vec<u8>>) -> Result<Self, String> {
        let total = public_keys.len();
        if total < Self::REQUIRED {
            return Err(format!(
                "Need at least {} public keys for a 3-of-N treasury (got {})",
                Self::REQUIRED,
                total
            ));
        }
        let address = multisig_address(&public_keys, Self::REQUIRED, total);
        Ok(TreasuryMultisigV2 {
            public_keys,
            required: Self::REQUIRED,
            total_signers: total,
            address,
        })
    }

    /// Create an unsigned spending proposal for the given transfer.
    ///
    /// The proposal is saved to a JSON file and then passed to
    /// `treasury-sign` (by any 3 of the N keyholders) before broadcasting.
    pub fn propose_spend(
        &self,
        to: String,
        amount_microunits: u64,
        fee: u64,
        nonce: u64,
        timestamp: i64,
    ) -> MultiSigTransaction {
        let base_tx = Transaction {
            sender: self.address.clone(),
            recipient: to,
            amount: amount_microunits,
            timestamp,
            signature: vec![],
            public_key: vec![], // Multisig — no single pubkey
            fee,
            nonce,
            lock_time: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            network_id: 0, // Default Testnet; caller should set to config.network_type.network_id()
            payload: vec![],
        };
        MultiSigTransaction {
            base_tx,
            required_signatures: self.required,
            public_keys: self.public_keys.clone(),
            signatures: vec![None; self.public_keys.len()],
        }
    }

    /// Sign a spend proposal with one of the N treasury keys.
    ///
    /// `key_index` is the 0-based position of the signer's public key in
    /// `self.public_keys`. Must match the keypair stored in their wallet file.
    pub fn sign_proposal(
        proposal: &mut MultiSigTransaction,
        key_index: usize,
        keypair: &FalconKeypair,
    ) -> Result<(), String> {
        let signing_bytes = proposal.base_tx.get_signing_bytes();
        let signature = keypair.sign_transaction_canonical(&signing_bytes);
        proposal.add_signature(key_index, signature)
    }

    /// Serialize this setup to pretty-printed JSON.
    /// Store this file alongside (but separate from) the key wallet files.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Deserialize from a JSON string (e.g., from `treasury_setup.json`).
    pub fn from_json(json: &str) -> Result<Self, String> {
        serde_json::from_str(json).map_err(|e| format!("Invalid treasury V2 JSON: {}", e))
    }

    /// Human-readable policy string, e.g. "3-of-5".
    pub fn policy_string(&self) -> String {
        format!("{}-of-{}", self.required, self.total_signers)
    }
}

// ---------------------------------------------------------------------------
// Common multisig configurations
// ---------------------------------------------------------------------------

/// Common multisig configurations.
///
/// `ThreeOfN` covers the general 3-of-N treasury policy.
#[derive(Debug, Clone, Copy)]
pub enum MultiSigType {
    TwoOfThree,      // 2-of-3 (legacy treasury)
    ThreeOfFive,     // 3-of-5
    FourOfSeven,     // 4-of-7
    ThreeOfN(usize), // 3-of-N (arbitrary N ≥ 3)
}

impl MultiSigType {
    pub fn required_signatures(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree => 2,
            MultiSigType::ThreeOfFive => 3,
            MultiSigType::FourOfSeven => 4,
            MultiSigType::ThreeOfN(_) => 3,
        }
    }
    pub fn total_signers(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree => 3,
            MultiSigType::ThreeOfFive => 5,
            MultiSigType::FourOfSeven => 7,
            MultiSigType::ThreeOfN(n) => *n,
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // TreasuryMultisigV2 — address stability
    // Generating twice with the same keys must yield the same address.
    // -----------------------------------------------------------------------
    #[test]
    fn test_treasury_v2_address_from_known_keys() {
        let k0 = FalconKeypair::generate();
        let k1 = FalconKeypair::generate();
        let k2 = FalconKeypair::generate();
        let k3 = FalconKeypair::generate();
        let k4 = FalconKeypair::generate();

        let pks = vec![
            k0.public_key.clone(),
            k1.public_key.clone(),
            k2.public_key.clone(),
            k3.public_key.clone(),
            k4.public_key.clone(),
        ];

        // Reconstruct the address two different ways — must agree.
        let addr_from_address_fn = multisig_address(&pks, 3, 5);
        let setup = TreasuryMultisigV2::from_public_keys(pks).expect("Valid keyset");

        assert_eq!(setup.address, addr_from_address_fn);
        assert!(
            setup.address.starts_with("ms"),
            "Multisig address must start with 'ms'"
        );
        assert_eq!(setup.required, 3);
        assert_eq!(setup.total_signers, 5);
    }

    // -----------------------------------------------------------------------
    // TreasuryMultisigV2 — full 3-of-5 sign and verify round-trip
    // -----------------------------------------------------------------------
    #[test]
    fn test_treasury_v2_3of5_sign_and_verify() {
        let (setup, keypairs) = TreasuryMultisigV2::generate(5);

        let mut proposal = setup.propose_spend(
            "0xdeadbeef".to_string(),
            5_000_000, // 5 QUA
            1_000,
            1,
            1_700_000_000,
        );

        // Sign with keys 0, 2, 4 (any 3 of the 5)
        TreasuryMultisigV2::sign_proposal(&mut proposal, 0, &keypairs[0]).expect("Sign with key 0");
        TreasuryMultisigV2::sign_proposal(&mut proposal, 2, &keypairs[2]).expect("Sign with key 2");

        // 2 signatures — not yet complete
        assert!(!proposal.is_complete(), "2 sigs should not satisfy 3-of-5");

        TreasuryMultisigV2::sign_proposal(&mut proposal, 4, &keypairs[4]).expect("Sign with key 4");

        // 3 signatures — complete and valid
        assert!(proposal.is_complete(), "3 sigs should satisfy 3-of-5");
        assert!(
            proposal.verify(),
            "3-of-5 proposal with valid sigs must verify"
        );

        let (collected, required) = proposal.signature_progress();
        assert_eq!(collected, 3);
        assert_eq!(required, 3);
    }

    // -----------------------------------------------------------------------
    // TreasuryMultisigV2 — threshold enforcement
    // Signing with fewer than 3 keys must not satisfy is_complete().
    // -----------------------------------------------------------------------
    #[test]
    fn test_treasury_v2_threshold_enforcement() {
        let (setup, keypairs) = TreasuryMultisigV2::generate(7); // 3-of-7

        let mut proposal =
            setup.propose_spend("0xrecipient".to_string(), 1_000_000, 500, 2, 1_700_000_001);

        // 0 sigs
        assert!(!proposal.is_complete());
        assert!(!proposal.verify());

        // 1 sig
        TreasuryMultisigV2::sign_proposal(&mut proposal, 3, &keypairs[3]).expect("Sign");
        assert!(!proposal.is_complete());

        // 2 sigs
        TreasuryMultisigV2::sign_proposal(&mut proposal, 6, &keypairs[6]).expect("Sign");
        assert!(!proposal.is_complete(), "2 sigs must not satisfy 3-of-7");

        // 3 sigs → complete
        TreasuryMultisigV2::sign_proposal(&mut proposal, 1, &keypairs[1]).expect("Sign");
        assert!(proposal.is_complete(), "3 sigs must satisfy 3-of-7");
        assert!(proposal.verify());
    }

    // -----------------------------------------------------------------------
    // TreasuryMultisigV2 — wrong keypair cannot forge
    // -----------------------------------------------------------------------
    #[test]
    fn test_treasury_v2_wrong_keypair_rejected() {
        let (setup, _keypairs) = TreasuryMultisigV2::generate(5);
        let attacker = FalconKeypair::generate();

        let mut proposal = setup.propose_spend(
            "0xattacker".to_string(),
            999_000_000,
            1_000,
            99,
            1_700_000_002,
        );

        // Attacker tries to sign as index 0
        let result = TreasuryMultisigV2::sign_proposal(&mut proposal, 0, &attacker);
        assert!(result.is_err(), "Wrong keypair must be rejected");
    }

    // -----------------------------------------------------------------------
    // TreasuryMultisigV2 — from_public_keys rejects insufficient keys
    // -----------------------------------------------------------------------
    #[test]
    fn test_treasury_v2_from_public_keys_validates_minimum() {
        let k0 = FalconKeypair::generate();
        let k1 = FalconKeypair::generate();
        // Only 2 keys — must fail (need >= 3)
        let result = TreasuryMultisigV2::from_public_keys(vec![
            k0.public_key.clone(),
            k1.public_key.clone(),
        ]);
        assert!(result.is_err(), "Should reject fewer than 3 public keys");
    }

    // -----------------------------------------------------------------------
    // MultiSigType::ThreeOfN — correct accessors
    // -----------------------------------------------------------------------
    #[test]
    fn test_multisig_type_three_of_n() {
        let t = MultiSigType::ThreeOfN(9);
        assert_eq!(t.required_signatures(), 3);
        assert_eq!(t.total_signers(), 9);
    }

    // -----------------------------------------------------------------------
    // multisig_address — deterministic regardless of key insertion order
    // -----------------------------------------------------------------------
    #[test]
    fn test_multisig_address_order_independent() {
        let k0 = FalconKeypair::generate();
        let k1 = FalconKeypair::generate();
        let k2 = FalconKeypair::generate();

        let order_a = vec![
            k0.public_key.clone(),
            k1.public_key.clone(),
            k2.public_key.clone(),
        ];
        let order_b = vec![
            k2.public_key.clone(),
            k0.public_key.clone(),
            k1.public_key.clone(),
        ];

        assert_eq!(
            multisig_address(&order_a, 3, 3),
            multisig_address(&order_b, 3, 3),
            "Address must be independent of key insertion order"
        );
    }
}
