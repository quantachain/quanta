#![allow(dead_code)]
use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
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
    pub reputation: i32,
    pub banned_until: Option<i64>,
    /// ADDRMAN FIX v3.1.0-alpha (2026-08-20): Bitcoin-style "new" vs "tried" table.
    /// false = "new table": discovered via gossip or inbound connect; not yet confirmed connectable.
    /// true  = "tried table": we personally connected OUTBOUND to this IP and it succeeded.
    /// ONLY verified peers are returned in GetAddr responses (gossip-safe).
    pub verified: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PeerSource {
    Seed,
    Discovered,
}

/// Peer discovery mechanism
pub struct PeerDiscovery {
    known_peers: Arc<RwLock<HashMap<SocketAddr, PeerMeta>>>,
    seed_nodes: Vec<SocketAddr>,
    dns_seeds: Vec<String>,
}

impl PeerDiscovery {
    /// Create a new peer discovery instance
    pub fn new(seed_nodes: Vec<SocketAddr>) -> Self {
        Self {
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            seed_nodes,
            dns_seeds: Vec::new(),
        }
    }

    /// Create with DNS seeds
    pub fn with_dns_seeds(seed_nodes: Vec<SocketAddr>, dns_seeds: Vec<String>) -> Self {
        Self {
            known_peers: Arc::new(RwLock::new(HashMap::new())),
            seed_nodes,
            dns_seeds,
        }
    }

    /// Resolve DNS seeds to socket addresses
    pub async fn resolve_dns_seeds(&self) -> Vec<SocketAddr> {
        let mut resolved = Vec::new();

        for dns_seed in &self.dns_seeds {
            info!("Resolving DNS seed: {}", dns_seed);

            // Try with standard port if not specified
            let lookup_addr = if dns_seed.contains(':') {
                dns_seed.clone()
            } else {
                format!("{}:8333", dns_seed) // Default Quanta P2P port
            };

            match tokio::task::spawn_blocking(move || lookup_addr.to_socket_addrs()).await {
                Ok(Ok(addrs)) => {
                    let addresses: Vec<SocketAddr> = addrs.collect();
                    info!(
                        "DNS seed {} resolved to {} addresses",
                        dns_seed,
                        addresses.len()
                    );
                    resolved.extend(addresses);
                }
                Ok(Err(e)) => {
                    warn!("Failed to resolve DNS seed {}: {}", dns_seed, e);
                }
                Err(e) => {
                    warn!("DNS resolution task failed for {}: {}", dns_seed, e);
                }
            }
        }

        // Add resolved addresses to known peers
        for addr in &resolved {
            self.add_peer_with_source(*addr, PeerSource::Seed).await;
        }

        resolved
    }

    /// Get seed nodes
    pub fn get_seed_nodes(&self) -> &[SocketAddr] {
        &self.seed_nodes
    }

    /// Add a known peer with metadata (added as UNVERIFIED / "new table")
    pub async fn add_peer(&self, addr: SocketAddr) {
        self.add_peer_with_source(addr, PeerSource::Discovered)
            .await;
    }

    /// Add a peer with specific source (added as UNVERIFIED / "new table")
    pub async fn add_peer_with_source(&self, addr: SocketAddr, source: PeerSource) {
        let mut peers = self.known_peers.write().await;
        peers.entry(addr).or_insert_with(|| {
            info!("Added known peer: {} (source: {:?})", addr, source);
            PeerMeta {
                address: addr,
                last_seen: chrono::Utc::now().timestamp(),
                failures: 0,
                source,
                reputation: 0,
                banned_until: None,
                verified: false, // ADDRMAN: starts as "new table" (unverified)
            }
        });
    }

    /// Update peer last seen time and promote to "tried" table (verified).
    /// Called ONLY after a successful OUTBOUND connection — this is the AddrMan promotion.
    pub async fn update_peer_seen(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        if let Some(meta) = peers.get_mut(&addr) {
            meta.last_seen = chrono::Utc::now().timestamp();
            meta.failures = 0;
            meta.reputation = (meta.reputation + 1).min(100);
            // ADDRMAN FIX v3.1.0-alpha (2026-08-20): Promote to "tried" table on successful outbound connection.
            // This mirrors Bitcoin's feeler connection model: only IPs we've personally
            // connected to outbound are considered "verified" and safe to gossip.
            meta.verified = true;
        }
    }

    /// Mark peer as failed (network/dial failure)
    pub async fn mark_peer_failed(&self, addr: SocketAddr) {
        let mut peers = self.known_peers.write().await;
        if let Some(meta) = peers.get_mut(&addr) {
            meta.failures += 1;
            
            // SECURITY HOTFIX (v3.1.2-alpha): Connection failures (timeouts, NAT blocks, Cloudflare resets)
            // MUST NOT decrease reputation or cause bans. If we ban Cloudflare IPs for dial failures,
            // we accidentally ban all inbound connections routed through that edge node.
            // Reputation is strictly reserved for malicious protocol behavior (invalid signatures, etc).

            let failures = meta.failures;
            let is_seed = meta.source == PeerSource::Seed;

            tracing::debug!(
                "Peer {} dial/network failed (failures: {})",
                addr, failures
            );

            // If a peer is completely unreachable after many attempts, just remove it from known_peers
            // so we stop trying to dial it. We DO NOT set banned_until.
            if failures > 10 && !is_seed {
                tracing::debug!("Peer {} unreachable after 10 attempts, removing from discovery", addr);
            }
        }
        
        // Actually perform the removal outside the get_mut scope
        let should_remove = {
            if let Some(meta) = peers.get(&addr) {
                meta.failures > 10 && meta.source != PeerSource::Seed
            } else {
                false
            }
        };
        
        if should_remove {
            peers.remove(&addr);
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

    /// Get seeds + healthy UNVERIFIED peers for connection attempts (feeler/new-table candidates).
    /// These are NOT gossiped to the network — only verified peers are.
    pub async fn get_random_peers(&self, count: usize) -> Vec<SocketAddr> {
        use rand::seq::SliceRandom;

        let peers = self.known_peers.read().await;
        let now = chrono::Utc::now().timestamp();

        // Include all known peers (verified or not) as candidates to connect to.
        // If they are NAT/dead, the outbound connect will fail → mark_peer_failed → eventually removed.
        // If they succeed → update_peer_seen → promoted to verified.
        let mut candidates: Vec<SocketAddr> = peers
            .values()
            .filter(|meta| {
                let not_banned = meta.banned_until.is_none_or(|ban_until| now > ban_until);
                let not_dead = meta.failures < 5 && meta.reputation > -20;
                not_banned && not_dead
            })
            .map(|meta| meta.address)
            .collect();

        // Always include seeds as fallback
        if candidates.len() < count {
            for seed in &self.seed_nodes {
                if !candidates.contains(seed) {
                    candidates.push(*seed);
                }
            }
        }

        let mut rng = rand::thread_rng();
        candidates.shuffle(&mut rng);
        candidates.into_iter().take(count).collect()
    }

    /// ADDRMAN FIX v3.1.0-alpha (2026-08-20): Get only VERIFIED ("tried") peers for GetAddr gossip.
    /// These are IPs we have personally connected to outbound at least once.
    /// This is the Bitcoin rule: never gossip unverified IPs.
    pub async fn get_verified_peers(&self, count: usize) -> Vec<SocketAddr> {
        use rand::seq::SliceRandom;

        let peers = self.known_peers.read().await;
        let now = chrono::Utc::now().timestamp();

        let mut verified: Vec<SocketAddr> = peers
            .values()
            .filter(|meta| {
                let not_banned = meta.banned_until.is_none_or(|ban_until| now > ban_until);
                // Only return peers we've personally verified via outbound connection
                meta.verified && not_banned && meta.failures < 3
            })
            .map(|meta| meta.address)
            .collect();

        // Always include seeds in addr gossip — they are known-good by definition
        for seed in &self.seed_nodes {
            if !verified.contains(seed) {
                verified.push(*seed);
            }
        }

        let mut rng = rand::thread_rng();
        verified.shuffle(&mut rng);
        verified.into_iter().take(count).collect()
    }

    /// Check if peer is currently banned
    pub async fn is_banned(&self, addr: &SocketAddr) -> bool {
        let peers = self.known_peers.read().await;
        if let Some(meta) = peers.get(addr) {
            if let Some(ban_until) = meta.banned_until {
                let now = chrono::Utc::now().timestamp();
                return now < ban_until;
            }
        }
        false
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
                reputation: 50,
                banned_until: None,
                verified: true, // ADDRMAN: seeds are pre-verified — always safe to gossip
            });
        }

        info!("Bootstrapped with {} seed nodes", self.seed_nodes.len());
        self.seed_nodes.clone()
    }

    /// Process Addr message from peer (with spam protection)
    pub async fn process_addr_message(&self, addrs: Vec<SocketAddr>, max_addrs: usize) {
        if addrs.len() > max_addrs {
            warn!(
                "Received too many addresses ({}), capping to {}",
                addrs.len(),
                max_addrs
            );
        }

        let mut peers = self.known_peers.write().await;
        let now = chrono::Utc::now().timestamp();

        // SECURITY FIX: Bounded known_peers map to prevent OOM via malicious Addr floods
        if peers.len() > 5000 {
            return;
        }

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
                reputation: 0,
                banned_until: None,
                verified: false, // ADDRMAN: received via gossip = unverified ("new" table)
            });
        }
    }
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
            // Reject: 10.x.x.x, 192.168.x.x
            // ALLOW 172.16-31.x.x to support Docker bridge networks for local testnets!
            !(ipv4.octets()[0] == 10 || (ipv4.octets()[0] == 192 && ipv4.octets()[1] == 168))
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

    #[test]
    fn test_address_bucketing_distribution() {
        let discovery = PeerDiscovery::with_dns_seeds(vec![], vec![]);

        // Note: process_addr_message actually uses `is_routable_addr` which filters loopback/private IPs
        // To properly test locally, we need to bypass is_routable_addr or use public IPs.
        // Let's use public IPs for the test to ensure they are added to the buckets.
        let public_subnet1: Vec<SocketAddr> = vec![
            "8.8.8.1:8333".parse().unwrap(),
            "8.8.8.2:8333".parse().unwrap(),
            "8.8.8.3:8333".parse().unwrap(),
        ];
        let public_subnet2: Vec<SocketAddr> = vec![
            "1.1.1.1:8333".parse().unwrap(),
            "1.1.1.2:8333".parse().unwrap(),
        ];

        futures::executor::block_on(discovery.process_addr_message(public_subnet1, 10));
        futures::executor::block_on(discovery.process_addr_message(public_subnet2, 10));

        let selected = futures::executor::block_on(discovery.get_random_peers(2));
        assert!(selected.len() <= 2);
        assert!(
            !selected.is_empty(),
            "Should select peers from the discovery pool"
        );
    }

    /// DDoS Protection: 4 strikes trigger a 24-hour ban.
    #[tokio::test]
    async fn test_ddos_protection_bans_rapidly() {
        let discovery = PeerDiscovery::with_dns_seeds(vec![], vec![]);
        let malicious_ip: SocketAddr = "10.0.0.99:8333".parse().unwrap();

        discovery
            .add_peer_with_source(malicious_ip, PeerSource::Discovered)
            .await;

        assert!(!discovery.is_banned(&malicious_ip).await);
        let meta = discovery.get_peer_meta(&malicious_ip).await.unwrap();
        assert_eq!(meta.failures, 0);

        discovery.mark_peer_failed(malicious_ip).await;
        discovery.mark_peer_failed(malicious_ip).await;
        discovery.mark_peer_failed(malicious_ip).await;
        discovery.mark_peer_failed(malicious_ip).await; // triggers ban

        assert!(discovery.is_banned(&malicious_ip).await);
        let meta_after = discovery.get_peer_meta(&malicious_ip).await.unwrap();
        assert!(meta_after.failures >= 3);
        assert!(meta_after.reputation <= -20);
    }
}

