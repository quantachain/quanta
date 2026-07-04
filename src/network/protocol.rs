use crate::core::block::Block;
use crate::core::transaction::Transaction;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::io::Read;

/// P2P protocol messages for blockchain network communication
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum P2PMessage {
    // Handshake messages
    Version {
        version: u32,
        height: u64,
        cumulative_work: u128,
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
    AlephBFTMessage(Vec<u8>),
    GetHeaders {
        start_height: u64,
    },
    Headers(Vec<BlockHeader>),
    GetHeight,
    Height {
        height: u64,
        cumulative_work: u128,
    },

    // Transaction propagation
    NewTx(Transaction),
    GetMempool,
    Mempool(Vec<Transaction>),

    // Keep-alive
    Ping(u64),
    Pong(u64),

    // BFT Consensus (Quanta 2.0)
    /// AlephBFT consensus message (serialized internal protocol data)
    BftMessage(Vec<u8>),

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

/// Simplified block header for efficient sync (v2 BFT)
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeader {
    pub index: u64,
    pub timestamp: i64,
    pub previous_hash: String,
    pub hash: String,
    /// BFT epoch this block belongs to.
    pub epoch: u64,
    /// Tendermint round in which this block was committed.
    pub bft_round: u32,
    /// Address of the proposer.
    pub proposer: String,
    /// Number of BFT signatures in the certificate.
    pub sig_count: usize,
    pub cumulative_work: u128,
    #[serde(default)]
    pub state_root: String,
}

impl From<&Block> for BlockHeader {
    fn from(block: &Block) -> Self {
        Self {
            index: block.index,
            timestamp: block.timestamp,
            previous_hash: block.previous_hash.clone(),
            hash: block.hash.clone(),
            epoch: block.epoch,
            bft_round: block.bft_round,
            proposer: block.proposer.clone(),
            sig_count: block.bft_signatures.len(),
            cumulative_work: 0, // Populated by sender
            state_root: block.state_root.clone(),
        }
    }
}

/// Protocol constants
pub const PROTOCOL_VERSION: u32 = 2; // v2: BFT from genesis
pub const MAX_MESSAGE_SIZE: usize = 8 * 1024 * 1024; // 8MB — 2× the 4MB block limit; headroom for bincode wrapper overhead
pub const PING_INTERVAL_SECS: u64 = 60;

/// Network magic bytes (prevents testnet/mainnet message mixing)
pub const TESTNET_MAGIC: [u8; 4] = *b"Q2T9"; // Quanta V2 Testnet (BFT) Reset 9

/// Default to Testnet magic for current Alpha phase
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



/// Serialize a P2P message with network magic bytes for transmission.
///
/// CRIT-6 FIX: Wraps every message in NetworkMessage with the correct magic
/// bytes before compressing. Receiving nodes verify the magic before
/// accepting — cross-network message injection is rejected.
pub fn serialize_message(msg: &P2PMessage) -> Result<Vec<u8>, String> {
    // 1. Wrap with magic bytes (prevents testnet/mainnet mixing)
    let wrapped = NetworkMessage::create(msg.clone());
    
    // 2. Serialize wrapped message to bincode
    let serialized = bincode::serialize(&wrapped)
        .map_err(|e| format!("Serialization error: {}", e))?;
    
    // 3. Compress with Zstd (Level 3) only for messages > 1KB
    if serialized.len() > 1024 {
        zstd::encode_all(serialized.as_slice(), 3)
            .map_err(|e| format!("Compression error: {}", e))
    } else {
        Ok(serialized)
    }
}

/// Deserialize a P2P message from wire data, verifying network magic bytes.
///
/// CRIT-6 FIX: Decodes NetworkMessage wrapper and verifies magic bytes before
/// returning the inner P2PMessage. Rejects cross-network injections.
/// HIGH-6 FIX: take() limit is now strictly MAX_MESSAGE_SIZE (was +1), and
/// decompressed buffer is pre-allocated with a capacity cap.
pub fn deserialize_message(data: &[u8]) -> Result<P2PMessage, String> {
    if data.len() > MAX_MESSAGE_SIZE {
        return Err("Message too large".to_string());
    }

    // 1. Try to decompress (detect zstd magic bytes)
    // Zstd magic: 0xFD2FB528 (LE) -> [0x28, 0xB5, 0x2F, 0xFD]
    let is_compressed = data.len() >= 4 &&
        data[0] == 0x28 && data[1] == 0xB5 && data[2] == 0x2F && data[3] == 0xFD;

    let decompressed = if is_compressed {
        let mut decoder = zstd::stream::Decoder::new(data)
            .map_err(|e| format!("Decompression error: {}", e))?;
        // HIGH-6 FIX: Pre-allocate with strict cap; take() = MAX exactly (no +1)
        let mut decomp_data = Vec::with_capacity(MAX_MESSAGE_SIZE);
        std::io::Read::take(&mut decoder, MAX_MESSAGE_SIZE as u64)
            .read_to_end(&mut decomp_data)
            .map_err(|e| format!("Decompression read error: {}", e))?;

        if decomp_data.len() > MAX_MESSAGE_SIZE {
            return Err("Decompressed message too large".to_string());
        }
        decomp_data
    } else {
        data.to_vec()
    };

    // 2. Deserialize the NetworkMessage wrapper
    let wrapped: NetworkMessage = bincode::deserialize(&decompressed)
        .map_err(|e| format!("Deserialization error: {}", e))?;

    // 3. CRIT-6: Verify network magic bytes — reject cross-network messages
    if !wrapped.verify() {
        return Err(format!(
            "Network magic mismatch: expected {:?}, got {:?}. \
             Cross-network message injection rejected.",
            NETWORK_MAGIC, wrapped.magic
        ));
    }

    Ok(wrapped.message)
}

