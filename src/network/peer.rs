#![allow(dead_code)]
use crate::network::protocol::{
    deserialize_message, serialize_message, P2PMessage, MAX_MESSAGE_SIZE,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::timeout;
use tracing::{info, warn};

/// Information about a connected peer
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub address: SocketAddr,
    pub node_id: String,
    pub version: u32,
    pub height: u64,
    pub cumulative_work: u128,
    pub connected_at: i64,
    pub last_seen: i64,
    /// Weighted misbehavior score (0–100). Replaces the old binary strike system.
    /// Score thresholds per offense type (matches Bitcoin DoSMan philosophy):
    ///   Invalid block    → +50  (one or two = ban; serious consensus violation)
    ///   Message flood    → +20  (5 floods = ban)
    ///   Invalid tx       → +10  (10 bad txs = ban)
    ///   Stale/old block  →  +0  (not scored — could be network latency)
    /// Peer is disconnected and IP-banned when score ≥ 100.
    pub misbehavior_score: u32,
    /// Legacy field kept so PeerManager cleanup can still gate on it
    pub strikes: u8,
    pub is_outbound: bool,
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
    pub async fn new(stream: TcpStream, address: SocketAddr) -> Result<Self, String> {
        let (shutdown_tx, _) = mpsc::channel(1);

        let info = PeerInfo {
            address,
            node_id: String::new(),
            version: 0,
            height: 0,
            cumulative_work: 0,
            connected_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
            misbehavior_score: 0,
            strikes: 0,
            is_outbound: false,
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

        Ok(())
    }

    /// Receive a message from this peer with timeout
    pub async fn receive_message(&self) -> Result<P2PMessage, String> {
        let result = timeout(Duration::from_secs(120), self.receive_message_internal()).await;

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
        read.read_exact(&mut len_bytes)
            .await
            .map_err(|e| format!("Failed to read message length: {}", e))?;

        let len = u32::from_be_bytes(len_bytes) as usize;

        if len > MAX_MESSAGE_SIZE {
            return Err(format!("Message too large: {} > {}", len, MAX_MESSAGE_SIZE));
        }

        // Read message data
        let mut data = vec![0u8; len];
        read.read_exact(&mut data)
            .await
            .map_err(|e| format!("Failed to read message data: {}", e))?;

        // CRIT-6: magic bytes verified inside deserialize_message (protocol.rs)
        deserialize_message(&data)
    }

    /// Update peer information after handshake
    pub async fn update_info(
        &self,
        node_id: String,
        version: u32,
        height: u64,
        cumulative_work: u128,
    ) {
        let mut info = self.info.write().await;
        info.node_id = node_id;
        info.version = version;
        info.height = height;
        info.cumulative_work = cumulative_work;
    }

    /// Update peer height specifically (e.g. from Height messages)
    pub async fn update_height(&self, height: u64, cumulative_work: u128) {
        let mut info = self.info.write().await;
        info.height = height;
        info.cumulative_work = cumulative_work;
    }

    /// Get peer information
    pub async fn get_info(&self) -> PeerInfo {
        self.info.read().await.clone()
    }

    /// Mark connection as outbound
    pub async fn set_outbound(&self) {
        self.info.write().await.is_outbound = true;
    }

    /// Add weighted misbehavior score for bad behavior.
    ///
    /// Recommended weights:
    ///   - Invalid block (consensus violation) → score = 50
    ///   - Message flood                       → score = 20
    ///   - Invalid transaction                 → score = 10
    ///
    /// Returns `true` when the peer should be banned (≥ 100 total).
    pub async fn add_misbehavior(&self, score: u32) -> bool {
        let mut info = self.info.write().await;
        info.misbehavior_score = info.misbehavior_score.saturating_add(score);
        // Keep legacy strikes field in sync for cleanup_dead_peers
        if info.misbehavior_score >= 100 {
            info.strikes = 3; // trigger ban in cleanup
        }
        tracing::warn!(
            "Peer {} misbehavior score: {}/100 (+{})",
            info.address,
            info.misbehavior_score,
            score
        );
        info.misbehavior_score >= 100
    }

    /// Legacy shim — adds 33 points (3 calls = ban, same as old 3-strike system).
    /// Prefer `add_misbehavior(weight)` for new code.
    pub async fn add_strike(&self) -> bool {
        self.add_misbehavior(34).await
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
    pub async fn handshake(
        &self,
        our_version: u32,
        our_height: u64,
        our_cumulative_work: u128,
        our_node_id: String,
    ) -> Result<(), String> {
        // Send our version
        let version_msg = P2PMessage::Version {
            version: our_version,
            height: our_height,
            cumulative_work: our_cumulative_work,
            timestamp: chrono::Utc::now().timestamp(),
            node_id: our_node_id,
        };

        self.send_message(version_msg).await?;

        // Wait for their version
        match self.receive_message().await? {
            P2PMessage::Version {
                version,
                height,
                cumulative_work,
                node_id,
                ..
            } => {
                if version != our_version {
                    tracing::warn!(
                        "Handshake rejected: peer {} has incompatible version {} (we are {})",
                        self.info.read().await.address,
                        version,
                        our_version
                    );
                    return Err("Incompatible protocol version".to_string());
                }
                self.update_info(node_id, version, height, cumulative_work)
                    .await;

                // Send verack
                self.send_message(P2PMessage::VerAck).await?;

                // Wait for their verack
                match self.receive_message().await? {
                    P2PMessage::VerAck => {
                        info!(
                            "Handshake completed with peer {}",
                            self.info.read().await.address
                        );
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

// ---------------------------------------------------------------------------
// PeerManager
// ---------------------------------------------------------------------------

/// Peer connection manager for handling incoming/outgoing connections.
///
/// HIGH-4 FIX: Added persistent `banned_ips` map (IpAddr → Instant) so bans
/// survive peer reconnections. An attacker can no longer reset their strike
/// count by disconnecting and re-connecting.
///
/// MED-2 FIX: Sybil protection now covers both IPv4 /24 and IPv6 /48 subnets.
pub struct PeerManager {
    peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    max_peers: usize,
    /// HIGH-4: Persistent IP ban list — IpAddr -> ban expiry Instant
    /// SECURITY FIX: Bounded by LruCache (max 5000) to prevent OOM via IP spoofing
    banned_ips: Arc<RwLock<lru::LruCache<IpAddr, Instant>>>,
    our_node_id: String,
}

/// Duration of a peer ban triggered by 3+ strikes.
pub const BAN_DURATION: Duration = Duration::from_secs(60 * 60); // 1 hour

impl PeerManager {
    pub fn new(max_peers: usize, our_node_id: String) -> Self {
        Self {
            peers: Arc::new(RwLock::new(Vec::new())),
            max_peers,
            banned_ips: Arc::new(RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(5000).unwrap(),
            ))),
            our_node_id,
        }
    }

    /// Add a new peer connection (Sybil Protection + IP Ban enforcement).
    pub async fn add_peer(&self, peer: Arc<Peer>) -> Result<(), String> {
        let peer_addr = peer.address().await;
        let peer_ip = peer_addr.ip();

        // HIGH-4: Check persistent ban list BEFORE acquiring the peers write lock
        {
            let mut bans = self.banned_ips.write().await;
            if let Some(&ban_expiry) = bans.get(&peer_ip) {
                if Instant::now() < ban_expiry {
                    return Err(format!(
                        "IP {} is banned (ban expires in {}s)",
                        peer_ip,
                        ban_expiry.duration_since(Instant::now()).as_secs()
                    ));
                } else {
                    // Ban has expired — remove it
                    bans.pop(&peer_ip);
                }
            }
        }

        let mut peers = self.peers.write().await;

        if peers.len() >= self.max_peers {
            return Err("Max peers reached".to_string());
        }

        // FLAP-FIX: Liveness-aware duplicate IP check.
        //
        // The previous hard check ("Already connected to this peer IP") held the
        // IP slot for up to 180 s after a peer's TCP stream died, because
        // cleanup_dead_peers() only runs every 10 s and uses a 180 s last_seen
        // timeout. Every reconnect attempt during that window was rejected,
        // causing "Connection reset by peer" on the initiating side, which then
        // retried every 10 s — producing the flapping loop seen in the logs.
        //
        // Fix: if the existing peer with the same IP is DEAD (last_seen > 30s ago),
        // evict it immediately and accept the fresh connection.
        // Only a LIVE duplicate causes a rejection.
        let mut stale_idx: Option<usize> = None;
        let mut subnet_count = 0;

        for (i, p) in peers.iter().enumerate() {
            if let Ok(info) = p.info.try_read() {
                // Exact address match
                if info.address == peer_addr {
                    // Prefer the newer connection: check liveness
                    let now = chrono::Utc::now().timestamp();
                    if now - info.last_seen > 30 {
                        stale_idx = Some(i);
                        break; // evict and accept below
                    }
                    return Err("Already connected to this peer".to_string());
                }

                // Same IP, different port — check liveness before rejecting
                if info.address.ip() == peer_ip {
                    let now = chrono::Utc::now().timestamp();
                    if now - info.last_seen > 30 {
                        // Stale connection holding the IP slot — evict it
                        stale_idx = Some(i);
                        break;
                    }
                    
                    let remote_node_id = &info.node_id;
                    let new_peer_info = peer.info.read().await;
                    let new_is_outbound = new_peer_info.is_outbound;
                    
                    if self.our_node_id == *remote_node_id {
                        return Err("Connected to self".to_string());
                    }
                    
                    // Deterministic tie-breaking for simultaneous connections
                    let we_are_larger = self.our_node_id > *remote_node_id;
                    
                    if we_are_larger {
                        // We are the larger node, so we prefer OUR OUTBOUND connection.
                        if new_is_outbound {
                            stale_idx = Some(i);
                            break;
                        } else {
                            return Err(format!("Tie-break: rejecting inbound connection from IP {} in favor of our outbound", peer_ip));
                        }
                    } else {
                        // We are the smaller node, so we prefer THEIR OUTBOUND (our INBOUND) connection.
                        if !new_is_outbound {
                            stale_idx = Some(i);
                            break;
                        } else {
                            return Err(format!("Tie-break: rejecting our outbound connection to IP {} in favor of their outbound", peer_ip));
                        }
                    }
                }

                match (info.address.ip(), peer_ip) {
                    // IPv4: compare first 3 octets (/24 subnet)
                    (IpAddr::V4(a), IpAddr::V4(b)) => {
                        // ALLOW Docker internal networks to bypass Sybil protection
                        let is_docker = a.octets()[0] == 172 && (16..=31).contains(&a.octets()[1]);
                        if !a.is_loopback() && !is_docker && a.octets()[0..3] == b.octets()[0..3] {
                            subnet_count += 1;
                        }
                    }
                    // MED-2 FIX: IPv6: compare first 6 bytes (/48 subnet)
                    (IpAddr::V6(a), IpAddr::V6(b)) => {
                        if a.octets()[0..6] == b.octets()[0..6] {
                            subnet_count += 1;
                        }
                    }
                    _ => {} // mixed IPv4/IPv6 — different subnets by definition
                }
            }
        }

        // Evict stale peer and proceed with the new live connection
        if let Some(idx) = stale_idx {
            let evicted_addr = peers[idx].address().await;
            warn!(
                "Evicting stale peer {} (last_seen > 30s) — accepting fresh connection from {}",
                evicted_addr, peer_addr
            );
            peers.remove(idx);
        }

        if subnet_count >= 2 {
            return Err(format!(
                "Too many connections from subnet of {} (Sybil Protection — max 2 per /24 IPv4 or /48 IPv6)",
                peer_ip
            ));
        }

        peers.push(peer);
        info!("Peer {} added. Total peers: {}", peer_addr, peers.len());
        Ok(())
    }

    /// Ban an IP address for BAN_DURATION (HIGH-4).
    ///
    /// Called by the network layer when a peer accumulates 3+ strikes.
    pub async fn ban_ip(&self, ip: IpAddr) {
        let expiry = Instant::now() + BAN_DURATION;
        self.banned_ips.write().await.put(ip, expiry);
        warn!(
            "IP {} BANNED for {} minutes (persistent — survives reconnect)",
            ip,
            BAN_DURATION.as_secs() / 60
        );
    }

    /// Remove a peer
    pub async fn remove_peer(&self, address: SocketAddr) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| !matches!(p.info.try_read(), Ok(info) if info.address == address));
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

        // Spawn concurrent sends — don't let one slow peer block everyone
        for peer in peers {
            let msg_clone = msg.clone();
            tokio::spawn(async move {
                if let Err(e) = peer.send_message(msg_clone).await {
                    warn!("Failed to send message to peer: {}", e);
                }
            });
        }
    }

    /// Clean up dead peers, banning IPs that have accumulated 3+ strikes.
    pub async fn cleanup_dead_peers(&self) {
        let peers_snapshot = self.peers.read().await.clone();
        let mut alive_peers = Vec::new();

        for peer in peers_snapshot.iter() {
            if peer.is_alive().await {
                alive_peers.push(Arc::clone(peer));
            } else {
                // HIGH-4: if peer was booted due to strikes, persist the ban
                let info = peer.info.read().await;
                if info.strikes >= 3 {
                    self.ban_ip(info.address.ip()).await;
                }
            }
        }

        let initial_count = peers_snapshot.len();
        let removed = initial_count - alive_peers.len();
        drop(peers_snapshot);

        if removed > 0 {
            *self.peers.write().await = alive_peers;
            info!("Cleaned up {} dead peers", removed);
        }
    }
}
