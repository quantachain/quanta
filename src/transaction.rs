use serde::{Serialize, Deserialize};
use crate::crypto::{verify_signature, sha3_hash};
use std::collections::HashMap;

/// Transaction structure with Falcon signature
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub sender: String,           // Sender address (40 hex chars)
    pub recipient: String,        // Recipient address
    pub amount: f64,              // Amount in coins
    pub timestamp: i64,           // Unix timestamp
    pub signature: Vec<u8>,       // Falcon signature (~666 bytes)
    pub public_key: Vec<u8>,      // Falcon public key (~897 bytes)
    pub fee: f64,                 // Transaction fee (0.001 QUA)
}

impl Transaction {
    /// Create a new transaction (unsigned)
    pub fn new(sender: String, recipient: String, amount: f64, timestamp: i64) -> Self {
        Self {
            sender,
            recipient,
            amount,
            timestamp,
            signature: vec![],
            public_key: vec![],
            fee: 0.001, // Fixed fee
        }
    }

    /// Get transaction data for signing (excludes signature and public key)
    pub fn get_signing_data(&self) -> Vec<u8> {
        format!(
            "{}:{}:{}:{}:{}",
            self.sender, self.recipient, self.amount, self.timestamp, self.fee
        )
        .into_bytes()
    }

    /// Verify the Falcon signature of this transaction
    pub fn verify(&self) -> bool {
        if self.signature.is_empty() || self.public_key.is_empty() {
            return false;
        }
        
        let data = self.get_signing_data();
        verify_signature(&data, &self.signature, &self.public_key)
    }

    /// Calculate transaction hash
    pub fn hash(&self) -> String {
        let data = format!(
            "{}:{}:{}:{}",
            self.sender, self.recipient, self.amount, self.timestamp
        );
        hex::encode(sha3_hash(data.as_bytes()))
    }

    /// Check if this is a coinbase transaction (mining reward)
    pub fn is_coinbase(&self) -> bool {
        self.sender == "COINBASE"
    }
}

/// UTXO (Unspent Transaction Output) for balance tracking
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UTXO {
    pub tx_hash: String,
    pub recipient: String,
    pub amount: f64,
    pub spent: bool,
}

/// UTXO Set - tracks all unspent outputs
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UTXOSet {
    utxos: HashMap<String, Vec<UTXO>>,
}

impl UTXOSet {
    pub fn new() -> Self {
        Self {
            utxos: HashMap::new(),
        }
    }

    /// Add a new UTXO from a transaction
    pub fn add_utxo(&mut self, tx: &Transaction) {
        let utxo = UTXO {
            tx_hash: tx.hash(),
            recipient: tx.recipient.clone(),
            amount: tx.amount,
            spent: false,
        };

        self.utxos
            .entry(tx.recipient.clone())
            .or_insert_with(Vec::new)
            .push(utxo);
    }

    /// Mark UTXOs as spent for a given address and amount
    pub fn spend_utxos(&mut self, address: &str, amount: f64) -> bool {
        if let Some(address_utxos) = self.utxos.get_mut(address) {
            let mut remaining = amount;
            let mut to_spend = Vec::new();
            
            // Collect UTXOs to spend
            for (idx, utxo) in address_utxos.iter().enumerate() {
                if !utxo.spent && remaining > 0.0001 {
                    to_spend.push(idx);
                    remaining -= utxo.amount;
                }
            }
            
            // Check if we have enough
            if remaining > 0.0001 {
                return false;
            }
            
            // Mark as spent
            for idx in to_spend {
                address_utxos[idx].spent = true;
            }
            
            true
        } else {
            false
        }
    }

    /// Get balance for an address
    pub fn get_balance(&self, address: &str) -> f64 {
        if let Some(address_utxos) = self.utxos.get(address) {
            address_utxos
                .iter()
                .filter(|utxo| !utxo.spent)
                .map(|utxo| utxo.amount)
                .sum()
        } else {
            0.0
        }
    }

    /// Check if address has sufficient balance
    pub fn has_sufficient_balance(&self, address: &str, amount: f64) -> bool {
        self.get_balance(address) >= amount
    }

    /// Get all UTXOs for an address
    pub fn get_utxos(&self, address: &str) -> Vec<UTXO> {
        if let Some(address_utxos) = self.utxos.get(address) {
            address_utxos
                .iter()
                .filter(|utxo| !utxo.spent)
                .cloned()
                .collect()
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_creation() {
        let tx = Transaction::new(
            "sender123".to_string(),
            "recipient456".to_string(),
            100.0,
            1234567890,
        );
        assert_eq!(tx.amount, 100.0);
        assert_eq!(tx.fee, 0.001);
    }

    #[test]
    fn test_utxo_set() {
        let mut utxo_set = UTXOSet::new();
        
        let tx = Transaction::new(
            "COINBASE".to_string(),
            "address1".to_string(),
            50.0,
            1234567890,
        );
        
        utxo_set.add_utxo(&tx);
        assert_eq!(utxo_set.get_balance("address1"), 50.0);
        
        // Spending marks entire UTXO as spent
        assert!(utxo_set.spend_utxos("address1", 30.0));
        assert_eq!(utxo_set.get_balance("address1"), 0.0); // Entire UTXO spent
    }
}
