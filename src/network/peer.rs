#![allow(dead_code)]
use crate::network::protocol::{
    deserialize_message, serialize_message, P2PMessage, MAX_MESSAGE_SIZE,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};
use lru::LruCache;
use std::num::NonZeroUsize;
use crate::network::swarm_command::SwarmCommand;

const BAN_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub address: SocketAddr,
    pub node_id: String,
    pub version: u32,
    pub height: u64,
    pub cumulative_work: u128,
    pub connected_at: i64,
    pub last_seen: i64,
    pub misbehavior_score: u32,
    pub strikes: u8,
    pub is_outbound: bool,
    pub reported_listen_port: Option<u16>,
}

pub struct Peer {
    pub info: Arc<RwLock<PeerInfo>>,
    pub swarm_tx: mpsc::Sender<SwarmCommand>,
}

impl Peer {
    pub async fn new(
        address: SocketAddr,
        node_id: String,
        swarm_tx: mpsc::Sender<SwarmCommand>,
    ) -> Result<Self, String> {
        let info = PeerInfo {
            address,
            node_id,
            version: 0,
            height: 0,
            cumulative_work: 0,
            connected_at: chrono::Utc::now().timestamp(),
            last_seen: chrono::Utc::now().timestamp(),
            misbehavior_score: 0,
            strikes: 0,
            is_outbound: false,
            reported_listen_port: None,
        };

        Ok(Self {
            info: Arc::new(RwLock::new(info)),
            swarm_tx,
        })
    }

    pub async fn send_message(&self, message: P2PMessage) -> Result<(), String> {
        let addr = self.info.read().await.address;
        self.swarm_tx.send(SwarmCommand::SendTo(addr, message)).await.map_err(|e| e.to_string())
    }

    pub async fn send_message_sync(&self, message: P2PMessage) -> Result<(), String> {
        self.send_message(message).await
    }

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

    pub async fn update_height(&self, height: u64, cumulative_work: u128) {
        let mut info = self.info.write().await;
        info.height = height;
        info.cumulative_work = cumulative_work;
    }

    pub async fn update_last_seen(&self) {
        let mut info = self.info.write().await;
        info.last_seen = chrono::Utc::now().timestamp();
    }

    pub async fn get_info(&self) -> PeerInfo {
        self.info.read().await.clone()
    }

    pub async fn set_outbound(&self) {
        self.info.write().await.is_outbound = true;
    }

    pub async fn add_misbehavior(&self, score: u32) -> bool {
        let mut info = self.info.write().await;
        info.misbehavior_score = info.misbehavior_score.saturating_add(score);
        if info.misbehavior_score >= 100 {
            info.strikes = 3; 
        }
        info.misbehavior_score >= 100
    }

    pub async fn add_strike(&self) -> bool {
        self.add_misbehavior(34).await
    }

    pub async fn address(&self) -> SocketAddr {
        self.info.read().await.address
    }

    pub async fn is_alive(&self) -> bool {
        let info = self.info.read().await;
        info.strikes < 3
            && chrono::Utc::now().timestamp() - info.last_seen < 60
            && info.misbehavior_score < 100
    }
}

pub struct PeerManager {
    peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    pub max_peers: usize,
    pub local_node_id: String,
    banned_ips: Arc<RwLock<LruCache<IpAddr, Instant>>>,
}

impl PeerManager {
    pub fn new(max_peers: usize, local_node_id: String) -> Self {
        Self {
            peers: Arc::new(RwLock::new(Vec::new())),
            max_peers,
            local_node_id,
            banned_ips: Arc::new(RwLock::new(LruCache::new(NonZeroUsize::new(1000).unwrap()))),
        }
    }

    pub async fn is_ip_banned(&self, ip: IpAddr) -> bool {
        let mut banned = self.banned_ips.write().await;
        if let Some(&expiry) = banned.get(&ip) {
            if Instant::now() < expiry {
                return true;
            } else {
                banned.pop(&ip);
            }
        }
        false
    }

    pub async fn add_peer(&self, peer: Arc<Peer>) -> Result<(), String> {
        let mut peers = self.peers.write().await;
        let peer_addr = peer.address().await;
        let peer_ip = peer_addr.ip();

        if self.is_ip_banned(peer_ip).await {
            return Err(format!("IP {} is banned", peer_ip));
        }

        if peers.len() >= self.max_peers {
            return Err("Max peers reached".to_string());
        }

        let new_peer_node_id = peer.info.read().await.node_id.clone();
        
        let mut subnet_count = 0;
        let mut stale_idx = None;

        for (i, p) in peers.iter().enumerate() {
            if let Ok(info) = p.info.try_read() {
                if info.address == peer_addr {
                    return Err("Peer already connected".to_string());
                }
                
                if !new_peer_node_id.is_empty() && info.node_id == new_peer_node_id {
                    let now = chrono::Utc::now().timestamp();
                    if now - info.last_seen > 30 {
                        stale_idx = Some(i);
                        break; 
                    } else {
                        let new_is_outbound = peer.info.read().await.is_outbound;
                        if !new_is_outbound {
                            stale_idx = Some(i);
                            break;
                        } else {
                            return Err(format!("Tie-break: rejecting our outbound connection to Node ID {} in favor of their outbound", new_peer_node_id));
                        }
                    }
                }

                match (info.address.ip(), peer_ip) {
                    (IpAddr::V4(a), IpAddr::V4(b)) => {
                        let is_docker = a.octets()[0] == 172 && (16..=31).contains(&a.octets()[1]);
                        if !a.is_loopback() && !is_docker && a.octets()[0..3] == b.octets()[0..3] {
                            subnet_count += 1;
                        }
                    }
                    (IpAddr::V6(a), IpAddr::V6(b)) => {
                        if a.octets()[0..6] == b.octets()[0..6] {
                            subnet_count += 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(idx) = stale_idx {
            let evicted_addr = peers[idx].address().await;
            warn!("Evicting stale peer {} — accepting fresh connection from {}", evicted_addr, peer_addr);
            peers.remove(idx);
        }

        if subnet_count >= 100 {
            return Err(format!("Too many connections from subnet of {}", peer_ip));
        }

        peers.push(peer);
        info!("Peer {} added. Total peers: {}", peer_addr, peers.len());
        Ok(())
    }

    pub async fn ban_ip(&self, ip: IpAddr) {
        let expiry = Instant::now() + BAN_DURATION;
        self.banned_ips.write().await.put(ip, expiry);
    }

    pub async fn remove_peer(&self, address: SocketAddr) {
        let mut peers = self.peers.write().await;
        peers.retain(|p| !matches!(p.info.try_read(), Ok(info) if info.address == address));
    }

    pub async fn get_peers(&self) -> Vec<Arc<Peer>> {
        self.peers.read().await.clone()
    }

    pub async fn peer_count(&self) -> usize {
        self.peers.read().await.len()
    }

    pub async fn get_peer(&self, addr: &SocketAddr) -> Option<Arc<Peer>> {
        let peers = self.peers.read().await;
        for peer in peers.iter() {
            if let Ok(info) = peer.info.try_read() {
                if info.address == *addr {
                    return Some(Arc::clone(peer));
                }
            }
        }
        None
    }


    pub async fn broadcast(&self, msg: P2PMessage) {
        let peers = self.peers.read().await.clone();
        for peer in peers {
            if peer.info.read().await.strikes >= 100 {
                continue;
            }
            let msg_clone = msg.clone();
            let peer_clone = Arc::clone(&peer);
            tokio::spawn(async move {
                let _ = peer_clone.send_message(msg_clone).await;
            });
        }
    }

    pub async fn cleanup_dead_peers(&self) {
        let peers_snapshot = self.peers.read().await.clone();
        let mut alive_peers = Vec::new();

        for peer in peers_snapshot.iter() {
            if peer.is_alive().await {
                alive_peers.push(Arc::clone(peer));
            } else {
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
        }
    }
}
