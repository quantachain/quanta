use bip39::{Mnemonic, Language};
use sha3::{Sha3_256, Digest};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use rand::RngCore;
use zeroize::Zeroize;

type HmacSha256 = Hmac<Sha3_256>;

/// HD Wallet (Hierarchical Deterministic) using BIP39 mnemonic
#[derive(Clone, Serialize, Deserialize)]
pub struct HDWallet {
    pub mnemonic: String,
    #[serde(skip)] // Don't serialize seed (security)
    pub seed: Vec<u8>,
    #[serde(skip)] // Don't serialize master key (security)
    pub master_key: Vec<u8>,
    pub accounts: Vec<HDAccount>,
    #[serde(skip)] // Optional passphrase (13th/25th word)
    pub passphrase: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub index: u32,
    pub address: String,
    pub public_key: Vec<u8>,
    pub label: Option<String>,
}

impl Drop for HDWallet {
    fn drop(&mut self) {
        // Zeroize sensitive data on drop
        self.seed.zeroize();
        self.master_key.zeroize();
        self.passphrase.zeroize();
    }
}

impl HDWallet {
    /// Create a new HD wallet with a random mnemonic
    pub fn new() -> Self {
        let mut entropy = [0u8; 32]; // 32 bytes = 256 bits = 24 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        Self::from_mnemonic(mnemonic.to_string(), "")
    }

    /// Create HD wallet from existing mnemonic phrase (with optional passphrase)
    /// The passphrase acts as a 13th/25th word, providing plausible deniability
    pub fn from_mnemonic(mnemonic_phrase: String, passphrase: &str) -> Self {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &mnemonic_phrase)
            .expect("Invalid mnemonic phrase");
        
        // Generate seed from mnemonic (with passphrase for plausible deniability)
        let seed = mnemonic.to_seed(passphrase);
        
        // Derive master key from seed
        let master_key = Self::derive_master_key(&seed);
        
        Self {
            mnemonic: mnemonic_phrase,
            seed: seed.to_vec(),
            master_key,
            accounts: Vec::new(),
            passphrase: passphrase.to_string(),
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
        
        // TODO: SECURITY - Generate real Falcon-512 keypair per account
        // This is a PLACEHOLDER. In production:
        // 1. Use account_key as seed for Falcon key generation
        // 2. Generate actual Falcon private + public key pair
        // 3. Store private key encrypted, public key here
        // 4. Sign with actual Falcon key, not this derived stub
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
    pub fn restore(mnemonic_phrase: String, passphrase: &str, account_count: u32) -> Self {
        let mut wallet = Self::from_mnemonic(mnemonic_phrase, passphrase);
        
        for i in 0..account_count {
            wallet.generate_account(Some(format!("Account {}", i)));
        }
        
        wallet
    }

    /// Export wallet data (encrypted with proper format)
    /// Format: [salt_len:4][salt][nonce:12][ciphertext]
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
        
        // CRITICAL: Proper structured format with length prefix
        // [salt_len:4][salt][nonce:12][ciphertext]
        let salt_bytes = salt.as_str().as_bytes();
        let salt_len = salt_bytes.len() as u32;
        
        let mut result = Vec::new();
        result.extend_from_slice(&salt_len.to_le_bytes()); // 4 bytes length prefix
        result.extend_from_slice(salt_bytes);              // variable salt
        result.extend_from_slice(&nonce);                  // 12 bytes nonce
        result.extend_from_slice(&ciphertext);             // variable ciphertext
        
        Ok(result)
    }
    
    /// Import wallet from encrypted data
    pub fn import_encrypted(encrypted_data: &[u8], password: &str) -> Result<Self, String> {
        use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
        use chacha20poly1305::aead::Aead;
        use argon2::{Argon2, PasswordHasher};
        use argon2::password_hash::SaltString;
        
        if encrypted_data.len() < 4 {
            return Err("Invalid encrypted data: too short".into());
        }
        
        // Parse format: [salt_len:4][salt][nonce:12][ciphertext]
        let salt_len = u32::from_le_bytes([
            encrypted_data[0],
            encrypted_data[1],
            encrypted_data[2],
            encrypted_data[3],
        ]) as usize;
        
        if encrypted_data.len() < 4 + salt_len + 12 {
            return Err("Invalid encrypted data: truncated".into());
        }
        
        let salt_bytes = &encrypted_data[4..4 + salt_len];
        let salt_str = std::str::from_utf8(salt_bytes)
            .map_err(|_| "Invalid salt encoding")?;
        let salt = SaltString::from_b64(salt_str)
            .map_err(|e| format!("Invalid salt: {}", e))?;
        
        let nonce_start = 4 + salt_len;
        let nonce = &encrypted_data[nonce_start..nonce_start + 12];
        let ciphertext = &encrypted_data[nonce_start + 12..];
        
        // Derive key from password
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Hashing failed: {}", e))?;
        
        let key_bytes = password_hash.hash.unwrap();
        let key = &key_bytes.as_bytes()[..32];
        
        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| format!("Cipher creation failed: {}", e))?;
        
        // Decrypt
        let plaintext = cipher
            .decrypt(nonce.into(), ciphertext)
            .map_err(|_| "Decryption failed: wrong password or corrupted data")?;
        
        // Deserialize wallet
        let mut wallet: HDWallet = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Deserialization failed: {}", e))?;
        
        // Regenerate seed and master_key (they were skipped in serialization)
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &wallet.mnemonic)
            .map_err(|e| format!("Invalid mnemonic: {}", e))?;
        wallet.seed = mnemonic.to_seed(&wallet.passphrase).to_vec();
        wallet.master_key = Self::derive_master_key(&wallet.seed);
        
        Ok(wallet)
    }

    /// Display wallet info
    pub fn display_info(&self) {
        println!("\n╔════════════════════════════════════════════════════════════╗");
        println!("║              HD WALLET INFORMATION                         ║");
        println!("╠════════════════════════════════════════════════════════════╣");
        println!("║ Mnemonic Phrase (24 words):                                ║");
        println!("║ {}   ║", self.mnemonic);
        if !self.passphrase.is_empty() {
            println!("║ Passphrase: [SET] (plausible deniability enabled)         ║");
        }
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

