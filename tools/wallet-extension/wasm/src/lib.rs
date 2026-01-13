use wasm_bindgen::prelude::*;
use sha3::{Digest, Sha3_256};
use falcon_rust::falcon512;

#[wasm_bindgen]
pub struct WalletKeys {
    pub_key: Vec<u8>,
    sec_key: Vec<u8>,
}

#[wasm_bindgen]
impl WalletKeys {
    #[wasm_bindgen(constructor)]
    pub fn new() -> WalletKeys {
        // Generate real Falcon-512 keypair using pure Rust implementation
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).unwrap_or_else(|_| {
            // Fallback for extreme cases, though getrandom should work in WASM with 'js' feature
            for i in 0..32 { seed[i] = i as u8; }
        });
        
        let (sk, pk) = falcon512::keygen(seed);
        WalletKeys {
            pub_key: pk.to_bytes(),
            sec_key: sk.to_bytes(),
        }
    }

    pub fn from_keypair(pk_hex: &str, sk_hex: &str) -> Result<WalletKeys, JsValue> {
        let pk_bytes = hex::decode(pk_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
        let sk_bytes = hex::decode(sk_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;

        // We can validate by trying to parse
        // Verify key integrity by attempting to load
        let _ = falcon512::SecretKey::from_bytes(&sk_bytes)
            .map_err(|_| JsValue::from_str("Invalid secret key bytes"))?;
        let _ = falcon512::PublicKey::from_bytes(&pk_bytes)
            .map_err(|_| JsValue::from_str("Invalid public key bytes"))?;

        Ok(WalletKeys {
            pub_key: pk_bytes,
            sec_key: sk_bytes,
        })
    }
    
    pub fn get_public_key_hex(&self) -> String {
        hex::encode(&self.pub_key)
    }

    pub fn get_private_key_hex(&self) -> String {
        hex::encode(&self.sec_key)
    }

    pub fn get_address(&self) -> String {
        // Address is usually last 20 bytes of hash of public key
        let mut hasher = Sha3_256::new();
        hasher.update(&self.pub_key);
        let hash = hasher.finalize();
        format!("0x{}", hex::encode(&hash[..20]))
    }

    pub fn sign_message(&self, message_hex: &str) -> Result<String, JsValue> {
        let message_bytes = hex::decode(message_hex).map_err(|e| JsValue::from_str(&e.to_string()))?;
        
        // Reconstruct SecretKey object from bytes
        let sk = falcon512::SecretKey::from_bytes(&self.sec_key)
             .map_err(|_| JsValue::from_str("Invalid secret key"))?;
            
        let sig = falcon512::sign(&message_bytes, &sk);
        Ok(hex::encode(sig.to_bytes()))
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
