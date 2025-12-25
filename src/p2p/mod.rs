// P2P networking module for QUANTA blockchain
pub mod protocol;
pub mod peer;
pub mod network;
pub mod discovery;

pub use protocol::{P2PMessage, MessageHandler};
pub use peer::{Peer, PeerInfo};
pub use network::{Network, NetworkConfig};
pub use discovery::PeerDiscovery;
