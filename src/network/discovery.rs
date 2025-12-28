use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Peer metadata for tracking peer health and source
#[derive(Clone, Debug)]
pub struct PeerMeta {
    pub address: SocketAddr,
    pub last_seen: i64,
    pub failures: u32,
    pub source: PeerSource,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PeerSource {
    Seed,
    Discovered,
    Manual,
}

/// Peer discovery mechanism
pub struct PeerDiscovery {
    known_peers: Arc<RwLock<HashMap<SocketAddr, PeerMeta>>>,
    seed_nodes: Vec<SocketAddr>,
}

impl PeerDiscovery {
    /// Create a new peer discovery instance
    pub fn new(seed_nodes: Vec<SocketAddr>) -> Self {
        Self {
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            seed_nodes,
        }
    }

    /// Get seed nodes
    pub fn get_seed_nodes(&self) -> &[SocketAddr] {
        &self.seed_nodes
    }

    /// Add a known peer with metadata
    pub async fn add_peer(&self, addr: SocketAddr) {
        self.add_peer_with_source(addr, PeerSource::Discovered).await;
    }
    
    /// Add a peer with specific source
    pub async fn add_peer_with_source(&self, addr: SocketAddr, source: PeerSource) {
        let mut peers = self.known_peers.write().await;
        peers.entry(addr).or_insert_with(|| {
            info!("Added known peer: {} (source: {:?})", addr, source);
            PeerMeta {
                address: addr,
                last_seen: chrono::Utc::now().timestamp(),
                failures: 0,
                source,
            }
        });
    }
    
    /// Update peer last seen time
    pub async fn update_peer_seen(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        if let Some(meta) = peers.get_mut(&addr) {
            meta.last_seen = chrono::Utc::now().timestamp();
            meta.failures = 0; // Reset failures on successful contact
        }
    }
    
    /// Mark peer as failed
    pub async fn mark_peer_failed(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        if let Some(meta) = peers.get_mut(&addr) {
            meta.failures += 1;
            let failures = meta.failures;
            let is_seed = meta.source == PeerSource::Seed;
            
            // Remove if too many failures (unless it's a seed)
            if failures > 5 && !is_seed {
                peers.remove(&addr);
                warn!("Removed peer {} after {} failures", addr, failures);
            }
        }
    }

    /// Get all known peer addresses
    pub async fn get_known_peers(&self) -> Vec<SocketAddr> {
        self.known_peers.read().await.keys().copied().collect()
    }
    
    /// Get peer metadata
    pub async fn get_peer_meta(&self, addr: &SocketAddr) -> Option<PeerMeta> {
        self.known_peers.read().await.get(addr).cloned()
    }

    /// Remove a peer
    pub async fn remove_peer(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        peers.remove(&addr);
        warn!("Removed peer: {}", addr);
    }

    /// Get random peers for connection (prioritizes healthy peers)
    pub async fn get_random_peers(&self, count: usize) -> Vec<SocketAddr> {
        use rand::seq::SliceRandom;
        
        let peers = self.known_peers.read().await;
        let now = chrono::Utc::now().timestamp();
        
        // Filter healthy peers (seen recently, low failures)
        let mut healthy: Vec<SocketAddr> = peers
            .values()
            .filter(|meta| {
                meta.failures < 3 && (now - meta.last_seen) < 3600 // Active in last hour
            })
            .map(|meta| meta.address)
            .collect();
        
        // Add seeds if we don't have enough healthy peers
        if healthy.len() < count {
            healthy.extend(self.seed_nodes.iter().copied());
        }
        
        let mut rng = rand::thread_rng();
        healthy.shuffle(&mut rng);
        
        healthy.into_iter().take(count).collect()
    }

    /// Bootstrap discovery from seed nodes (deduplicated)
    pub async fn bootstrap(&self) -> Vec<SocketAddr> {
        let mut peers = self.known_peers.write().await;
        
        // Only add seeds if not already present
        for &seed in &self.seed_nodes {
            peers.entry(seed).or_insert_with(|| PeerMeta {
                address: seed,
                last_seen: chrono::Utc::now().timestamp(),
                failures: 0,
                source: PeerSource::Seed,
            });
        }
        
        info!("Bootstrapped with {} seed nodes", self.seed_nodes.len());
        self.seed_nodes.clone()
    }
    
    /// Process Addr message from peer (with spam protection)
    pub async fn process_addr_message(&self, addrs: Vec<SocketAddr>, max_addrs: usize) {
        if addrs.len() > max_addrs {
            warn!("Received too many addresses ({}), capping to {}", addrs.len(), max_addrs);
        }
        
        let mut peers = self.known_peers.write().await;
        let now = chrono::Utc::now().timestamp();
        
        for addr in addrs.into_iter().take(max_addrs) {
            // Validate routable IP (reject private unless allowed)
            if !is_routable_addr(&addr) {
                continue;
            }
            
            peers.entry(addr).or_insert_with(|| PeerMeta {
                address: addr,
                last_seen: now,
                failures: 0,
                source: PeerSource::Discovered,
            });
        }
    }
}

/// Default seed nodes for the QUANTA network
pub fn default_seed_nodes() -> Vec<SocketAddr> {
    vec![
        // Add your seed nodes here when deploying
        // "seed1.quanta.network:8333".parse().unwrap(),
        // "seed2.quanta.network:8333".parse().unwrap(),
    ]
}

/// Check if address is routable (not private/loopback unless allowed)
fn is_routable_addr(addr: &SocketAddr) -> bool {
    let ip = addr.ip();
    
    // Allow loopback for local testing
    if ip.is_loopback() {
        return true;
    }
    
    // Reject private IPs (can be made configurable)
    match ip {
        std::net::IpAddr::V4(ipv4) => {
            // Reject: 10.x.x.x, 172.16-31.x.x, 192.168.x.x
            !(ipv4.octets()[0] == 10
                || (ipv4.octets()[0] == 172 && (16..=31).contains(&ipv4.octets()[1]))
                || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168))
        }
        std::net::IpAddr::V6(ipv6) => {
            // Reject private/link-local
            !ipv6.is_unique_local() && !ipv6.is_multicast()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_peer_discovery() {
        let seeds = vec!["127.0.0.1:8333".parse().unwrap()];
        let discovery = PeerDiscovery::new(seeds);
        
        discovery.add_peer("127.0.0.1:8334".parse().unwrap()).await;
        
        let peers = discovery.get_known_peers().await;
        assert_eq!(peers.len(), 1);
    }
    
    #[tokio::test]
    async fn test_addr_spam_protection() {
        let discovery = PeerDiscovery::new(vec![]);
        
        // Try to add 100 addresses
        let addrs: Vec<SocketAddr> = (8333..8433)
            .map(|port| format!("1.2.3.{}:{}", port % 256, port).parse().unwrap())
            .collect();
        
        discovery.process_addr_message(addrs, 50).await;
        
        let peers = discovery.get_known_peers().await;
        assert!(peers.len() <= 50); // Should be capped
    }
    
    #[tokio::test]
    async fn test_peer_failure_tracking() {
        let discovery = PeerDiscovery::new(vec![]);
        let addr = "1.2.3.4:8333".parse().unwrap();
        
        discovery.add_peer(addr).await;
        
        // Mark as failed multiple times
        for _ in 0..6 {
            discovery.mark_peer_failed(addr).await;
        }
        
        let peers = discovery.get_known_peers().await;
        assert!(!peers.contains(&addr)); // Should be removed after too many failures
    }
}
