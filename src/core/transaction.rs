use serde::{Serialize, Deserialize};
use crate::crypto::{verify_signature_strict, canonical_signing_hash};
use crate::core::{TESTNET_NETWORK_ID};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Signature scheme enum — enables crypto agility via soft fork
// ---------------------------------------------------------------------------

/// Identifies which signature algorithm was used to sign this transaction.
///
/// Consensus rule: nodes must reject any transaction whose `sig_scheme` value
/// they do not recognize. This allows future algorithms to be introduced via
/// a soft fork without breaking older nodes that will simply reject unknown
/// scheme values (conservative upgrade path).
///
/// FROZEN VALUES — do not reorder or delete:
///   0 = Falcon512  (current, post-quantum)
///   1 = Reserved   (placeholder; no implementation)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Copy)]
#[repr(u8)]
pub enum SignatureScheme {
    /// Falcon-512 (NIST PQC Round 3 — compact lattice signatures).
    Falcon512 = 0,
    /// Reserved for future algorithms. Transactions using this value will be
    /// rejected by all current nodes until a soft fork activates support.
    Reserved = 1,
}

impl Default for SignatureScheme {
    fn default() -> Self {
        SignatureScheme::Falcon512
    }
}

// ---------------------------------------------------------------------------
// Transaction
// ---------------------------------------------------------------------------

/// A signed transaction on the Quanta blockchain.
/// Amount and fee are denominated in microunits (1 QUA = 1_000_000 microunits).
///
/// SIGNING CONTRACT:
///   The bytes that are signed are: SHA3-256(SIGNING_DOMAIN || get_signing_bytes())
///   where SIGNING_DOMAIN = b"QUANTA_TX_V1:" (see crypto::SIGNING_DOMAIN).
///   This is enforced by `get_signing_data()` and verified by `verify()`.
///
/// CONSENSUS RULE:
///   Nodes must only call `verify()` inside consensus logic.
///   Signing (keypair operations) must never occur inside the consensus path.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
    pub timestamp: i64,
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
    pub fee: u64,
    pub nonce: u64,
    /// The block height before which this transaction cannot be included.
    /// Defaults to 0 (no lock time). Prevents fee sniping.
    #[serde(default)]
    pub lock_time: u64,
    pub tx_type: TransactionType,
    /// Signature scheme used. Defaults to `Falcon512`.
    /// Included in the signing payload so that scheme substitution is rejected.
    #[serde(default)]
    pub sig_scheme: SignatureScheme,
    /// Chain network identifier — prevents cross-chain replay attacks.
    ///
    /// FROZEN VALUES (never renumber):
    ///   0 = Testnet  (QUA7 and all future testnets — **current default**)
    ///   1 = Mainnet
    ///
    /// `#[serde(default)]` keeps this backwards-compatible when deserializing
    /// old transactions from disk or the network; they will read as 0 (Testnet),
    /// which is correct for the existing QUA7 testnet chain.
    /// Wallets and the node must set this to `config.network_type.network_id()`
    /// before signing.
    #[serde(default)]
    pub network_id: u32,
}

/// Transaction types supported by the protocol.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Transfer,
    TimeLockTransfer { unlock_height: u64 },
    MultiSigTransfer { signers_required: u8 },
}

impl Transaction {
    /// Create a new unsigned Transfer transaction.
    pub fn new(sender: String, recipient: String, amount: u64, timestamp: i64) -> Self {
        Self {
            sender,
            recipient,
            amount,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 1000,
            nonce: 0,
            lock_time: 0,
            tx_type: TransactionType::Transfer,
            sig_scheme: SignatureScheme::Falcon512,
            // Default to Testnet (0). The caller must override with
            // `tx.network_id = config.network_type.network_id()` before signing.
            network_id: TESTNET_NETWORK_ID,
        }
    }

    /// Create an unsigned TimeLockTransfer transaction.
    pub fn new_time_lock(
        sender: String,
        recipient: String,
        amount: u64,
        unlock_height: u64,
        timestamp: i64,
        nonce: u64,
    ) -> Self {
        Self {
            sender,
            recipient,
            amount,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 5000, // Higher fee for time-locked transfers
            nonce,
            lock_time: 0,
            tx_type: TransactionType::TimeLockTransfer { unlock_height },
            sig_scheme: SignatureScheme::Falcon512,
            network_id: TESTNET_NETWORK_ID,
        }
    }

    // -----------------------------------------------------------------------
    // Serialization helpers
    // -----------------------------------------------------------------------

    /// Produce the raw bytes that are fed into the canonical signing hash.
    ///
    /// CONSENSUS RULES (FROZEN FOREVER):
    ///   - All integers encoded as LITTLE-ENDIAN.
    ///   - Strings encoded as UTF-8.
    ///   - `sig_scheme` encoded as a single byte (its u8 discriminant).
    ///   - `network_id` encoded as 4 LE bytes — cross-chain replay protection.
    ///   - `public_key` included to bind the signature to a specific key.
    ///   - Signature field is EXCLUDED (you cannot sign the signature).
    ///
    /// The returned bytes are NOT what the user passes to `sign()`.
    /// The signer must call `canonical_signing_hash(get_signing_bytes())` which
    /// prepends the domain tag and applies SHA3-256, yielding a 32-byte value
    /// that is then signed with Falcon-512.
    pub fn get_signing_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(260);

        buf.extend_from_slice(self.sender.as_bytes());
        buf.extend_from_slice(self.recipient.as_bytes());
        buf.extend_from_slice(&self.amount.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.fee.to_le_bytes());
        buf.extend_from_slice(&self.nonce.to_le_bytes());
        buf.extend_from_slice(&self.lock_time.to_le_bytes());
        buf.extend_from_slice(&self.public_key);
        // Include sig_scheme so that scheme substitution attacks fail.
        buf.push(self.sig_scheme as u8);
        // Include network_id so that cross-chain replay attacks fail.
        // A Testnet signature (network_id=0) is invalid on Mainnet (1).
        buf.extend_from_slice(&self.network_id.to_le_bytes());

        match &self.tx_type {
            TransactionType::Transfer => buf.push(0u8),
            TransactionType::TimeLockTransfer { unlock_height } => {
                buf.push(1u8);
                buf.extend_from_slice(&unlock_height.to_le_bytes());
            }
            TransactionType::MultiSigTransfer { signers_required } => {
                buf.push(2u8);
                buf.push(*signers_required);
            }
        }

        buf
    }

    /// Compute the canonical 32-byte signing hash: SHA3-256(SIGNING_DOMAIN || get_signing_bytes()).
    ///
    /// This is the exact 32-byte value that was signed by the sender's Falcon-512 key.
    /// Verification calls this and passes the result to `verify_signature_strict()`.
    pub fn get_signing_data(&self) -> [u8; 32] {
        canonical_signing_hash(&self.get_signing_bytes())
    }

    // -----------------------------------------------------------------------
    // Canonical transaction hash (for mempool dedup, Merkle tree, etc.)
    // -----------------------------------------------------------------------

    /// Calculate the transaction's canonical identifier hash.
    ///
    /// Covers ALL fields except `signature` (the signature signs the hash,
    /// so including it would be circular). Used for:
    ///   - Mempool deduplication
    ///   - Merkle tree leaves
    ///   - Block explorers
    ///
    /// CONSENSUS RULES (FROZEN FOREVER):
    ///   - All integers LITTLE-ENDIAN.
    ///   - `sig_scheme` byte included.
    ///   - `public_key` included (prevents key substitution).
    pub fn hash(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();

        hasher.update(self.sender.as_bytes());
        hasher.update(self.recipient.as_bytes());
        hasher.update(&self.amount.to_le_bytes());
        hasher.update(&self.timestamp.to_le_bytes());
        hasher.update(&self.fee.to_le_bytes());
        hasher.update(&self.nonce.to_le_bytes());
        hasher.update(&self.lock_time.to_le_bytes());
        hasher.update(&self.public_key);
        hasher.update(&[self.sig_scheme as u8]);
        hasher.update(&self.network_id.to_le_bytes());

        match &self.tx_type {
            TransactionType::Transfer => hasher.update(&[0u8]),
            TransactionType::TimeLockTransfer { unlock_height } => {
                hasher.update(&[1u8]);
                hasher.update(&unlock_height.to_le_bytes());
            }
            TransactionType::MultiSigTransfer { signers_required } => {
                hasher.update(&[2u8]);
                hasher.update(&[*signers_required]);
            }
        }

        hex::encode(hasher.finalize())
    }

    // -----------------------------------------------------------------------
    // Verification — THE ONLY signature-checking entry point
    // -----------------------------------------------------------------------

    /// Verify the transaction signature.
    ///
    /// This is the ONLY function that should be called in consensus paths.
    /// It performs the following checks in order:
    ///
    /// 1. Coinbase / Treasury bypass — these are validated by block reward rules, not signatures.
    /// 2. Empty signature or public key — rejected immediately.
    /// 3. Sender matches public key — prevents key substitution attacks.
    /// 4. Signature scheme must be `Falcon512` — any other value is rejected.
    /// 5. Delegate to `verify_signature_strict()` with the canonical signing hash.
    ///
    /// Returns `true` only if ALL checks pass.
    pub fn verify(&self) -> bool {
        // Rule 1: System transactions bypass signature verification.
        if self.is_coinbase() || self.sender == "TREASURY" || self.is_genesis_premine() {
            return true;
        }

        // Rule 2: Reject empty fields immediately.
        if self.signature.is_empty() || self.public_key.is_empty() {
            tracing::debug!("Transaction verify: empty signature or public key");
            return false;
        }

        // Rule 3: Verify sender derives from the supplied public key.
        let derived = self.derive_address_from_pubkey();
        if self.sender != derived {
            tracing::warn!(
                "Transaction verify: sender address mismatch — claimed {}, derived {}",
                self.sender, derived
            );
            return false;
        }

        // Rule 4: Only the active signature scheme is accepted.
        match self.sig_scheme {
            SignatureScheme::Falcon512 => {} // allowed
            SignatureScheme::Reserved => {
                tracing::warn!("Transaction verify: reserved signature scheme rejected");
                return false;
            }
        }

        // Rule 5: Strict Falcon-512 verification against the canonical signing hash.
        let signing_hash = self.get_signing_data();
        let ok = verify_signature_strict(&signing_hash, &self.signature, &self.public_key);
        if !ok {
            tracing::debug!("Transaction verify: Falcon-512 strict verification failed");
        }
        ok
    }

    /// Derive address from the public key embedded in this transaction.
    fn derive_address_from_pubkey(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let hash = Sha3_256::digest(&self.public_key);
        format!("0x{}", hex::encode(&hash[..20]))
    }

    /// Returns `true` if this is a coinbase (mining reward) transaction.
    pub fn is_coinbase(&self) -> bool {
        self.sender == "COINBASE"
    }

    /// Returns `true` if this is a genesis premine credit.
    /// Genesis premine funds are immediately spendable (no coinbase maturity lock).
    pub fn is_genesis_premine(&self) -> bool {
        self.sender == "GENESIS"
    }
}

// ---------------------------------------------------------------------------
// Account state types
// ---------------------------------------------------------------------------

/// A single locked balance entry (e.g., coinbase maturity, vesting).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockedBalance {
    pub amount: u64,
    pub unlock_height: u64,
}

/// Per-address account record (account-based model, not UTXO).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountBalance {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
    pub locked_balances: Vec<LockedBalance>,
}

/// In-memory account state database.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountState {
    accounts: HashMap<String, AccountBalance>,
}

impl AccountState {
    pub fn new() -> Self {
        Self { accounts: HashMap::new() }
    }

    /// Calculate deterministic state root hash of all accounts.
    ///
    /// CONSENSUS FIX: locked_balances is a Vec whose insertion order differs
    /// between create_block_template (coinbase credited first) and
    /// validate_block_consensus (user txs applied first, coinbase in a second
    /// pass).  Sorting by (unlock_height, amount) before hashing makes the
    /// state root independent of insertion order.
    pub fn calculate_state_root(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        
        let mut keys: Vec<&String> = self.accounts.keys().collect();
        keys.sort();
        
        for key in keys {
            if let Some(acc) = self.accounts.get(key) {
                hasher.update(acc.address.as_bytes());
                hasher.update(&acc.balance.to_le_bytes());
                hasher.update(&acc.nonce.to_le_bytes());
                // Sort locked balances deterministically so insertion order
                // (which differs between mining and validation paths) does not
                // affect the hash.
                let mut sorted_locks: Vec<&LockedBalance> = acc.locked_balances.iter().collect();
                sorted_locks.sort_by_key(|l| (l.unlock_height, l.amount));
                for locked in sorted_locks {
                    hasher.update(&locked.amount.to_le_bytes());
                    hasher.update(&locked.unlock_height.to_le_bytes());
                }
            }
        }
        
        hex::encode(hasher.finalize())
    }

    /// Credit an account from a transaction.
    ///   - Coinbase credits are locked until `current_height + coinbase_maturity`.
    ///   - Regular credits are immediately spendable.
    pub fn credit_account(&mut self, tx: &Transaction, current_height: u64, coinbase_maturity: u64) {
        if tx.amount == 0 {
            return;
        }

        let account = self.accounts.entry(tx.recipient.clone()).or_insert(AccountBalance {
            address: tx.recipient.clone(),
            balance: 0,
            nonce: 0,
            locked_balances: Vec::new(),
        });

        if tx.is_coinbase() {
            // Mining rewards: locked for coinbase_maturity blocks before spending.
            account.locked_balances.push(LockedBalance {
                amount: tx.amount,
                unlock_height: current_height + coinbase_maturity,
            });
        } else if tx.is_genesis_premine() {
            // Genesis premine: always immediately spendable.
            // The caller also passes coinbase_maturity=0 for these, but we guard
            // here explicitly so a future refactor cannot accidentally re-lock them.
            account.balance = account.balance.saturating_add(tx.amount);
        } else if let TransactionType::TimeLockTransfer { unlock_height } = tx.tx_type {
            account.locked_balances.push(LockedBalance {
                amount: tx.amount,
                unlock_height,
            });
        } else {
            account.balance = account.balance.saturating_add(tx.amount);
        }
    }

    /// Debit an account.
    /// Returns `false` if the account has insufficient spendable balance.
    pub fn debit_account(&mut self, address: &str, total_amount: u64) -> bool {
        if let Some(account) = self.accounts.get_mut(address) {
            if account.balance >= total_amount {
                account.balance -= total_amount;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Move mature coinbase balances into the spendable pool.
    /// Called once per new block.
    pub fn unlock_mature_coinbase(&mut self, current_height: u64) {
        for account in self.accounts.values_mut() {
            let mut unlocked_total = 0u64;
            account.locked_balances.retain(|lock| {
                if current_height >= lock.unlock_height {
                    unlocked_total += lock.amount;
                    false
                } else {
                    true
                }
            });
            account.balance = account.balance.saturating_add(unlocked_total);
        }
    }

    /// Add a locked balance with a specific unlock height (vesting / anti-dump).
    pub fn add_locked_balance(&mut self, address: &str, amount: u64, unlock_height: u64) {
        let account = self.accounts.entry(address.to_string()).or_insert(AccountBalance {
            address: address.to_string(),
            balance: 0,
            nonce: 0,
            locked_balances: Vec::new(),
        });
        account.locked_balances.push(LockedBalance { amount, unlock_height });
    }

    /// Spendable balance for an address.
    pub fn get_balance(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.balance).unwrap_or(0)
    }

    /// Total balance (spendable + all locked entries).
    pub fn get_total_balance(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| {
            let locked: u64 = acc.locked_balances.iter().map(|l| l.amount).sum();
            acc.balance + locked
        }).unwrap_or(0)
    }

    /// Current nonce for an address.
    pub fn get_nonce(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.nonce).unwrap_or(0)
    }

    /// Increment nonce for an address (called after a transaction is applied).
    pub fn increment_nonce(&mut self, address: &str) {
        if let Some(acc) = self.accounts.get_mut(address) {
            acc.nonce += 1;
        } else {
            self.accounts.insert(address.to_string(), AccountBalance {
                address: address.to_string(),
                balance: 0,
                nonce: 1,
                locked_balances: Vec::new(),
            });
        }
    }

    /// Returns `true` if the transaction nonce is the next expected value.
    pub fn verify_nonce(&self, address: &str, tx_nonce: u64) -> bool {
        let account_nonce = self.get_nonce(address);
        tx_nonce == account_nonce + 1 || (account_nonce == 0 && tx_nonce == 1)
    }

    /// Returns `true` if the address can spend `amount`.
    pub fn has_sufficient_balance(&self, address: &str, amount: u64) -> bool {
        self.get_balance(address) >= amount
    }

    /// All known addresses.
    pub fn get_accounts(&self) -> Vec<String> {
        self.accounts.keys().cloned().collect()
    }

    /// Full account record for an address (balance + locked + nonce).
    /// Returns `None` if the address has never appeared on-chain.
    pub fn get_account(&self, address: &str) -> Option<&AccountBalance> {
        self.accounts.get(address)
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::FalconKeypair;

    fn signed_transfer(kp: &FalconKeypair, amount: u64, nonce: u64) -> Transaction {
        let mut tx = Transaction::new(
            kp.get_address(),
            "0xrecipient000000000000000000000000000000".to_string(),
            amount,
            1_700_000_000,
        );
        tx.nonce = nonce;
        tx.public_key = kp.public_key.clone();
        let signing_bytes = tx.get_signing_bytes();
        tx.signature = kp.sign_transaction_canonical(&signing_bytes);
        tx
    }

    #[test]
    fn test_valid_transaction_verifies() {
        let kp = FalconKeypair::generate();
        let tx = signed_transfer(&kp, 1_000_000, 1);
        assert!(tx.verify(), "Properly signed transaction must verify");
    }

    #[test]
    fn test_sig_scheme_defaults_to_falcon512() {
        let tx = Transaction::new("a".into(), "b".into(), 0, 0);
        assert_eq!(tx.sig_scheme, SignatureScheme::Falcon512);
    }

    #[test]
    fn test_reserved_scheme_rejected() {
        let kp = FalconKeypair::generate();
        let mut tx = signed_transfer(&kp, 1_000, 1);
        tx.sig_scheme = SignatureScheme::Reserved;
        assert!(!tx.verify(), "Reserved signature scheme must be rejected by verify()");
    }

    #[test]
    fn test_tampered_amount_invalidates_signature() {
        let kp = FalconKeypair::generate();
        let mut tx = signed_transfer(&kp, 1_000, 1);
        tx.amount = 9_999_999;
        assert!(!tx.verify(), "Tampering with amount must invalidate signature");
    }

    #[test]
    fn test_empty_signature_rejected() {
        let kp = FalconKeypair::generate();
        let mut tx = signed_transfer(&kp, 1_000, 1);
        tx.signature = vec![];
        assert!(!tx.verify(), "Empty signature must be rejected");
    }

    #[test]
    fn test_wrong_sender_rejected() {
        let kp = FalconKeypair::generate();
        let attacker = FalconKeypair::generate();
        let mut tx = signed_transfer(&kp, 1_000, 1);
        // Replace public key with attacker's — sender address will no longer match
        tx.public_key = attacker.public_key.clone();
        assert!(!tx.verify(), "Mismatched sender/pubkey must be rejected");
    }

    #[test]
    fn test_hash_is_deterministic() {
        let kp = FalconKeypair::generate();
        let tx = signed_transfer(&kp, 500_000, 2);
        assert_eq!(tx.hash(), tx.hash(), "Transaction hash must be deterministic");
    }

    #[test]
    fn test_signing_data_changes_with_amount() {
        let kp = FalconKeypair::generate();
        let mut tx1 = Transaction::new(kp.get_address(), "0xrecip".into(), 100, 0);
        tx1.public_key = kp.public_key.clone();
        let mut tx2 = tx1.clone();
        tx2.amount = 999;
        assert_ne!(
            tx1.get_signing_data(),
            tx2.get_signing_data(),
            "Different amounts must produce different signing hashes"
        );
    }

    #[test]
    fn test_signing_data_changes_with_lock_time() {
        let kp = FalconKeypair::generate();
        let mut tx1 = Transaction::new(kp.get_address(), "0xrecip".into(), 100, 0);
        tx1.public_key = kp.public_key.clone();
        let mut tx2 = tx1.clone();
        tx2.lock_time = 1_000_000;
        assert_ne!(
            tx1.get_signing_data(),
            tx2.get_signing_data(),
            "Different lock_times must produce different signing hashes"
        );
    }

    fn insert_balance(state: &mut AccountState, address: &str, balance: u64) {
        state.accounts.insert(address.to_string(), AccountBalance {
            address: address.to_string(),
            balance,
            nonce: 0,
            locked_balances: Vec::new(),
        });
    }

    #[test]
    fn test_calculate_state_root_deterministic() {
        let mut state = AccountState::new();
        insert_balance(&mut state, "0xAlice", 1000);
        insert_balance(&mut state, "0xBob", 500);
        let root1 = state.calculate_state_root();
        
        let mut state2 = AccountState::new();
        // Insert in reverse order to ensure sorting works
        insert_balance(&mut state2, "0xBob", 500);
        insert_balance(&mut state2, "0xAlice", 1000);
        let root2 = state2.calculate_state_root();
        
        assert_eq!(root1, root2, "State root must be deterministic regardless of insertion order");
        assert_ne!(root1, "", "State root should not be empty");
    }

    #[test]
    fn test_coinbase_bypasses_signature_check() {
        let tx = Transaction {
            sender: "COINBASE".to_string(),
            recipient: "0xminer000000000000000000000000000000".to_string(),
            amount: 100_000_000,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            lock_time: 0,
            tx_type: TransactionType::Transfer,
            sig_scheme: SignatureScheme::Falcon512,
            network_id: 0,
        };
        assert!(tx.verify(), "Coinbase must bypass signature verification");
    }

    /// A Testnet-signed transaction must be rejected if `network_id` is changed
    /// to Mainnet — proving cross-chain replay protection is in effect.
    #[test]
    fn test_cross_chain_replay_rejected() {
        use crate::core::{TESTNET_NETWORK_ID, MAINNET_NETWORK_ID};
        let kp = FalconKeypair::generate();
        // Sign as Testnet (network_id = 0)
        let mut tx = signed_transfer(&kp, 1_000, 1);
        assert_eq!(tx.network_id, TESTNET_NETWORK_ID);
        assert!(tx.verify(), "Testnet-signed tx must verify on Testnet");

        // Mutate to Mainnet — signature payload now differs, must fail.
        tx.network_id = MAINNET_NETWORK_ID;
        assert!(
            !tx.verify(),
            "Testnet-signed tx must NOT verify on Mainnet (cross-chain replay rejected)"
        );
    }
}
