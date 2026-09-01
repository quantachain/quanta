use libp2p::{
    core::{
        muxing::StreamMuxerBox,
        transport::Boxed,
        upgrade,
    },
    tcp, yamux, Swarm, SwarmBuilder, PeerId, Transport,
};
use libp2p::identity;
use std::sync::Arc;
use std::time::Duration;
use crate::network::p2p_transport::QuantaAuth;
use crate::network::p2p_behaviour::QuantaBehaviour;
use libp2p::gossipsub;
use libp2p::kad;
use libp2p::request_response;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn build_swarm(
    node_id: String,
    server_config: Arc<rustls::ServerConfig>,
    client_config: Arc<rustls::ClientConfig>,
) -> Result<Swarm<QuantaBehaviour>, Box<dyn std::error::Error>> {
    let auth_upgrade = QuantaAuth {
        node_id: node_id.clone(),
        server_config,
        client_config,
    };

    let transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
        .upgrade(upgrade::Version::V1)
        .authenticate(auth_upgrade)
        .multiplex(yamux::Config::default())
        .timeout(Duration::from_secs(20))
        .boxed();

    // Setup Gossipsub
    let message_id_fn = |message: &gossipsub::Message| {
        let mut s = DefaultHasher::new();
        message.data.hash(&mut s);
        gossipsub::MessageId::from(s.finish().to_string())
    };

    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .heartbeat_interval(Duration::from_secs(1))
        .validation_mode(gossipsub::ValidationMode::Strict)
        .message_id_fn(message_id_fn)
        .build()
        .map_err(|e| format!("Failed to build gossipsub config: {}", e))?;

    let gossipsub = gossipsub::Behaviour::new(
        gossipsub::MessageAuthenticity::Anonymous,
        gossipsub_config,
    )
    .map_err(|e| format!("Failed to build gossipsub: {}", e))?;

    // Setup Kademlia
    let keypair = identity::Keypair::generate_ed25519();
    let local_peer_id = keypair.public().to_peer_id();
    let store = kad::store::MemoryStore::new(local_peer_id);
    let mut kademlia = kad::Behaviour::new(local_peer_id, store);
    kademlia.set_mode(Some(kad::Mode::Server));

    // Setup Request Response
    let request_response = request_response::Behaviour::new(
        [(
            "/quanta/reqresp/1.0.0",
            request_response::ProtocolSupport::Full,
        )],
        request_response::Config::default(),
    );

    let behaviour = QuantaBehaviour {
        gossipsub,
        kademlia,
        request_response,
    };

    let swarm = SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_other_transport(|_key| transport)
        .unwrap()
        .with_behaviour(|_key| behaviour)
        .unwrap()
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

    Ok(swarm)
}
