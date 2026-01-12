use wasm_bindgen::prelude::*;
use sha3::{Digest, Sha3_256};
use serde::{Serialize, Deserialize};

// Falcon-512 Constants
const FALCON_PK_SIZE: usize = 897;
const FALCON_SK_SIZE: usize = 1281;
const FALCON_SIG_SIZE: usize = 666;

#[wasm_bindgen]
pub struct WalletKeys {
    pub_key: Vec<u8>,
    sec_key: Vec<u8>,
}

#[wasm_bindgen]
impl WalletKeys {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WalletKeys {
        // SIMULATION MODE (High Fidelity)
        // We generate random bytes of the EXACT Falcon-512 lengths.
        let mut pk = vec![0u8; FALCON_PK_SIZE];
        let mut sk = vec![0u8; FALCON_SK_SIZE];
        
        // Fill with random data
        getrandom::getrandom(&mut pk).unwrap_or(());
        getrandom::getrandom(&mut sk).unwrap_or(());
        
        // Ensure first byte is 0x00/0x50 header for realism if needed, 
        // but for now random is fine.
        
        WalletKeys {
            pub_key: pk,
            sec_key: sk,
        }
    }

    pub fn from_private(secret_hex: &str) -> Result<WalletKeys, JsValue> {
        let sec_key = hex::decode(secret_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Mock Public Key derivation
        // In simulation, we just hash the SK to get a "consistent" PK
        let mut hasher = Sha3_256::new();
        hasher.update(&sec_key);
        let hash = hasher.finalize();
        
        let mut pk = Vec::with_capacity(FALCON_PK_SIZE);
        while pk.len() < FALCON_PK_SIZE {
            pk.extend_from_slice(&hash);
        }
        pk.truncate(FALCON_PK_SIZE);

        Ok(WalletKeys {
            pub_key: pk,
            sec_key: sec_key,
        })
    }
    
    pub fn get_public_key_hex(&self) -> String {
        hex::encode(&self.pub_key)
    }

    pub fn get_private_key_hex(&self) -> String {
        hex::encode(&self.sec_key)
    }

    pub fn get_address(&self) -> String {
        let mut hasher = Sha3_256::new();
        hasher.update(&self.pub_key);
        let hash = hasher.finalize();
        format!("0x{}", hex::encode(&hash[..20]))
    }

    pub fn sign_message(&self, message_hex: &str) -> Result<String, JsValue> {
        let mut sig = vec![0u8; FALCON_SIG_SIZE];
        // Make signature deterministic based on message for realism
        let message_bytes = hex::decode(message_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut hasher = Sha3_256::new();
        hasher.update(&message_bytes);
        hasher.update(&self.sec_key); // Bind to key
        let hash = hasher.finalize();
        
        for i in 0..32 {
            sig[i+1] = hash[i]; // Embed hash into sig
        }
        // Fill rest with pseudo-random pattern
        for i in 33..FALCON_SIG_SIZE {
            sig[i] = (sig[i-1] as u16 + 7) as u8; 
        }
        
        Ok(hex::encode(sig))
    }
    
    pub fn sign_transaction_hash(&self, hash_hex: &str) -> Result<String, JsValue> {
        self.sign_message(hash_hex)
    }
}

#[wasm_bindgen]
pub fn get_address_from_pubkey(pubkey_hex: &str) -> Result<String, JsValue> {
    let pub_key = hex::decode(pubkey_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let mut hasher = Sha3_256::new();
    hasher.update(&pub_key);
    let hash = hasher.finalize();
    Ok(format!("0x{}", hex::encode(&hash[..20])))
}
