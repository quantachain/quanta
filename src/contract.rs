/// Quantum Smart Contract System (Quasar Framework)
/// Account-based model with quantum-safe primitives

use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use std::collections::HashMap;

/// Account types in the Quasar system
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AccountType {
    /// Regular user account
    User,
    /// Smart contract account with executable code
    Program,
    /// Data account owned by a program
    ProgramData,
}

/// Account information (like Solana's AccountInfo)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Account {
    /// Account public key (address)
    pub key: String,
    /// Quantum-safe public key (Falcon-512)
    pub quantum_key: Vec<u8>,
    /// Balance in QUA tokens
    pub balance: u64,
    /// Stored data (state for contracts, balance for users)
    pub data: Vec<u8>,
    /// Owner program (for data accounts)
    pub owner: String,
    /// Whether account is a program
    pub account_type: AccountType,
    /// Whether this account is executable
    pub executable: bool,
    /// Rent epoch for account storage
    pub rent_epoch: u64,
}

impl Account {
    /// Create new user account
    pub fn new_user(key: String, quantum_key: Vec<u8>, balance: u64) -> Self {
        Self {
            key,
            quantum_key,
            balance,
            data: Vec::new(),
            owner: "system".to_string(),
            account_type: AccountType::User,
            executable: false,
            rent_epoch: 0,
        }
    }

    /// Create new program account
    pub fn new_program(key: String, code: Vec<u8>, owner: String) -> Self {
        Self {
            key,
            quantum_key: Vec::new(),
            balance: 0,
            data: code,
            owner,
            account_type: AccountType::Program,
            executable: true,
            rent_epoch: 0,
        }
    }

    /// Create program data account
    pub fn new_program_data(key: String, owner: String) -> Self {
        Self {
            key,
            quantum_key: Vec::new(),
            balance: 0,
            data: Vec::new(),
            owner,
            account_type: AccountType::ProgramData,
            executable: false,
            rent_epoch: 0,
        }
    }

    /// Get account size in bytes
    pub fn size(&self) -> usize {
        self.data.len() + self.quantum_key.len() + 200 // Approximate overhead
    }
}

/// Contract instruction - similar to Solana's Instruction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractInstruction {
    /// Program account to execute
    pub program_id: String,
    /// Accounts involved in the instruction
    pub accounts: Vec<AccountMeta>,
    /// Instruction data (function selector + args)
    pub data: Vec<u8>,
}

/// Account metadata for instructions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountMeta {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

impl AccountMeta {
    pub fn new(pubkey: String, is_signer: bool, is_writable: bool) -> Self {
        Self {
            pubkey,
            is_signer,
            is_writable,
        }
    }

    pub fn new_readonly(pubkey: String, is_signer: bool) -> Self {
        Self::new(pubkey, is_signer, false)
    }
}

/// Contract execution context
#[derive(Debug)]
pub struct ExecutionContext<'a> {
    /// Program being executed
    pub program_id: String,
    /// Accounts passed to the instruction
    pub accounts: Vec<&'a mut Account>,
    /// Instruction data
    pub data: Vec<u8>,
    /// Block height
    pub block_height: u64,
    /// Quantum entropy seed for randomness
    pub quantum_entropy: [u8; 32],
}

/// Contract execution result
#[derive(Debug, Serialize, Deserialize)]
pub enum ContractResult {
    Success { logs: Vec<String> },
    Error { code: u32, message: String },
}

/// Program Derived Address (PDA) - quantum-safe version
pub struct QuantumPDA;

impl QuantumPDA {
    /// Find program address using quantum-safe hashing
    pub fn find_program_address(seeds: &[&[u8]], program_id: &str) -> (String, u8) {
        for bump in 0..=255u8 {
            let mut hasher = Sha3_256::new();
            for seed in seeds {
                hasher.update(seed);
            }
            hasher.update(&[bump]);
            hasher.update(program_id.as_bytes());
            hasher.update(b"QuantumPDA");

            let hash = hasher.finalize();
            let address = hex::encode(hash);
            
            // Check if this is a valid PDA (not a user-controlled key)
            // In practice, we'd verify this doesn't collide with ed25519/Falcon keys
            return (address, bump);
        }
        
        panic!("Unable to find valid program address");
    }
}

/// Contract state storage
pub struct ContractStorage {
    /// All accounts in the system
    accounts: HashMap<String, Account>,
    /// Program code cache
    program_cache: HashMap<String, Vec<u8>>,
}

impl ContractStorage {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            program_cache: HashMap::new(),
        }
    }

    /// Get account by key
    pub fn get_account(&self, key: &str) -> Option<&Account> {
        self.accounts.get(key)
    }

    /// Get mutable account
    pub fn get_account_mut(&mut self, key: &str) -> Option<&mut Account> {
        self.accounts.get_mut(key)
    }

    /// Create or update account
    pub fn set_account(&mut self, key: String, account: Account) {
        self.accounts.insert(key, account);
    }

    /// Deploy a program
    pub fn deploy_program(&mut self, program_id: String, code: Vec<u8>, deployer: String) -> Result<(), String> {
        // Create program account
        let program_account = Account::new_program(program_id.clone(), code.clone(), deployer);
        self.accounts.insert(program_id.clone(), program_account);
        
        // Cache the code
        self.program_cache.insert(program_id, code);
        
        Ok(())
    }

    /// Get program code
    pub fn get_program_code(&self, program_id: &str) -> Option<&Vec<u8>> {
        self.program_cache.get(program_id)
    }

    /// Check if account exists
    pub fn account_exists(&self, key: &str) -> bool {
        self.accounts.contains_key(key)
    }
}

/// Quantum primitives available to contracts
pub mod quantum_primitives {
    use sha3::{Digest, Sha3_256};

    /// Generate quantum random number (simulated for now)
    pub fn quantum_random(seed: [u8; 32], max: usize) -> Result<usize, String> {
        if max == 0 {
            return Err("Max must be greater than 0".to_string());
        }
        
        let mut hasher = Sha3_256::new();
        hasher.update(seed);
        hasher.update(b"quantum_random");
        let hash = hasher.finalize();
        
        let random_u64 = u64::from_le_bytes(hash[0..8].try_into().unwrap());
        Ok((random_u64 as usize) % max)
    }

    /// Verify Falcon signature in contract
    pub fn verify_falcon_signature(
        message: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<bool, String> {
        use pqcrypto_falcon::falcon512::{PublicKey, DetachedSignature, verify_detached_signature};
        use pqcrypto_traits::sign::{PublicKey as _, DetachedSignature as _};

        let pk = PublicKey::from_bytes(public_key)
            .map_err(|e| format!("Invalid public key: {:?}", e))?;
        let sig = DetachedSignature::from_bytes(signature)
            .map_err(|e| format!("Invalid signature: {:?}", e))?;

        Ok(verify_detached_signature(&sig, message, &pk).is_ok())
    }

    /// Kyber encryption (placeholder for contract use)
    pub fn kyber_encrypt(data: &[u8], _public_key: &[u8]) -> Result<Vec<u8>, String> {
        // TODO: Implement Kyber encryption for contract data
        // For now, just return the data (not secure!)
        Ok(data.to_vec())
    }

    /// Kyber decryption
    pub fn kyber_decrypt(ciphertext: &[u8], _secret_key: &[u8]) -> Result<Vec<u8>, String> {
        // TODO: Implement Kyber decryption
        Ok(ciphertext.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_account() {
        let account = Account::new_user(
            "user123".to_string(),
            vec![1, 2, 3],
            1000,
        );
        
        assert_eq!(account.balance, 1000);
        assert!(matches!(account.account_type, AccountType::User));
        assert!(!account.executable);
    }

    #[test]
    fn test_create_program_account() {
        let code = vec![0x00, 0x61, 0x73, 0x6d]; // WASM magic number
        let program = Account::new_program(
            "program123".to_string(),
            code.clone(),
            "deployer".to_string(),
        );
        
        assert_eq!(program.data, code);
        assert!(program.executable);
        assert!(matches!(program.account_type, AccountType::Program));
    }

    #[test]
    fn test_quantum_pda() {
        let seeds = &[b"token", b"mint", b"authority"];
        let program_id = "program123";
        
        let (address, bump) = QuantumPDA::find_program_address(seeds, program_id);
        
        assert!(!address.is_empty());
        assert!(bump <= 255);
        
        // Should be deterministic
        let (address2, bump2) = QuantumPDA::find_program_address(seeds, program_id);
        assert_eq!(address, address2);
        assert_eq!(bump, bump2);
    }

    #[test]
    fn test_contract_storage() {
        let mut storage = ContractStorage::new();
        
        let account = Account::new_user("user1".to_string(), vec![], 500);
        storage.set_account("user1".to_string(), account);
        
        assert!(storage.account_exists("user1"));
        assert!(!storage.account_exists("user2"));
        
        let retrieved = storage.get_account("user1").unwrap();
        assert_eq!(retrieved.balance, 500);
    }

    #[test]
    fn test_quantum_random() {
        let seed = [0u8; 32];
        let result1 = quantum_primitives::quantum_random(seed, 100).unwrap();
        let result2 = quantum_primitives::quantum_random(seed, 100).unwrap();
        
        // Same seed should give same result (deterministic)
        assert_eq!(result1, result2);
        assert!(result1 < 100);
    }
}
