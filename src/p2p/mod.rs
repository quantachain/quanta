// P2P networking module for QUANTA blockchain
pub mod protocol;
pub mod peer;
pub mod network;
pub mod discovery;

pub use network::{Network, NetworkConfig};
