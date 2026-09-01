pub mod discovery;
#[allow(clippy::module_inception)]
pub mod network;
pub mod peer;
pub mod protocol;
// PQC TRANSPORT v3.1.0-alpha (2026-08-20): TLS module wraps all P2P connections
// with TLS 1.3 + X25519MLKEM768 hybrid post-quantum key exchange.
pub mod tls;

pub use discovery::PeerDiscovery;
pub use network::{Network, NetworkConfig};
#[allow(unused_imports)]
pub use peer::{Peer, PeerManager};
#[allow(unused_imports)]
pub use protocol::P2PMessage;
pub mod swarm;
pub mod p2p_transport;
pub mod p2p_behaviour;
pub mod p2p_codec;
pub mod swarm_command;
