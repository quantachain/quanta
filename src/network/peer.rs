use crate::network::protocol::{P2PMessage, serialize_message, deserialize_message};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub address: SocketAddr,
    pub node_id: String,
    pub version: u32,
    pub height: u64,
    pub connected_at: i64,
    pub last_seen: i64,
}

/// Represents a connection to a peer in the network
pub struct Peer {
    info: Arc<RwLock<PeerInfo>>,
    read_half: Arc<RwLock<ReadHalf<TcpStream>>>,
    write_half: Arc<RwLock<WriteHalf<TcpStream>>>,
    shutdown_tx: mpsc::Sender<()>,
}

impl Peer {
    /// Create a new peer connection
    pub async fn new(
        stream: TcpStream,
        address: SocketAddr,
    ) -> Result<Self, String> {
        let (shutdown_tx, _) = mpsc::channel(1);
        
        let info = PeerInfo {
            address,
            node_id: String::new(),
            version: 0,
            height: 0,
            connected_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
        };

        // CRITICAL: Split stream to avoid read/write lock contention
        let (read_half, write_half) = tokio::io::split(stream);

        Ok(Self {
            info: Arc::new(RwLock::new(info)),
            read_half: Arc::new(RwLock::new(read_half)),
            write_half: Arc::new(RwLock::new(write_half)),
            shutdown_tx,
        })
    }

    /// Send a message to this peer
    pub async fn send_message(&self, msg: P2PMessage) -> Result<(), String> {
        let data = serialize_message(&msg)?;
        let len = data.len() as u32;
        
        let mut write = self.write_half.write().await;
        
        // Write length prefix (4 bytes) then message data
        write
            .write_all(&len.to_be_bytes())
            .await
            .map_err(|e| format!("Failed to write message length: {}", e))?;
        
        write
            .write_all(&data)
            .await
            .map_err(|e| format!("Failed to write message data: {}", e))?;
        
        write
            .flush()
            .await
            .map_err(|e| format!("Failed to flush stream: {}", e))?;

        debug!("Sent message to {}: {:?}", self.info.read().await.address, msg);
        Ok(())
    }

    /// Receive a message from this peer with timeout
    pub async fn receive_message(&self) -> Result<P2PMessage, String> {
        let result = timeout(
            Duration::from_secs(120),
            self.receive_message_internal()
        ).await;

        match result {
            Ok(Ok(msg)) => {
                // Update last seen time
                self.info.write().await.last_seen = chrono::Utc::now().timestamp();
                Ok(msg)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Receive timeout".to_string()),
        }
    }

    /// Internal message receiving logic
    async fn receive_message_internal(&self) -> Result<P2PMessage, String> {
        let mut read = self.read_half.write().await;
        
        // Read length prefix (4 bytes)
        let mut len_bytes = [0u8; 4];
        read
            .read_exact(&mut len_bytes)
            .await
            .map_err(|e| format!("Failed to read message length: {}", e))?;
        
        let len = u32::from_be_bytes(len_bytes) as usize;
        
        if len > 10 * 1024 * 1024 {
            return Err("Message too large".to_string());
        }
        
        // Read message data
        let mut data = vec![0u8; len];
        read
            .read_exact(&mut data)
            .await
            .map_err(|e| format!("Failed to read message data: {}", e))?;
        
        deserialize_message(&data)
    }

    /// Update peer information after handshake
    pub async fn update_info(&self, node_id: String, version: u32, height: u64) {
        let mut info = self.info.write().await;
        info.node_id = node_id;
        info.version = version;
        info.height = height;
    }

    /// Get peer information
    pub async fn get_info(&self) -> PeerInfo {
        self.info.read().await.clone()
    }

    /// Get peer address
    pub async fn address(&self) -> SocketAddr {
        self.info.read().await.address
    }

    /// Check if peer is alive
    pub async fn is_alive(&self) -> bool {
        let info = self.info.read().await;
        let now = chrono::Utc::now().timestamp();
        now - info.last_seen < 180 // 3 minutes timeout
    }

    /// Perform handshake with peer
    pub async fn handshake(&self, our_version: u32, our_height: u64, our_node_id: String) -> Result<(), String> {
        // Send our version
        let version_msg = P2PMessage::Version {
            version: our_version,
            height: our_height,
            timestamp: chrono::Utc::now().timestamp(),
            node_id: our_node_id,
        };
        
        self.send_message(version_msg).await?;
        
        // Wait for their version
        match self.receive_message().await? {
            P2PMessage::Version { version, height, node_id, .. } => {
                self.update_info(node_id, version, height).await;
                
                // Send verack
                self.send_message(P2PMessage::VerAck).await?;
                
                // Wait for their verack
                match self.receive_message().await? {
                    P2PMessage::VerAck => {
                        info!("Handshake completed with peer {}", self.info.read().await.address);
                        Ok(())
                    }
                    _ => Err("Expected VerAck".to_string()),
                }
            }
            _ => Err("Expected Version message".to_string()),
        }
    }

    /// Disconnect from peer
    pub async fn disconnect(&self) {
        let _ = self.send_message(P2PMessage::Disconnect).await;
        let _ = self.shutdown_tx.send(()).await;
    }
}

/// Peer connection manager for handling incoming/outgoing connections
pub struct PeerManager {
    peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    max_peers: usize,
}

impl PeerManager {
    pub fn new(max_peers: usize) -> Self {
        Self {
            peers: Arc::new(RwLock::new(Vec::new())),
            max_peers,
        }
    }

    /// Add a new peer connection
    pub async fn add_peer(&self, peer: Arc<Peer>) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        
        if peers.len() >= self.max_peers {
            return Err("Max peers reached".to_string());
        }
        
        // Check if already connected
        let peer_addr = peer.address().await;
        if peers.iter().any(|p| {
            matches!(p.info.try_read(), Ok(info) if info.address == peer_addr)
        }) {
            return Err("Already connected to this peer".to_string());
        }
        
        peers.push(peer);
        info!("Peer added. Total peers: {}", peers.len());
        Ok(())
    }

    /// Remove a peer
    pub async fn remove_peer(&self, address: SocketAddr) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| {
            !matches!(p.info.try_read(), Ok(info) if info.address == address)
        });
        info!("Peer removed. Total peers: {}", peers.len());
    }

    /// Get all connected peers
    pub async fn get_peers(&self) -> Vec<Arc<Peer>> {
        self.peers.read().await.clone()
    }

    /// Get number of connected peers
    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    /// Broadcast message to all peers (PARALLELIZED)
    pub async fn broadcast(&self, msg: P2PMessage) {
        let peers = self.peers.read().await.clone();
        
        // Spawn concurrent sends - don't let one slow peer block everyone
        for peer in peers {
            let msg_clone = msg.clone();
            tokio::spawn(async move {
                if let Err(e) = peer.send_message(msg_clone).await {
                    warn!("Failed to send message to peer: {}", e);
                }
            });
        }
    }

    /// Clean up dead peers
    pub async fn cleanup_dead_peers(&self) {
        let peers = self.peers.read().await;
        let mut alive_peers = Vec::new();
        
        for peer in peers.iter() {
            if peer.is_alive().await {
                alive_peers.push(Arc::clone(peer));
            }
        }
        
        let initial_count = peers.len();
        drop(peers);
        
        let removed = initial_count - alive_peers.len();
        if removed > 0 {
            *self.peers.write().await = alive_peers;
            info!("Cleaned up {} dead peers", removed);
        }
    }
}
