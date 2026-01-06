use serde::{Serialize, Deserialize};
use crate::crypto::verify_signature;
use std::collections::HashMap;

/// Transaction structure with Falcon signature
/// Amount is in microunits (1 QUA = 1_000_000 microunits)
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub sender: String,           // Sender address (derived from public_key)
    pub recipient: String,        // Recipient address
    pub amount: u64,              // Amount in microunits (1 QUA = 1_000_000)
    pub timestamp: i64,           // Unix timestamp
    pub signature: Vec<u8>,       // Falcon signature (~666 bytes)
    pub public_key: Vec<u8>,      // Falcon public key (~897 bytes)
    pub fee: u64,                 // Transaction fee in microunits
    pub nonce: u64,               // Nonce for replay protection
    pub tx_type: TransactionType, // Transaction type
}

/// Transaction types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransactionType {
    Transfer,
    DeployContract { code: Vec<u8> },
    CallContract { contract: String, function: String, args: Vec<u8> },
}

impl Transaction {
    /// Create a new transaction (unsigned) - amounts in microunits
    pub fn new(sender: String, recipient: String, amount: u64, timestamp: i64) -> Self {
        Self {
            sender,
            recipient,
            amount,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 1000, // 0.001 QUA = 1000 microunits
            nonce: 0,
            tx_type: TransactionType::Transfer,
        }
    }
    
    /// Create deploy contract transaction
    #[allow(dead_code)]
    pub fn new_deploy_contract(sender: String, code: Vec<u8>, timestamp: i64, nonce: u64) -> Self {
        Self {
            sender,
            recipient: String::new(),
            amount: 0,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 10_000, // 0.01 QUA for deployment
            nonce,
            tx_type: TransactionType::DeployContract { code },
        }
    }
    
    /// Create call contract transaction
    #[allow(dead_code)]
    pub fn new_call_contract(
        sender: String,
        contract: String,
        function: String,
        args: Vec<u8>,
        timestamp: i64,
        nonce: u64,
    ) -> Self {
        Self {
            sender,
            recipient: contract.clone(),
            amount: 0,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 5000, // 0.005 QUA for calls
            nonce,
            tx_type: TransactionType::CallContract { contract, function, args },
        }
    }

    /// Get transaction data for signing - MUST match hash calculation
    /// Everything except signature itself
    /// 
    /// CONSENSUS RULES (FROZEN FOREVER):
    /// - All integers are LITTLE-ENDIAN (to_le_bytes)
    /// - Public key is included (binds signature to key, prevents key substitution)
    /// - Strings are UTF-8 bytes
    pub fn get_signing_data(&self) -> Vec<u8> {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        
        // CRITICAL: This must match hash() exactly (except signature)
        hasher.update(self.sender.as_bytes());
        hasher.update(self.recipient.as_bytes());
        hasher.update(&self.amount.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.timestamp.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.fee.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.nonce.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.public_key);
        
        // Include tx_type
        match &self.tx_type {
            TransactionType::Transfer => hasher.update(&[0u8]),
            TransactionType::DeployContract { code } => {
                hasher.update(&[1u8]);
                hasher.update(code);
            }
            TransactionType::CallContract { contract, function, args } => {
                hasher.update(&[2u8]);
                hasher.update(contract.as_bytes());
                hasher.update(function.as_bytes());
                hasher.update(args);
            }
        }
        
        hasher.finalize().to_vec()
    }

    /// Verify the Falcon signature AND sender matches public_key
    /// Special case: coinbase transactions bypass signature verification
    pub fn verify(&self) -> bool {
        // Coinbase transactions are verified by consensus rules, not signatures
        if self.is_coinbase() {
            return true; // Coinbase validity checked elsewhere (block reward rules)
        }
        
        if self.signature.is_empty() || self.public_key.is_empty() {
            return false;
        }
        
        // CRITICAL: Verify sender matches the public key
        let derived_address = self.derive_address_from_pubkey();
        if self.sender != derived_address {
            tracing::warn!("Sender mismatch: {} != {}", self.sender, derived_address);
            return false;
        }
        
        let data = self.get_signing_data();
        verify_signature(&data, &self.signature, &self.public_key)
    }
    
    /// Derive address from public key (must match sender)
    fn derive_address_from_pubkey(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let hash = Sha3_256::digest(&self.public_key);
        format!("0x{}", hex::encode(&hash[..20])) // 0x + 40 hex chars = 42 total
    }

    /// Calculate transaction hash - includes ALL fields except signature
    /// This prevents hash collisions and replay attacks
    /// 
    /// CONSENSUS RULES (FROZEN FOREVER):
    /// - All integers are LITTLE-ENDIAN
    /// - Public key included (prevents key substitution attacks)
    /// - Signature NOT included (can't sign the signature)
    pub fn hash(&self) -> String {
        use sha3::{Digest, Sha3_256};
        let mut hasher = Sha3_256::new();
        
        // Include all transaction data EXCEPT signature (signature signs the hash)
        hasher.update(self.sender.as_bytes());
        hasher.update(self.recipient.as_bytes());
        hasher.update(&self.amount.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.timestamp.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.fee.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.nonce.to_le_bytes()); // LITTLE-ENDIAN
        hasher.update(&self.public_key);
        
        // Include tx_type discriminant
        match &self.tx_type {
            TransactionType::Transfer => hasher.update(&[0u8]),
            TransactionType::DeployContract { code } => {
                hasher.update(&[1u8]);
                hasher.update(code);
            }
            TransactionType::CallContract { contract, function, args } => {
                hasher.update(&[2u8]);
                hasher.update(contract.as_bytes());
                hasher.update(function.as_bytes());
                hasher.update(args);
            }
        }
        
        hex::encode(hasher.finalize())
    }

    /// Check if this is a coinbase transaction (mining reward)
    pub fn is_coinbase(&self) -> bool {
        self.sender == "COINBASE"
    }
}

/// Account balance tracking (account-based model, not UTXO)
/// This is simpler and works better with smart contracts
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountBalance {
    pub address: String,
    pub balance: u64,        // in microunits (spendable)
    pub nonce: u64,          // for replay protection
    pub locked_balance: u64, // coinbase rewards locked until maturity
    pub unlock_height: u64,  // block height when locked_balance becomes spendable
}

/// Account state database (account-based model, NOT UTXO)
/// Tracks balance + nonce for each address
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountState {
    accounts: HashMap<String, AccountBalance>,
}

impl AccountState {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
        }
    }

    /// Credit account from transaction (add balance)
    /// For coinbase: locked until maturity height
    /// For regular: immediately spendable
    pub fn credit_account(&mut self, tx: &Transaction, current_height: u64, coinbase_maturity: u64) {
        if tx.amount == 0 {
            return; // Skip zero-amount txs (like contract calls)
        }
        
        let account = self.accounts.entry(tx.recipient.clone()).or_insert(AccountBalance {
            address: tx.recipient.clone(),
            balance: 0,
            nonce: 0,
            locked_balance: 0,
            unlock_height: 0,
        });
        
        if tx.is_coinbase() {
            // Coinbase rewards are locked until maturity
            account.locked_balance = account.locked_balance.saturating_add(tx.amount);
            account.unlock_height = current_height + coinbase_maturity;
        } else {
            // Regular transactions are immediately spendable
            account.balance = account.balance.saturating_add(tx.amount);
        }
    }

    /// Debit account (spend balance + fee)
    /// Returns true if successful, false if insufficient funds
    pub fn debit_account(&mut self, address: &str, total_amount: u64) -> bool {
        if let Some(account) = self.accounts.get_mut(address) {
            if account.balance >= total_amount {
                account.balance -= total_amount;
                account.nonce += 1; // Increment nonce on spend
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// Unlock mature coinbase rewards (called at each new block)
    pub fn unlock_mature_coinbase(&mut self, current_height: u64) {
        for account in self.accounts.values_mut() {
            if account.locked_balance > 0 && current_height >= account.unlock_height {
                account.balance = account.balance.saturating_add(account.locked_balance);
                account.locked_balance = 0;
                account.unlock_height = 0;
            }
        }
    }
    
    /// Add locked balance for mining reward vesting (ANTI-DUMP mechanism)
    /// Used for 50% of mining rewards locked for 6 months
    pub fn add_locked_balance(&mut self, address: &str, amount: u64, unlock_height: u64) {
        let account = self.accounts.entry(address.to_string()).or_insert(AccountBalance {
            address: address.to_string(),
            balance: 0,
            nonce: 0,
            locked_balance: 0,
            unlock_height: 0,
        });
        
        // Add to locked balance with max unlock height
        account.locked_balance = account.locked_balance.saturating_add(amount);
        account.unlock_height = account.unlock_height.max(unlock_height);
    }

    /// Get balance for an address (spendable only)
    pub fn get_balance(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.balance).unwrap_or(0)
    }
    
    /// Get total balance (spendable + locked)
    pub fn get_total_balance(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.balance + acc.locked_balance).unwrap_or(0)
    }
    
    /// Get account nonce
    pub fn get_nonce(&self, address: &str) -> u64 {
        self.accounts.get(address).map(|acc| acc.nonce).unwrap_or(0)
    }
    
    /// Increment nonce for account (CRITICAL for transaction ordering)
    pub fn increment_nonce(&mut self, address: &str) {
        if let Some(acc) = self.accounts.get_mut(address) {
            acc.nonce += 1;
        } else {
            // Create account with nonce 1 if doesn't exist
            self.accounts.insert(address.to_string(), AccountBalance {
                address: address.to_string(),
                balance: 0,
                nonce: 1,
                locked_balance: 0,
                unlock_height: 0,
            });
        }
    }
    
    /// Verify transaction nonce matches account nonce
    pub fn verify_nonce(&self, address: &str, tx_nonce: u64) -> bool {
        let account_nonce = self.get_nonce(address);
        tx_nonce == account_nonce + 1 || (account_nonce == 0 && tx_nonce == 1)
    }

    /// Check if address has sufficient balance
    pub fn has_sufficient_balance(&self, address: &str, amount: u64) -> bool {
        self.get_balance(address) >= amount
    }

    /// Get all account addresses
    pub fn get_accounts(&self) -> Vec<String> {
        self.accounts.keys().cloned().collect()
    }
}

