use crate::core::transaction::Transaction;
use crate::crypto::signatures::{verify_signature_strict, FalconKeypair};
use serde::{Deserialize, Serialize};
use sha3::{Sha3_256, Digest};

/// Multi-signature transaction requiring M-of-N signatures
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultiSigTransaction {
    pub base_tx: Transaction,
    pub required_signatures: usize,  // M
    pub public_keys: Vec<Vec<u8>>,   // N public keys
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
                required_signatures, public_keys.len()
            ));
        }
        let signatures = vec![None; public_keys.len()];
        Ok(Self { base_tx, required_signatures, public_keys, signatures })
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
        let valid = self.signatures.iter().enumerate()
            .filter_map(|(i, sig_opt)| sig_opt.as_ref().map(|sig| (i, sig)))
            .filter(|(i, sig)| verify_signature_strict(&signing_hash, sig, &self.public_keys[*i]))
            .count();
        valid >= self.required_signatures
    }

    /// Get the canonical multisig address for this keyset
    pub fn get_multisig_address(&self) -> String {
        multisig_address(&self.public_keys, self.required_signatures, self.public_keys.len())
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
    hasher.update(&[required as u8, total as u8]);
    for pk in &sorted_keys {
        hasher.update(pk);
    }
    let hash = hasher.finalize();
    format!("ms{}", hex::encode(&hash[..20]))
}

// ---------------------------------------------------------------------------
// Treasury Multisig — 2-of-3, all keys held by founder (pre-testnet)
// ---------------------------------------------------------------------------

/// Treasury setup for the pre-testnet phase.
///
/// The founder generates 3 separate Falcon-512 keypairs and stores them in 3
/// separate wallet files (treasury_key0.qua, treasury_key1.qua, treasury_key2.qua).
/// Any 2 of the 3 keys can authorize a spend.
///
/// # Workflow
/// ```
/// # 1. One-time setup (generates 3 keys + prints treasury address)
/// quanta-wallet treasury-init --out treasury_setup.json
///
/// # 2. Add treasury_address from setup to quanta.toml
///
/// # 3. Propose a spend (creates unsigned JSON)
/// quanta-wallet treasury-propose --setup treasury_setup.json \
///     --to 0xRecipient --amount 100 --out proposal.json
///
/// # 4. Sign with key 0
/// quanta-wallet treasury-sign --proposal proposal.json --key treasury_key0.qua
///
/// # 5. Sign with key 1 (2-of-3 satisfied)
/// quanta-wallet treasury-sign --proposal proposal.json --key treasury_key1.qua
///
/// # 6. Broadcast the completed proposal
/// quanta-wallet treasury-broadcast --proposal proposal.json --node http://localhost:3000
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TreasuryMultisig {
    /// The 3 Falcon-512 public keys (each stored in a separate wallet file)
    pub public_keys: Vec<Vec<u8>>,
    /// 2-of-3 threshold
    pub required: usize,
    /// The canonical 2-of-3 multisig address — use this as TREASURY_ADDRESS
    pub address: String,
}

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
        let setup = TreasuryMultisig { public_keys, required: 2, address };
        (setup, [k0, k1, k2])
    }

    /// Reconstruct from 3 known public keys (to verify address or re-display).
    pub fn from_public_keys(pk0: Vec<u8>, pk1: Vec<u8>, pk2: Vec<u8>) -> Self {
        let public_keys = vec![pk0, pk1, pk2];
        let address = multisig_address(&public_keys, 2, 3);
        TreasuryMultisig { public_keys, required: 2, address }
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
            sender:     self.address.clone(),
            recipient:  to,
            amount:     amount_microunits,
            timestamp,
            signature:  vec![],
            public_key: vec![],  // Multisig — no single pubkey
            fee,
            nonce,
            tx_type:    crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
        };
        MultiSigTransaction {
            base_tx,
            required_signatures: self.required,
            public_keys:  self.public_keys.clone(),
            signatures:   vec![None; self.public_keys.len()],
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

/// Common multisig configurations
#[derive(Debug, Clone, Copy)]
pub enum MultiSigType {
    TwoOfThree,    // 2-of-3
    ThreeOfFive,   // 3-of-5
    FourOfSeven,   // 4-of-7
}

impl MultiSigType {
    pub fn required_signatures(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree  => 2,
            MultiSigType::ThreeOfFive => 3,
            MultiSigType::FourOfSeven => 4,
        }
    }
    pub fn total_signers(&self) -> usize {
        match self {
            MultiSigType::TwoOfThree  => 3,
            MultiSigType::ThreeOfFive => 5,
            MultiSigType::FourOfSeven => 7,
        }
    }
}
