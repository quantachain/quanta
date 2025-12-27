use bip39::{Mnemonic, Language};
use sha3::{Sha3_256, Digest};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use rand::RngCore;

type HmacSha256 = Hmac<Sha3_256>;

/// HD Wallet (Hierarchical Deterministic) using BIP39 mnemonic
#[derive(Clone, Serialize, Deserialize)]
pub struct HDWallet {
    pub mnemonic: String,
    pub seed: Vec<u8>,
    pub master_key: Vec<u8>,
    pub accounts: Vec<HDAccount>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub index: u32,
    pub address: String,
    pub public_key: Vec<u8>,
    pub label: Option<String>,
}

impl HDWallet {
    /// Create a new HD wallet with a random mnemonic
    pub fn new() -> Self {
        let mut entropy = [0u8; 32]; // 32 bytes = 256 bits = 24 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        Self::from_mnemonic(mnemonic.to_string())
    }

    /// Create HD wallet from existing mnemonic phrase
    pub fn from_mnemonic(mnemonic_phrase: String) -> Self {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &mnemonic_phrase)
            .expect("Invalid mnemonic phrase");
        
        // Generate seed from mnemonic (with empty passphrase)
        let seed = mnemonic.to_seed("");
        
        // Derive master key from seed
        let master_key = Self::derive_master_key(&seed);
        
        Self {
            mnemonic: mnemonic_phrase,
            seed: seed.to_vec(),
            master_key,
            accounts: Vec::new(),
        }
    }

    /// Derive master key from seed using HMAC-SHA3
    fn derive_master_key(seed: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(b"Quanta HD Wallet Master Key")
            .expect("HMAC initialization failed");
        mac.update(seed);
        mac.finalize().into_bytes().to_vec()
    }

    /// Derive a child key for a specific account index
    fn derive_account_key(&self, index: u32) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(&self.master_key)
            .expect("HMAC initialization failed");
        mac.update(&index.to_be_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// Derive address from account key
    fn derive_address(account_key: &[u8]) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(account_key);
        let hash = hasher.finalize();
        hex::encode(&hash[..20]) // Use first 20 bytes like Ethereum
    }

    /// Generate a new account/address
    pub fn generate_account(&mut self, label: Option<String>) -> HDAccount {
        let index = self.accounts.len() as u32;
        let account_key = self.derive_account_key(index);
        let address = Self::derive_address(&account_key);
        
        // For public key, we'll use a portion of the derived key
        // In production, you'd generate actual Falcon keys per account
        let public_key = account_key[..32].to_vec();
        
        let account = HDAccount {
            index,
            address,
            public_key,
            label,
        };
        
        self.accounts.push(account.clone());
        account
    }

    /// Get account by index
    pub fn get_account(&self, index: u32) -> Option<&HDAccount> {
        self.accounts.iter().find(|a| a.index == index)
    }

    /// Get all accounts
    pub fn get_accounts(&self) -> &[HDAccount] {
        &self.accounts
    }

    /// Restore wallet from mnemonic and regenerate accounts
    pub fn restore(mnemonic_phrase: String, account_count: u32) -> Self {
        let mut wallet = Self::from_mnemonic(mnemonic_phrase);
        
        for i in 0..account_count {
            wallet.generate_account(Some(format!("Account {}", i)));
        }
        
        wallet
    }

    /// Export wallet data (encrypted)
    pub fn export_encrypted(&self, password: &str) -> Result<Vec<u8>, String> {
        use chacha20poly1305::{ChaCha20Poly1305, KeyInit, AeadCore};
        use chacha20poly1305::aead::{Aead, OsRng};
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::SaltString;
        
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Hashing failed: {}", e))?;
        
        let key_bytes = password_hash.hash.unwrap();
        let key = &key_bytes.as_bytes()[..32];
        
        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| format!("Cipher creation failed: {}", e))?;
        
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
        
        let wallet_data = serde_json::to_vec(self)
            .map_err(|e| format!("Serialization failed: {}", e))?;
        
        let ciphertext = cipher
            .encrypt(&nonce, wallet_data.as_ref())
            .map_err(|e| format!("Encryption failed: {}", e))?;
        
        // Combine salt + nonce + ciphertext
        let mut result = salt.as_str().as_bytes().to_vec();
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }

    /// Display wallet info
    pub fn display_info(&self) {
        println!("\n╔════════════════════════════════════════════════════════════╗");
        println!("║              HD WALLET INFORMATION                         ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║ Mnemonic Phrase (24 words):                                ║");
        println!("║ {}   ║", self.mnemonic);
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║ Accounts: {}                                              ║", self.accounts.len());
        println!("╠════════════════════════════════════════════════════════════╣");
        
        for account in &self.accounts {
            let label = account.label.as_deref().unwrap_or("Unnamed");
            println!("║ {} (#{})                                      ║", label, account.index);
            println!("║ Address: {}...{}      ║", 
                &account.address[..10], 
                &account.address[account.address.len()-6..]
            );
        }
        
        println!("╚════════════════════════════════════════════════════════════╝");
        println!("\n⚠️  IMPORTANT: Keep your mnemonic phrase safe!");
        println!("   It can restore your entire wallet and all accounts.");
        println!("   Never share it with anyone!\n");
    }
}

impl Default for HDWallet {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hd_wallet_creation() {
        let wallet = HDWallet::new();
        assert!(!wallet.mnemonic.is_empty());
        assert!(!wallet.seed.is_empty());
    }

    #[test]
    fn test_account_generation() {
        let mut wallet = HDWallet::new();
        let account = wallet.generate_account(Some("Test Account".to_string()));
        assert_eq!(account.index, 0);
        assert!(!account.address.is_empty());
    }

    #[test]
    fn test_multiple_accounts() {
        let mut wallet = HDWallet::new();
        wallet.generate_account(Some("Account 1".to_string()));
        wallet.generate_account(Some("Account 2".to_string()));
        wallet.generate_account(Some("Account 3".to_string()));
        
        assert_eq!(wallet.accounts.len(), 3);
        assert_ne!(wallet.accounts[0].address, wallet.accounts[1].address);
    }

    #[test]
    fn test_wallet_restoration() {
        let wallet1 = HDWallet::new();
        let mnemonic = wallet1.mnemonic.clone();
        
        let wallet2 = HDWallet::from_mnemonic(mnemonic);
        assert_eq!(wallet1.seed, wallet2.seed);
        assert_eq!(wallet1.master_key, wallet2.master_key);
    }

    #[test]
    fn test_deterministic_derivation() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art";
        let wallet1 = HDWallet::from_mnemonic(mnemonic.to_string());
        let wallet2 = HDWallet::from_mnemonic(mnemonic.to_string());
        
        assert_eq!(wallet1.seed, wallet2.seed);
        assert_eq!(wallet1.master_key, wallet2.master_key);
    }
}
