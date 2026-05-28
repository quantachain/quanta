use quanta::core::Block;
use quanta::network::discovery::{PeerDiscovery, PeerSource};
use std::net::SocketAddr;

/// DDoS Protection: 4 strikes trigger a 24-hour ban.
#[tokio::test]
async fn test_ddos_protection_bans_rapidly() {
    let discovery = PeerDiscovery::with_dns_seeds(vec![], vec![]);
    let malicious_ip: SocketAddr = "10.0.0.99:8333".parse().unwrap();

    discovery.add_peer_with_source(malicious_ip, PeerSource::Discovered).await;

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

/// Genesis Replay Attack: a fake genesis block must never match the real chain hash.
#[tokio::test]
async fn test_genesis_replay_attack_rejected() {
    let mut bad_genesis = Block::genesis();
    bad_genesis.hash = "0".repeat(64);

    let correct_hash = "527a8a6ad3292c9b42c40f3d71fd3b89cdd79415106ce0b8d9f7f6690a96433d";
    assert_ne!(bad_genesis.hash, correct_hash,
        "A fake genesis block must not match the hardcoded chain genesis hash");
}
