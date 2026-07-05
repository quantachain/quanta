#![allow(dead_code)]
use crate::core::TESTNET_NETWORK_ID;
use crate::crypto::{canonical_signing_hash, verify_signature_strict};
use serde::{Deserialize, Serialize};
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
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    std::hash::Hash,
    Copy,
    codec::Encode,
    codec::Decode,
)]
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
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    std::hash::Hash,
    codec::Encode,
    codec::Decode,
)]
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
    /// Arbitrary metadata payload (e.g. for AI agent data provenance or contract arguments).
    #[serde(default)]
    pub payload: Vec<u8>,
}
/// Transaction types supported by the protocol.
///
/// FROZEN VALUES — do not reorder or delete; only append.
///   0 = Transfer
///   1 = TimeLockTransfer
///   2 = MultiSigTransfer
///   3 = Stake          (v2 — register as BFT validator)
///   4 = Unstake        (v2 — deregister, begin unbonding)
///   5 = ContractDeploy (v2 — deploy a named contract template)
///   6 = ContractCall   (v2 — invoke a deployed contract)
///   7 = SlashEvidence  (v3 — submit double-signing proof for slashing)
#[derive(
    Clone,
    Debug,
    Serialize,
    Deserialize,
    PartialEq,
    Eq,
    std::hash::Hash,
    codec::Encode,
    codec::Decode,
)]
pub enum TransactionType {
    /// Standard value transfer.
    Transfer,
    /// Value transfer locked until a specific block height.
    TimeLockTransfer { unlock_height: u64 },
    /// Multi-signature transfer requiring M-of-N signers.
    MultiSigTransfer { signers_required: u8 },
    /// v2: Register as a BFT validator by staking QUA and providing
    /// a Falcon-512 public key (897 bytes) that will be used to sign
    /// BFT prevote / precommit messages.
    Stake { validator_pubkey: Vec<u8> },
    /// v2: Deregister as validator and begin the unbonding period.
    /// Staked QUA is locked for UNBONDING_EPOCHS epochs before release.
    Unstake,
    /// v2: Deploy a named smart contract template.
    /// `template_id` identifies which built-in template to instantiate.
    /// `init_args` is a JSON-encoded initialisation argument map.
    ContractDeploy { template_id: u8, init_args: Vec<u8> },
    /// v2: Invoke a method on a deployed contract.
    /// `contract_address` is the on-chain address of the contract.
    /// `method` is the UTF-8 method name.
    /// `call_args` is a JSON-encoded argument map.
    ContractCall {
        contract_address: String,
        method: String,
        call_args: Vec<u8>,
    },
    /// v3: Submit equivocation proof to slash a double-signing validator.
    ///
    /// The submitter provides two conflicting BFT signatures from the SAME
    /// validator at the SAME (height, round) but for DIFFERENT block hashes.
    /// Any honest node can collect these and submit for a whistleblower reward.
    SlashEvidence {
        /// Address of the validator being slashed.
        offender: String,
        /// Block height at which both votes were cast.
        height: u64,
        /// BFT round at which both votes were cast.
        round: u32,
        /// First BFT signature blob (raw_sig || payload).
        sig_a: Vec<u8>,
        /// Block hash that sig_a signed.
        hash_a: String,
        /// Second BFT signature blob (raw_sig || payload).
        sig_b: Vec<u8>,
        /// Block hash that sig_b signed (must differ from hash_a).
        hash_b: String,
    },
}

// ---------------------------------------------------------------------------
// Stablecoin Bridge Intent
// ---------------------------------------------------------------------------

/// Represents a stablecoin execution intent embedded in a transaction's payload.
/// This allows an AI agent to signal a bridge to execute an off-chain stablecoin transfer
/// upon successful on-chain transaction execution.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StablecoinIntent {
    /// Token symbol (e.g., "USDC", "USDT")
    pub token: String,
    /// Amount in token's base units (e.g., 6 decimals for USDC)
    pub amount: u64,
    /// Destination chain (e.g., "ETH", "SOL")
    pub dest_chain: String,
    /// Destination address on the target chain
    pub recipient: String,
}

impl StablecoinIntent {
    /// Serializes the intent to JSON bytes for the transaction payload
    pub fn to_payload(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Attempts to deserialize an intent from a transaction payload
    pub fn from_payload(payload: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(payload)
    }
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
            payload: vec![],
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
            payload: vec![],
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
        // Include payload (length prefix + bytes)
        buf.extend_from_slice(&(self.payload.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.payload);

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
            TransactionType::Stake { validator_pubkey } => {
                buf.push(3u8);
                buf.extend_from_slice(validator_pubkey);
            }
            TransactionType::Unstake => buf.push(4u8),
            TransactionType::ContractDeploy {
                template_id,
                init_args,
            } => {
                buf.push(5u8);
                buf.push(*template_id);
                buf.extend_from_slice(init_args);
            }
            TransactionType::ContractCall {
                contract_address,
                method,
                call_args,
            } => {
                buf.push(6u8);
                buf.extend_from_slice(contract_address.as_bytes());
                buf.extend_from_slice(method.as_bytes());
                buf.extend_from_slice(call_args);
            }
            TransactionType::SlashEvidence {
                offender,
                height,
                round,
                sig_a,
                hash_a,
                sig_b,
                hash_b,
            } => {
                buf.push(7u8);
                buf.extend_from_slice(offender.as_bytes());
                buf.extend_from_slice(&height.to_le_bytes());
                buf.extend_from_slice(&round.to_le_bytes());
                buf.extend_from_slice(sig_a);
                buf.extend_from_slice(hash_a.as_bytes());
                buf.extend_from_slice(sig_b);
                buf.extend_from_slice(hash_b.as_bytes());
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
            TransactionType::Stake { validator_pubkey } => {
                hasher.update(&[3u8]);
                hasher.update(validator_pubkey);
            }
            TransactionType::Unstake => hasher.update(&[4u8]),
            TransactionType::ContractDeploy {
                template_id,
                init_args,
            } => {
                hasher.update(&[5u8, *template_id]);
                hasher.update(init_args);
            }
            TransactionType::ContractCall {
                contract_address,
                method,
                call_args,
            } => {
                hasher.update(&[6u8]);
                hasher.update(contract_address.as_bytes());
                hasher.update(method.as_bytes());
                hasher.update(call_args);
            }
            TransactionType::SlashEvidence {
                offender,
                height,
                round,
                sig_a,
                hash_a,
                sig_b,
                hash_b,
            } => {
                hasher.update(&[7u8]);
                hasher.update(offender.as_bytes());
                hasher.update(&height.to_le_bytes());
                hasher.update(&round.to_le_bytes());
                hasher.update(sig_a);
                hasher.update(hash_a.as_bytes());
                hasher.update(sig_b);
                hasher.update(hash_b.as_bytes());
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
                self.sender,
                derived
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
    pub fn is_genesis_premine(&self) -> bool {
        self.sender == "GENESIS"
    }

    /// Returns `true` if this is a v2 `Stake` (validator registration) transaction.
    pub fn is_stake(&self) -> bool {
        matches!(self.tx_type, TransactionType::Stake { .. })
    }

    /// Returns `true` if this is a v2 `Unstake` (validator deregistration) transaction.
    pub fn is_unstake(&self) -> bool {
        matches!(self.tx_type, TransactionType::Unstake)
    }

    /// Returns `true` if this is a v2 `ContractDeploy` transaction.
    pub fn is_contract_deploy(&self) -> bool {
        matches!(self.tx_type, TransactionType::ContractDeploy { .. })
    }

    /// Returns `true` if this is a v2 `ContractCall` transaction.
    pub fn is_contract_call(&self) -> bool {
        matches!(self.tx_type, TransactionType::ContractCall { .. })
    }

    /// Returns `true` if this is a v3 `SlashEvidence` transaction.
    pub fn is_slash_evidence(&self) -> bool {
        matches!(self.tx_type, TransactionType::SlashEvidence { .. })
    }
} // impl Transaction

// ---------------------------------------------------------------------------
// Account state types
// ---------------------------------------------------------------------------

/// A single locked balance entry (e.g., coinbase maturity, unbonding stake).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockedBalance {
    pub amount: u64,
    pub unlock_height: u64,
}

/// Per-address account record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountBalance {
    pub address: String,
    pub balance: u64,
    pub nonce: u64,
    pub locked_balances: Vec<LockedBalance>,
}

/// BFT validator registration record.
///
/// Stored in `AccountState::validators` keyed by the validator’s address.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidatorInfo {
    /// Falcon-512 public key (897 bytes) used for BFT signing.
    pub falcon_pk: Vec<u8>,
    /// Staked QUA (microunits) locked while registered.
    pub stake: u64,
    /// Epoch in which this validator registered.
    pub registered_epoch: u64,
    /// Whether this validator is in the active set for the current epoch.
    pub active: bool,
    /// Epoch at which unbonding began (set when Unstake tx is processed).
    /// The stake is returned at epoch `unbonding_epoch + UNBONDING_EPOCHS`.
    /// 0 means not currently unbonding.
    #[serde(default)]
    pub unbonding_epoch: u64,
    /// If slashed: the epoch after which this address may re-register.
    /// 0 means not slashed / cooldown expired.
    #[serde(default)]
    pub slash_cooldown_until_epoch: u64,
    /// Block height of the last block this validator successfully proposed.
    /// Used for downtime detection at epoch boundaries.
    #[serde(default)]
    pub last_proposed_height: u64,
    /// Number of slots this validator was the DESIGNATED proposer in the current epoch.
    #[serde(default)]
    pub epoch_slots_assigned: u64,
    /// Number of slots this validator actually produced a block in the current epoch.
    #[serde(default)]
    pub epoch_slots_produced: u64,
}

/// A single on-chain event emitted by a native contract execution.
/// Indexed by QuaScan for contract activity feeds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractEvent {
    /// Block height at which this event was emitted.
    pub height: u64,
    /// Short event name (e.g. "AgentJobClaimed", "StreamWithdrawn").
    pub name: String,
    /// Arbitrary key-value data for the event.
    pub data: HashMap<String, String>,
}

/// Minimal deployed-contract state.
/// Full contract storage lives inside the `storage` map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractState {
    /// Address of the account that deployed this contract.
    pub owner: String,
    /// Template ID (matches `ContractDeploy::template_id`).
    pub template_id: u8,
    /// Block height at which the contract was deployed.
    pub deployed_at: u64,
    /// Contract-specific key-value storage.
    pub storage: HashMap<String, String>,
    /// Ordered log of events emitted by this contract.
    #[serde(default)]
    pub events: Vec<ContractEvent>,
}

/// In-memory global state — accounts, validators, contracts.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountState {
    accounts: HashMap<String, AccountBalance>,
    /// Registered BFT validators (address → ValidatorInfo).
    #[serde(default)]
    validators: HashMap<String, ValidatorInfo>,
    /// Deployed smart contracts (contract address → ContractState).
    #[serde(default)]
    pub contracts: HashMap<String, ContractState>,
}

impl AccountState {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            validators: HashMap::new(),
            contracts: HashMap::new(),
        }
    }

    /// Calculate deterministic state root hash of all accounts AND validators.
    ///
    /// SECURITY FIX (2026-06-24): Previously only hashed accounts. Validators
    /// were excluded, meaning a validator could be added/removed without changing
    /// the state root (chain-split vector). Now both maps are included.
    ///
    /// CONSENSUS FIX: locked_balances is sorted by (unlock_height, amount) to
    /// make the hash independent of insertion order.
    pub fn calculate_state_root(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();

        // --- Accounts (sorted by address) ---
        let mut account_keys: Vec<&String> = self.accounts.keys().collect();
        account_keys.sort();
        for key in account_keys {
            if let Some(acc) = self.accounts.get(key) {
                hasher.update(acc.address.as_bytes());
                hasher.update(&acc.balance.to_le_bytes());
                hasher.update(&acc.nonce.to_le_bytes());
                let mut sorted_locks: Vec<&LockedBalance> = acc.locked_balances.iter().collect();
                sorted_locks.sort_by_key(|l| (l.unlock_height, l.amount));
                for locked in sorted_locks {
                    hasher.update(&locked.amount.to_le_bytes());
                    hasher.update(&locked.unlock_height.to_le_bytes());
                }
            }
        }

        // --- Validators (sorted by address) ---
        let mut val_keys: Vec<&String> = self.validators.keys().collect();
        val_keys.sort();
        for key in val_keys {
            if let Some(v) = self.validators.get(key) {
                hasher.update(key.as_bytes());
                hasher.update(&v.stake.to_le_bytes());
                hasher.update(&[v.active as u8]);
                hasher.update(&v.registered_epoch.to_le_bytes());
                hasher.update(&v.unbonding_epoch.to_le_bytes());
                hasher.update(&v.slash_cooldown_until_epoch.to_le_bytes());
                hasher.update(&v.last_proposed_height.to_le_bytes());
                hasher.update(&v.epoch_slots_assigned.to_le_bytes());
                hasher.update(&v.epoch_slots_produced.to_le_bytes());
            }
        }

        hex::encode(hasher.finalize())
    }

    /// Credit an account from a transaction.
    ///   - Coinbase credits are locked until `current_height + coinbase_maturity`.
    ///   - Regular credits are immediately spendable.
    pub fn credit_account(
        &mut self,
        tx: &Transaction,
        current_height: u64,
        coinbase_maturity: u64,
    ) {
        // Automatically route any ContractDeploy/ContractCall logic
        crate::core::contracts::NativeContracts::execute(self, tx, current_height);

        if tx.amount == 0 {
            return;
        }

        let account = self
            .accounts
            .entry(tx.recipient.clone())
            .or_insert(AccountBalance {
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

    /// Directly credit an account with a specific amount (spendable immediately).
    /// Useful for internal contract transfers (e.g. Escrow unlocking).
    pub fn credit_account_direct(&mut self, address: &str, amount: u64) {
        if amount == 0 {
            return;
        }
        let account = self
            .accounts
            .entry(address.to_string())
            .or_insert(AccountBalance {
                address: address.to_string(),
                balance: 0,
                nonce: 0,
                locked_balances: Vec::new(),
            });
        account.balance = account.balance.saturating_add(amount);
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
        let account = self
            .accounts
            .entry(address.to_string())
            .or_insert(AccountBalance {
                address: address.to_string(),
                balance: 0,
                nonce: 0,
                locked_balances: Vec::new(),
            });
        account.locked_balances.push(LockedBalance {
            amount,
            unlock_height,
        });
    }

    /// Spendable balance for an address.
    pub fn get_balance(&self, address: &str) -> u64 {
        self.accounts
            .get(address)
            .map(|acc| acc.balance)
            .unwrap_or(0)
    }

    /// Total balance (spendable + all locked entries).
    pub fn get_total_balance(&self, address: &str) -> u64 {
        self.accounts
            .get(address)
            .map(|acc| {
                let locked: u64 = acc.locked_balances.iter().map(|l| l.amount).sum();
                acc.balance + locked
            })
            .unwrap_or(0)
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
            self.accounts.insert(
                address.to_string(),
                AccountBalance {
                    address: address.to_string(),
                    balance: 0,
                    nonce: 1,
                    locked_balances: Vec::new(),
                },
            );
        }
    }

    /// Override nonce for an address — used only during pre-checkpoint sync
    /// to faithfully replay a post-reorg chain whose nonce sequence differs
    /// from a clean sequential replay.
    pub fn set_nonce(&mut self, address: &str, nonce: u64) {
        if let Some(acc) = self.accounts.get_mut(address) {
            acc.nonce = nonce;
        } else {
            self.accounts.insert(
                address.to_string(),
                AccountBalance {
                    address: address.to_string(),
                    balance: 0,
                    nonce,
                    locked_balances: Vec::new(),
                },
            );
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

    /// Register a new BFT validator.
    ///
    /// The caller must have already debited `stake` from the sender’s balance.
    pub fn register_validator(
        &mut self,
        address: &str,
        falcon_pk: Vec<u8>,
        stake: u64,
        current_epoch: u64,
    ) {
        self.validators.insert(
            address.to_string(),
            ValidatorInfo {
                falcon_pk,
                stake,
                registered_epoch: current_epoch,
                active: true,
                unbonding_epoch: 0,
                slash_cooldown_until_epoch: 0,
                last_proposed_height: 0,
                epoch_slots_assigned: 0,
                epoch_slots_produced: 0,
            },
        );
    }

    /// Deregister a validator (Unstake transaction).
    ///
    /// Sets `active = false` and records the unbonding epoch so the stake
    /// can be returned after `UNBONDING_EPOCHS` epochs have passed.
    pub fn deregister_validator(&mut self, address: &str, current_epoch: u64) {
        if let Some(info) = self.validators.get_mut(address) {
            info.active = false;
            info.unbonding_epoch = current_epoch;
        }
    }

    /// Record that a validator produced a block at `height`.
    /// Call this once per block from the block-application path.
    pub fn record_block_proposed(&mut self, address: &str, height: u64) {
        if let Some(info) = self.validators.get_mut(address) {
            info.last_proposed_height = height;
            info.epoch_slots_produced = info.epoch_slots_produced.saturating_add(1);
        }
    }

    /// Record that a validator was the designated proposer for a slot.
    pub fn record_slot_assigned(&mut self, address: &str) {
        if let Some(info) = self.validators.get_mut(address) {
            info.epoch_slots_assigned = info.epoch_slots_assigned.saturating_add(1);
        }
    }

    /// Process epoch boundary: return unbonded stake and soft-slash inactive validators.
    ///
    /// Called once per epoch (when `block.index % EPOCH_SIZE == 0`).
    /// Returns a list of `(address, amount)` credits to apply to account balances
    /// (unbonded stake returns). The caller applies these credits to the state.
    pub fn process_epoch_boundary(
        &mut self,
        current_epoch: u64,
        soft_slash_pct: u64, // % of stake burned for downtime (e.g. 5)
        burn_address: &str,
    ) -> Vec<(String, u64)> {
        use crate::consensus::authorities::{MAX_MISSED_SLOTS_PCT, UNBONDING_EPOCHS};
        let mut stake_returns: Vec<(String, u64)> = Vec::new();
        let mut to_remove: Vec<String> = Vec::new();

        for (address, info) in self.validators.iter_mut() {
            // --- UNBONDING RETURN ---
            if !info.active && info.unbonding_epoch > 0 {
                let release_epoch = info.unbonding_epoch + UNBONDING_EPOCHS;
                if current_epoch >= release_epoch && info.stake > 0 {
                    tracing::info!(
                        "Unbonding complete for {}: returning {} microunits (unbonded epoch {}, released at {})",
                        address, info.stake, info.unbonding_epoch, release_epoch
                    );
                    stake_returns.push((address.clone(), info.stake));
                    info.stake = 0;
                    to_remove.push(address.clone());
                }
            }

            // --- DOWNTIME SOFT-SLASH ---
            if info.active && info.epoch_slots_assigned > 0 {
                let missed = info
                    .epoch_slots_assigned
                    .saturating_sub(info.epoch_slots_produced);
                let missed_pct = missed * 100 / info.epoch_slots_assigned;
                if missed_pct > MAX_MISSED_SLOTS_PCT {
                    let slash_amount = info.stake * soft_slash_pct / 100;
                    if slash_amount > 0 && slash_amount <= info.stake {
                        tracing::warn!(
                            "Downtime soft-slash {}: missed {}% of slots ({}/{}), burning {} microunits",
                            address, missed_pct, missed, info.epoch_slots_assigned, slash_amount
                        );
                        info.stake = info.stake.saturating_sub(slash_amount);
                        // Return burn amount as a "credit" to the burn address
                        stake_returns.push((burn_address.to_string(), slash_amount));
                    }
                }
            }

            // Reset per-epoch slot counters for the next epoch.
            info.epoch_slots_assigned = 0;
            info.epoch_slots_produced = 0;
        }

        // Remove fully-unbonded validator records.
        for addr in to_remove {
            self.validators.remove(&addr);
        }

        stake_returns
    }

    /// Slash a validator for equivocation (double-signing).
    ///
    /// Burns `slash_pct`% of their stake and marks them as slashed.
    /// Returns `(burned, whistleblower_reward)` in microunits.
    pub fn slash_validator(
        &mut self,
        offender: &str,
        current_epoch: u64,
        slash_pct: u64,         // % of stake to burn (e.g. 50)
        whistleblower_pct: u64, // % of slashed amount to reward whistleblower (e.g. 10)
    ) -> Option<(u64, u64)> {
        use crate::consensus::authorities::SLASH_COOLDOWN_EPOCHS;
        let info = self.validators.get_mut(offender)?;
        if !info.active {
            return None; // Already inactive
        }
        let slash_amount = info.stake * slash_pct / 100;
        let whistleblower_reward = slash_amount * whistleblower_pct / 100;
        let burned = slash_amount.saturating_sub(whistleblower_reward);

        info.stake = info.stake.saturating_sub(slash_amount);
        info.active = false;
        info.slash_cooldown_until_epoch = current_epoch + SLASH_COOLDOWN_EPOCHS;
        info.unbonding_epoch = 0; // Slash voids normal unbonding

        tracing::warn!(
            "SLASHED validator {}: burned {} microunits, whistleblower reward {} microunits, \
             cooldown until epoch {}",
            offender,
            burned,
            whistleblower_reward,
            info.slash_cooldown_until_epoch
        );
        Some((burned, whistleblower_reward))
    }

    /// Remove a fully-unbonded validator record after unbonding expires.
    pub fn remove_validator(&mut self, address: &str) {
        self.validators.remove(address);
    }

    /// Return info for a specific validator.
    pub fn get_validator_info(&self, address: &str) -> Option<&ValidatorInfo> {
        self.validators.get(address)
    }

    /// Return the full validators map (for authority module / epoch rotation).
    pub fn get_validators(&self) -> &HashMap<String, ValidatorInfo> {
        &self.validators
    }

    /// Compute the epoch committee: top-N active validators by stake,
    /// sorted deterministically by address for tie-breaking.
    ///
    /// `max_committee_size` is typically 21.
    pub fn compute_epoch_committee(&self, max_committee_size: usize) -> Vec<String> {
        let mut active: Vec<(&String, &ValidatorInfo)> =
            self.validators.iter().filter(|(_, v)| v.active).collect();

        // Primary sort: stake descending. Secondary sort: address ascending (tie-break).
        active.sort_by(|(addr_a, info_a), (addr_b, info_b)| {
            info_b
                .stake
                .cmp(&info_a.stake)
                .then_with(|| addr_a.cmp(addr_b))
        });

        active
            .into_iter()
            .take(max_committee_size)
            .map(|(addr, _)| addr.clone())
            .collect()
    }

    // ---- Contract helpers --------------------------------------------------

    /// Store a deployed contract state.
    pub fn deploy_contract(&mut self, address: String, state: ContractState) {
        self.contracts.insert(address, state);
    }

    /// Retrieve mutable contract state for execution.
    pub fn get_contract_mut(&mut self, address: &str) -> Option<&mut ContractState> {
        self.contracts.get_mut(address)
    }

    /// Retrieve immutable contract state for reads.
    pub fn get_contract(&self, address: &str) -> Option<&ContractState> {
        self.contracts.get(address)
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
        assert!(
            !tx.verify(),
            "Reserved signature scheme must be rejected by verify()"
        );
    }

    #[test]
    fn test_tampered_amount_invalidates_signature() {
        let kp = FalconKeypair::generate();
        let mut tx = signed_transfer(&kp, 1_000, 1);
        tx.amount = 9_999_999;
        assert!(
            !tx.verify(),
            "Tampering with amount must invalidate signature"
        );
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
        assert_eq!(
            tx.hash(),
            tx.hash(),
            "Transaction hash must be deterministic"
        );
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
        state.accounts.insert(
            address.to_string(),
            AccountBalance {
                address: address.to_string(),
                balance,
                nonce: 0,
                locked_balances: Vec::new(),
            },
        );
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

        assert_eq!(
            root1, root2,
            "State root must be deterministic regardless of insertion order"
        );
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
            payload: vec![],
        };
        assert!(tx.verify(), "Coinbase must bypass signature verification");
    }

    /// A Testnet-signed transaction must be rejected if `network_id` is changed
    /// to Mainnet — proving cross-chain replay protection is in effect.
    #[test]
    fn test_cross_chain_replay_rejected() {
        use crate::core::{MAINNET_NETWORK_ID, TESTNET_NETWORK_ID};
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

    /// An AI Payload must be included in the cryptographic signature.
    /// Tampering with the payload after signing must invalidate the transaction.
    #[test]
    fn test_ai_payload_signature() {
        let kp = FalconKeypair::generate();
        let mut tx = Transaction::new(
            kp.get_address(),
            "0xrecipient".to_string(),
            1_000_000,
            chrono::Utc::now().timestamp(),
        );
        tx.public_key = kp.public_key.clone();
        tx.fee = 1_000;
        tx.nonce = 1;
        tx.payload = b"{\"ai_agent\":\"alpha\",\"task\":\"fetch_data\"}".to_vec();

        let signing_data = tx.get_signing_data();
        tx.signature = kp.sign_transaction_canonical(&signing_data);

        assert!(
            tx.verify(),
            "AI Payload transaction should verify correctly"
        );

        // Tamper with the payload — signature must become invalid
        tx.payload = b"{\"ai_agent\":\"alpha\",\"task\":\"fetch_data_modified\"}".to_vec();
        assert!(!tx.verify(), "Tampered AI Payload must fail verification");
    }
}
