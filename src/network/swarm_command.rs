use libp2p::Multiaddr;
use crate::network::protocol::P2PMessage;
use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub enum SwarmCommand {
    Broadcast(P2PMessage),
    SendTo(SocketAddr, P2PMessage),
    Dial(SocketAddr),
    Disconnect(SocketAddr),
}
