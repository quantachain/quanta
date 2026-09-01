use libp2p::{
    gossipsub, kad, request_response, swarm::NetworkBehaviour,
};
use crate::network::p2p_codec::QuantaCodec;

#[derive(NetworkBehaviour)]
pub struct QuantaBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,
    pub request_response: request_response::Behaviour<QuantaCodec>,
}
