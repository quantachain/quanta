use crate::core::block::Block;
use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use hmac::{Hmac, Mac};
use sha3::Sha3_256;

type HmacSha3_256 = Hmac<Sha3_256>;

/// P2P protocol messages for blockchain network communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum P2PMessage {
    // Handshake messages
    Version {
        version: u32,
        height: u64,
        timestamp: i64,
        node_id: String,
    },
    VerAck,

    // Peer discovery
    GetAddr,
    Addr(Vec<SocketAddr>),

    // Blockchain synchronization
    GetBlocks {
        start_height: u64,
        end_height: u64,
    },
    Block(Block),
    GetHeaders {
        start_height: u64,
    },
    Headers(Vec<BlockHeader>),
    GetHeight,
    Height(u64),

    // Transaction propagation
    NewTx(Transaction),
    GetMempool,
    Mempool(Vec<Transaction>),

    // Keep-alive
    Ping(u64),
    Pong(u64),

    // Error handling
    Error(String),
    Disconnect,
}

/// Authenticated message wrapper (prevents Sybil attacks and tampering)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AuthenticatedMessage {
    pub message: P2PMessage,
    pub hmac: Vec<u8>, // HMAC-SHA3-256 of message
    pub nonce: u64, // Prevents replay attacks
}

/// Simplified block header for efficient sync
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    pub index: u64,
    pub timestamp: i64,
    pub previous_hash: String,
    pub hash: String,
    pub nonce: u64,
    pub difficulty: u32,
}

impl From<&Block> for BlockHeader {
    fn from(block: &Block) -> Self {
        Self {
            index: block.index,
            timestamp: block.timestamp,
            previous_hash: block.previous_hash.clone(),
            hash: block.hash.clone(),
            nonce: block.nonce,
            difficulty: block.difficulty,
        }
    }
}

/// Protocol constants
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const PING_INTERVAL_SECS: u64 = 60;
pub const PEER_TIMEOUT_SECS: u64 = 180;

//  CRITICAL SECURITY WARNING 
// NETWORK_SECRET must be CHANGED before testnet launch!
// 
// PRODUCTION SETUP:
// 1. Generate: openssl rand -hex 32
// 2. Store in environment: QUANTA_NETWORK_SECRET=<your_secret>
// 3. Read from env or config file (NEVER commit to git)
// 4. All testnet nodes MUST share the same secret
// 5. Use different secrets for mainnet vs testnet
//
//  TESTNET SECRET (Updated 2026-01-04):
const NETWORK_SECRET: &[u8] = b"0ca4cea38e2e914d3170feab4990b5a08dbe83153b2766ff60a228271887d0f9";

impl AuthenticatedMessage {
    /// Create authenticated message with HMAC
    pub fn create(message: P2PMessage) -> Result<Self, String> {
        let nonce = rand::random::<u64>();
        let message_bytes = bincode::serialize(&message)
            .map_err(|e| format!("Serialization error: {}", e))?;
        
        // Compute HMAC-SHA3-256
        let mut mac = HmacSha3_256::new_from_slice(NETWORK_SECRET)
            .map_err(|e| format!("HMAC error: {}", e))?;
        mac.update(&message_bytes);
        mac.update(&nonce.to_le_bytes());
        let hmac = mac.finalize().into_bytes().to_vec();
        
        Ok(Self {
            message,
            hmac,
            nonce,
        })
    }
    
    /// Verify message HMAC (prevents tampering and Sybil attacks)
    pub fn verify(&self) -> bool {
        let message_bytes = match bincode::serialize(&self.message) {
            Ok(b) => b,
            Err(_) => return false,
        };
        
        let mut mac = match HmacSha3_256::new_from_slice(NETWORK_SECRET) {
            Ok(m) => m,
            Err(_) => return false,
        };
        mac.update(&message_bytes);
        mac.update(&self.nonce.to_le_bytes());
        
        mac.verify_slice(&self.hmac).is_ok()
    }
}

/// Message handler trait for processing P2P messages
#[async_trait::async_trait]
pub trait MessageHandler: Send + Sync {
    async fn handle_version(&self, version: u32, height: u64, node_id: String) -> Result<(), String>;
    async fn handle_block(&self, block: Block) -> Result<(), String>;
    async fn handle_transaction(&self, tx: Transaction) -> Result<(), String>;
    async fn handle_get_blocks(&self, start: u64, end: u64) -> Result<Vec<Block>, String>;
    async fn handle_get_height(&self) -> Result<u64, String>;
    async fn handle_get_mempool(&self) -> Result<Vec<Transaction>, String>;
}

/// Serialize a message for network transmission
pub fn serialize_message(msg: &P2PMessage) -> Result<Vec<u8>, String> {
    bincode::serialize(msg).map_err(|e| format!("Serialization error: {}", e))
}

/// Deserialize a message from network data
pub fn deserialize_message(data: &[u8]) -> Result<P2PMessage, String> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err("Message too large".to_string());
    }
    bincode::deserialize(data).map_err(|e| format!("Deserialization error: {}", e))
}

