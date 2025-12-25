use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Peer discovery mechanism
pub struct PeerDiscovery {
    known_peers: Arc<RwLock<Vec<SocketAddr>>>,
    seed_nodes: Vec<SocketAddr>,
}

impl PeerDiscovery {
    /// Create a new peer discovery instance
    pub fn new(seed_nodes: Vec<SocketAddr>) -> Self {
        Self {
            known_peers: Arc::new(RwLock::new(Vec::new())),
            seed_nodes,
        }
    }

    /// Get seed nodes
    pub fn get_seed_nodes(&self) -> &[SocketAddr] {
        &self.seed_nodes
    }

    /// Add a known peer
    pub async fn add_peer(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        if !peers.contains(&addr) {
            peers.push(addr);
            info!("Added known peer: {}", addr);
        }
    }

    /// Get all known peers
    pub async fn get_known_peers(&self) -> Vec<SocketAddr> {
        self.known_peers.read().await.clone()
    }

    /// Remove a peer
    pub async fn remove_peer(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        peers.retain(|&a| a != addr);
        warn!("Removed peer: {}", addr);
    }

    /// Get random peers for connection
    pub async fn get_random_peers(&self, count: usize) -> Vec<SocketAddr> {
        use rand::seq::SliceRandom;
        
        let mut peers = self.known_peers.read().await.clone();
        peers.extend_from_slice(&self.seed_nodes);
        
        let mut rng = rand::thread_rng();
        peers.shuffle(&mut rng);
        
        peers.into_iter().take(count).collect()
    }

    /// Bootstrap discovery from seed nodes
    pub async fn bootstrap(&self) -> Vec<SocketAddr> {
        let mut peers = self.known_peers.write().await;
        peers.extend_from_slice(&self.seed_nodes);
        
        info!("Bootstrapped with {} seed nodes", self.seed_nodes.len());
        self.seed_nodes.clone()
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
}
