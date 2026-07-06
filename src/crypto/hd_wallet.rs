use crate::crypto::signatures::FalconKeypair;
use bip39::{Language, Mnemonic};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce as ChaCha20Nonce,
};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use zeroize::{Zeroize, Zeroizing};

type HmacSha256 = Hmac<Sha3_256>;

/// HD Wallet (Hierarchical Deterministic) using BIP39 mnemonic.
///
/// Each account holds a REAL Falcon-512 keypair. The secret key is encrypted
/// with ChaCha20-Poly1305 under an account-specific key derived from the HD
/// master key: `account_key = HMAC-SHA3(master_key, index_be)`.
///
/// A single 24-word mnemonic restores all accounts and their private keys.
#[derive(Clone, Serialize, Deserialize)]
pub struct HDWallet {
    pub mnemonic: String,
    #[serde(skip)] // Never serialise — regenerated from mnemonic on load
    pub seed: Vec<u8>,
    #[serde(skip)] // Never serialise — regenerated from seed on load
    pub master_key: Vec<u8>,
    pub accounts: Vec<HDAccount>,
    #[serde(skip)] // Never serialise — re-entered by user on load
    pub passphrase: String,
}

/// One account within an HDWallet.
///
/// `public_key` is stored in plaintext (needed for address derivation and
/// signature verification). `encrypted_secret_key` + `sk_nonce` contain the
/// Falcon-512 secret key encrypted with ChaCha20-Poly1305; the key is
/// `HMAC-SHA3(master_key, account_index_be)`.
#[derive(Clone, Serialize, Deserialize)]
pub struct HDAccount {
    pub index: u32,
    /// Quanta address: `"0x" + hex(SHA3-256(falcon_pubkey)[..20])`
    pub address: String,
    /// Falcon-512 public key (897 bytes), stored in plaintext.
    pub public_key: Vec<u8>,
    pub label: Option<String>,
    /// ChaCha20-Poly1305 ciphertext of the Falcon-512 secret key bytes.
    /// Empty on legacy/placeholder accounts — regenerate to produce a real keypair.
    #[serde(default)]
    pub encrypted_secret_key: Vec<u8>,
    /// 12-byte random nonce for the ChaCha20-Poly1305 encryption above.
    #[serde(default)]
    pub sk_nonce: Vec<u8>,
}

impl Drop for HDWallet {
    fn drop(&mut self) {
        self.seed.zeroize();
        self.master_key.zeroize();
        self.passphrase.zeroize();
    }
}

impl HDWallet {
    /// Create a new HD wallet with a cryptographically-random 24-word mnemonic.
    pub fn new() -> Self {
        let mut entropy = [0u8; 32]; // 256 bits → 24 BIP39 words
        rand::thread_rng().fill_bytes(&mut entropy);
        let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy).unwrap();
        Self::from_mnemonic(mnemonic.to_string(), "")
    }

    /// Create HD wallet from an existing mnemonic phrase.
    ///
    /// `passphrase` is the optional BIP39 passphrase ("25th word") for
    /// plausible deniability. Empty string means no passphrase.
    pub fn from_mnemonic(mnemonic_phrase: String, passphrase: &str) -> Self {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &mnemonic_phrase)
            .expect("Invalid mnemonic phrase");
        let seed = mnemonic.to_seed(passphrase);
        let master_key = Self::derive_master_key(&seed);
        Self {
            mnemonic: mnemonic_phrase,
            seed: seed.to_vec(),
            master_key,
            accounts: Vec::new(),
            passphrase: passphrase.to_string(),
        }
    }

    /// `master_key = HMAC-SHA3-256("Quanta HD Wallet Master Key", seed)`
    fn derive_master_key(seed: &[u8]) -> Vec<u8> {
        let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(b"Quanta HD Wallet Master Key")
            .expect("HMAC initialization failed");
        mac.update(seed);
        mac.finalize().into_bytes().to_vec()
    }

    /// `account_key = HMAC-SHA3-256(master_key, index_be_bytes)`
    fn derive_account_key(&self, index: u32) -> Vec<u8> {
        let mut mac = <HmacSha256 as hmac::Mac>::new_from_slice(&self.master_key)
            .expect("HMAC initialization failed");
        mac.update(&index.to_be_bytes());
        mac.finalize().into_bytes().to_vec()
    }

    /// Generate a new account with a REAL Falcon-512 keypair.
    ///
    /// The secret key is encrypted with the account-derived key (ChaCha20-Poly1305)
    /// and stored in `HDAccount::encrypted_secret_key`. Call `get_keypair(index)`
    /// to decrypt and use the keypair for transaction signing.
    pub fn generate_account(&mut self, label: Option<String>) -> HDAccount {
        let index = self.accounts.len() as u32;
        let account_key = self.derive_account_key(index);

        // --- Generate a real Falcon-512 keypair deterministically ---
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&account_key);
        let keypair = FalconKeypair::generate_from_seed(seed);
        let public_key = keypair.public_key.clone();

        // Derive the address consistently with QuantumWallet::get_address():
        // "0x" + hex(SHA3-256(falcon_pubkey)[..20])
        let pk_hash = Sha3_256::digest(&public_key);
        let address = format!("0x{}", hex::encode(&pk_hash[..20]));

        // --- Encrypt the Falcon-512 secret key under the account-derived key ---
        // account_key is 32 bytes (HMAC-SHA3-256 output) — perfect for ChaCha20-Poly1305.
        let cipher = ChaCha20Poly1305::new_from_slice(&account_key)
            .expect("32-byte account key is always valid for ChaCha20-Poly1305");
        let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);

        let encrypted_secret_key = cipher
            .encrypt(&nonce, keypair.secret_key_bytes())
            .expect("in-memory ChaCha20-Poly1305 encryption must not fail");

        let account = HDAccount {
            index,
            address,
            public_key,
            label,
            encrypted_secret_key,
            sk_nonce: nonce.to_vec(),
        };

        self.accounts.push(account.clone());
        account
    }

    /// Decrypt and return the Falcon-512 keypair for `index`.
    ///
    /// The keypair is fully reconstructed (both signing key and public key).
    /// Secret key bytes are zeroized when the returned `FalconKeypair` is
    /// dropped — callers should drop it as soon as possible after signing.
    ///
    /// # Errors
    /// - Account index not found
    /// - Legacy placeholder account (no encrypted SK stored)
    /// - Decryption failure (wrong wallet or corrupted data)
    #[allow(dead_code)]
    pub fn get_keypair(&self, index: u32) -> Result<FalconKeypair, String> {
        let account = self
            .accounts
            .iter()
            .find(|a| a.index == index)
            .ok_or_else(|| format!("Account {} not found in HD wallet", index))?;

        if account.encrypted_secret_key.is_empty() {
            return Err(format!(
                "Account {} has no encrypted secret key (legacy placeholder). \
                 Regenerate this account to create a real Falcon-512 keypair.",
                index
            ));
        }

        if account.sk_nonce.len() != 12 {
            return Err(format!(
                "Account {} has a malformed nonce ({} bytes, expected 12)",
                index,
                account.sk_nonce.len()
            ));
        }

        let account_key = self.derive_account_key(index);
        let cipher = ChaCha20Poly1305::new_from_slice(&account_key)
            .map_err(|_| "Failed to initialise decryption cipher".to_string())?;

        let nonce = ChaCha20Nonce::from_slice(&account.sk_nonce);

        let mut sk_bytes = Zeroizing::new(
            cipher
                .decrypt(nonce, account.encrypted_secret_key.as_ref())
                .map_err(|_| {
                    "Secret key decryption failed — wallet may be corrupted or \
                     master key does not match this account"
                        .to_string()
                })?,
        );

        let keypair = FalconKeypair::from_secret_key_bytes(&sk_bytes, &account.public_key);
        sk_bytes.zeroize();
        keypair
    }

    /// Get account by index.
    #[allow(dead_code)]
    pub fn get_account(&self, index: u32) -> Option<&HDAccount> {
        self.accounts.iter().find(|a| a.index == index)
    }

    /// Get all accounts.
    #[allow(dead_code)]
    pub fn get_accounts(&self) -> &[HDAccount] {
        &self.accounts
    }

    /// Restore wallet from mnemonic and regenerate N accounts.
    ///
    /// Because each account's secret key is re-encrypted from the same
    /// master key, restore produces identical addresses and keypairs.
    #[allow(dead_code)]
    pub fn restore(mnemonic_phrase: String, passphrase: &str, account_count: u32) -> Self {
        let mut wallet = Self::from_mnemonic(mnemonic_phrase, passphrase);
        for i in 0..account_count {
            wallet.generate_account(Some(format!("Account {}", i)));
        }
        wallet
    }

    /// Export wallet to encrypted bytes (Argon2 + ChaCha20-Poly1305).
    ///
    /// Format: `[salt_len:4LE][salt][nonce:12][ciphertext]`
    ///
    /// The ciphertext contains the full serialised wallet JSON including
    /// per-account encrypted secret keys (double-encrypted: once by the HD
    /// derivation key, again by the export password).
    pub fn export_encrypted(&self, password: &str) -> Result<Vec<u8>, String> {
        use argon2::password_hash::SaltString;
        use argon2::{Argon2, PasswordHasher};

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

        let wallet_data =
            serde_json::to_vec(self).map_err(|e| format!("Serialization failed: {}", e))?;

        let ciphertext = cipher
            .encrypt(&nonce, wallet_data.as_ref())
            .map_err(|e| format!("Encryption failed: {}", e))?;

        let salt_bytes = salt.as_str().as_bytes();
        let salt_len = salt_bytes.len() as u32;

        let mut result = Vec::new();
        result.extend_from_slice(&salt_len.to_le_bytes());
        result.extend_from_slice(salt_bytes);
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    /// Import wallet from bytes produced by `export_encrypted`.
    pub fn import_encrypted(encrypted_data: &[u8], password: &str) -> Result<Self, String> {
        use argon2::password_hash::SaltString;
        use argon2::{Argon2, PasswordHasher};

        if encrypted_data.len() < 4 {
            return Err("Invalid encrypted data: too short".into());
        }

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
        let salt_str = std::str::from_utf8(salt_bytes).map_err(|_| "Invalid salt encoding")?;
        let salt = SaltString::from_b64(salt_str).map_err(|e| format!("Invalid salt: {}", e))?;

        let nonce_start = 4 + salt_len;
        let nonce = &encrypted_data[nonce_start..nonce_start + 12];
        let ciphertext = &encrypted_data[nonce_start + 12..];

        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| format!("Hashing failed: {}", e))?;

        let key_bytes = password_hash.hash.unwrap();
        let key = &key_bytes.as_bytes()[..32];

        let cipher = ChaCha20Poly1305::new_from_slice(key)
            .map_err(|e| format!("Cipher creation failed: {}", e))?;

        let plaintext = cipher
            .decrypt(nonce.into(), ciphertext)
            .map_err(|_| "Decryption failed: wrong password or corrupted data")?;

        let mut wallet: HDWallet = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("Deserialization failed: {}", e))?;

        // Regenerate seed and master_key (skipped during serialization).
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, &wallet.mnemonic)
            .map_err(|e| format!("Invalid mnemonic in wallet file: {}", e))?;
        wallet.seed = mnemonic.to_seed(&wallet.passphrase).to_vec();
        wallet.master_key = Self::derive_master_key(&wallet.seed);

        Ok(wallet)
    }

    /// Display wallet info.
    pub fn display_info(&self) {
        println!("\n");
        println!("              HD WALLET INFORMATION                         ");
        println!();
        println!(" Mnemonic (24 words):                                       ");
        println!(" {}   ", self.mnemonic);
        if !self.passphrase.is_empty() {
            println!(" Passphrase: [SET] (plausible deniability enabled)         ");
        }
        println!();
        println!(
            " Accounts: {}                                               ",
            self.accounts.len()
        );
        println!();

        for account in &self.accounts {
            let label = account.label.as_deref().unwrap_or("Unnamed");
            let sk_status = if account.encrypted_secret_key.is_empty() {
                "⚠  legacy placeholder"
            } else {
                "✓  real Falcon-512 keypair"
            };
            println!(" {} (#{}): {}", label, account.index, sk_status);
            let addr = &account.address;
            let tail = addr.len().saturating_sub(6);
            println!(
                " Address: {}...{}      ",
                &addr[..12.min(addr.len())],
                &addr[tail..]
            );
            println!(" Pub Key: {} bytes", account.public_key.len());
            println!();
        }

        println!("\n  IMPORTANT: Keep your mnemonic phrase safe!");
        println!("   It can restore your entire wallet and all accounts.");
        println!("   Never share it with anyone!\n");
    }
}

impl Default for HDWallet {
    fn default() -> Self {
        Self::new()
    }
}
