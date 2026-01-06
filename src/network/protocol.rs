use crate::core::block::Block;
use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

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

/// Network message wrapper with magic bytes for network identification
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NetworkMessage {
    pub magic: [u8; 4], // Network identifier
    pub message: P2PMessage,
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
pub const MAX_MESSAGE_SIZE: usize = 2 * 1024 * 1024; // 2MB
pub const PING_INTERVAL_SECS: u64 = 60;
pub const PEER_TIMEOUT_SECS: u64 = 180;

/// Network magic bytes (prevents testnet/mainnet message mixing)
pub const TESTNET_MAGIC: [u8; 4] = *b"QUAX"; // Quanta Testnet
pub const MAINNET_MAGIC: [u8; 4] = *b"QUAM"; // Quanta Mainnet

/// Get network magic based on configuration
#[cfg(feature = "mainnet")]
pub const NETWORK_MAGIC: [u8; 4] = MAINNET_MAGIC;

#[cfg(not(feature = "mainnet"))]
pub const NETWORK_MAGIC: [u8; 4] = TESTNET_MAGIC;

impl NetworkMessage {
    /// Create network message with magic bytes
    pub fn create(message: P2PMessage) -> Self {
        Self {
            magic: NETWORK_MAGIC,
            message,
        }
    }
    
    /// Verify message has correct network magic
    pub fn verify(&self) -> bool {
        self.magic == NETWORK_MAGIC
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

