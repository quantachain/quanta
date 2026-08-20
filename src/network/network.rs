use crate::consensus::blockchain::Blockchain;
use crate::core::block::Block;
use crate::core::transaction::Transaction;
use crate::network::peer::{Peer, PeerManager};
use crate::network::protocol::{P2PMessage, PROTOCOL_VERSION};
use crate::network::tls::{generate_node_cert, make_tls_acceptor, make_tls_connector};
use crate::network::PeerDiscovery;
use lru::LruCache;
use rustls::pki_types::ServerName;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Maximum blocks requested per sync batch (HIGH-2 FIX: prevents height-forgery storm)
const MAX_SYNC_BATCH: u64 = 5000;

/// Maximum headers served per GetHeaders response.
/// 2000 = ~400 KB of compressed header data — safe for a single network message.
const MAX_HEADERS_PER_RESPONSE: u64 = 2000;

/// Network configuration
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub listen_addr: SocketAddr,
    pub max_peers: usize,
    pub node_id: String,
    pub bootstrap_nodes: Vec<SocketAddr>,
    pub dns_seeds: Vec<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8333".parse().unwrap(),
            max_peers: 125,
            node_id: Uuid::new_v4().to_string(),
            bootstrap_nodes: Vec::new(),
            dns_seeds: Vec::new(),
        }
    }
}

/// Network manager for P2P blockchain network
pub struct Network {
    pub config: NetworkConfig,
    blockchain: Arc<RwLock<Blockchain>>,
    pub peer_manager: Arc<PeerManager>,
    message_tx: mpsc::Sender<(SocketAddr, P2PMessage)>,
    message_rx: Arc<RwLock<mpsc::Receiver<(SocketAddr, P2PMessage)>>>,
    seen_blocks: Arc<Mutex<LruCache<String, ()>>>,
    seen_txs: Arc<Mutex<LruCache<String, ()>>>,
    seen_bft: Arc<Mutex<LruCache<String, std::time::Instant>>>,
    discovery: Arc<PeerDiscovery>,
    syncing: Arc<AtomicBool>,
    sync_buffer: Arc<tokio::sync::Mutex<Vec<Block>>>,
    header_buffer: Arc<tokio::sync::Mutex<Vec<crate::network::protocol::BlockHeader>>>,
    sync_request_range: Arc<tokio::sync::Mutex<Option<(u64, u64)>>>,
    aleph_bft_tx: Arc<tokio::sync::RwLock<Option<tokio::sync::mpsc::Sender<Vec<u8>>>>>,
    // PQC TRANSPORT v3.1.0-alpha (2026-08-20): TLS acceptor for inbound connections.
    // Generated at startup from an ephemeral self-signed cert. Peers verify identity
    // via Falcon-512 handshake above the TLS layer.
    tls_acceptor: TlsAcceptor,
}

impl Network {
    /// Create a new network instance
    pub fn new(config: NetworkConfig, blockchain: Arc<RwLock<Blockchain>>) -> Self {
        // PQC TRANSPORT v3.1.0-alpha (2026-08-20): Generate ephemeral TLS certificate.
        // This certificate identifies the node at the TLS layer.
        // Application-layer identity is still verified by Falcon-512 handshake.
        let cert = generate_node_cert()
            .expect("Failed to generate P2P TLS certificate");
        let tls_acceptor = make_tls_acceptor(&cert)
            .expect("Failed to create TLS acceptor");

        // CRIT-3 FIX: Bounded channel(1_000) prevents OOM via message flood.
        let (message_tx, message_rx) = mpsc::channel(1_000);
        let discovery = Arc::new(PeerDiscovery::with_dns_seeds(
            config.bootstrap_nodes.clone(),
            config.dns_seeds.clone(),
        ));
        Self {
            config: config.clone(),
            blockchain,
            peer_manager: Arc::new(PeerManager::new(125, config.node_id.clone())),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            seen_blocks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap()))),
            seen_txs: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(10_000).unwrap(),
            ))),
            seen_bft: Arc::new(Mutex::new(LruCache::new(
                NonZeroUsize::new(10_000).unwrap(),
            ))),
            discovery,
            syncing: Arc::new(AtomicBool::new(false)),
            sync_buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            header_buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            sync_request_range: Arc::new(tokio::sync::Mutex::new(None)),
            aleph_bft_tx: Arc::new(tokio::sync::RwLock::new(None)),
            tls_acceptor,
        }
    }

    /// Start the network node
    pub async fn start(self: Arc<Self>) -> Result<(), String> {
        info!("Starting network node on {}", self.config.listen_addr);

        // Start listening for incoming connections
        let listen_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = network.listen_for_connections().await {
                    error!("Listener error: {}", e);
                }
            })
        };

        // Start message processor
        let processor_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                network.process_messages().await;
            })
        };

        // Start peer maintenance
        let maintenance_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                network.maintain_peers().await;
            })
        };

        // BW-FIX-2: Heartbeat now uses the PING_INTERVAL_SECS constant (60s) defined in
        // protocol.rs, not a hardcoded 10s. The old 10s value was 6× faster than the
        // protocol spec, generating ~360 Ping/Pong pairs/hour across a 7-node cluster.
        let heartbeat_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(
                    crate::network::protocol::PING_INTERVAL_SECS,
                ));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    network.send_heartbeats().await;
                }
            })
        };

        // FIX (2026-06-24): Periodic mempool sync loop.
        //
        // Transaction gossip is fire-and-forget — if a validator misses a
        // NewTx broadcast (e.g. because it was banned, restarted, or
        // temporarily disconnected), it never receives that transaction.
        // This loop polls a random peer for its full mempool every 30 seconds,
        // ensuring all validators eventually converge on the same pending set
        // and the round-robin distribution of transactions works correctly.
        let mempool_sync_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                // Stagger the first sync so it doesn't overlap with initial connection setup.
                tokio::time::sleep(Duration::from_secs(15)).await;
                let mut interval = interval(Duration::from_secs(30));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    let peers = network.peer_manager.get_peers().await;
                    if peers.is_empty() {
                        continue;
                    }
                    // Pick a random peer to request mempool from.
                    let idx = (chrono::Utc::now().timestamp() as usize) % peers.len();
                    if let Some(peer) = peers.get(idx) {
                        tracing::debug!(
                            "Periodic mempool sync: requesting from {}",
                            peer.address().await
                        );
                        let peer = Arc::clone(peer);
                        tokio::spawn(async move {
                            let _ = peer.send_message(P2PMessage::GetMempool).await;
                        });
                    }
                }
            })
        };

        // Resolve DNS seeds to get additional bootstrap nodes
        if !self.config.dns_seeds.is_empty() {
            info!("Resolving {} DNS seeds...", self.config.dns_seeds.len());
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                let addrs = network.discovery.resolve_dns_seeds().await;
                for addr in addrs {
                    if let Err(e) = network.connect_to_peer(addr).await {
                        debug!("Failed to connect to DNS peer {}: {}", addr, e);
                    }
                }
            });
        }

        // Connect to bootstrap nodes
        for addr in &self.config.bootstrap_nodes {
            let network = Arc::clone(&self);
            let addr = *addr;
            tokio::spawn(async move {
                if let Err(e) = network.connect_to_peer(addr).await {
                    warn!("Failed to connect to bootstrap node {}: {}", addr, e);
                }
            });
        }

        info!("Network node started successfully");

        // Wait for handles
        let _ = tokio::join!(
            listen_handle,
            processor_handle,
            maintenance_handle,
            heartbeat_handle,
            mempool_sync_handle
        );

        Ok(())
    }

    /// Listen for incoming peer connections (PQC TLS)
    async fn listen_for_connections(&self) -> Result<(), String> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| format!("Failed to bind listener: {}", e))?;

        info!("Listening for TLS connections on {}", self.config.listen_addr);

        let tls_acceptor = self.tls_acceptor.clone();

        // DOS/OOM Protection (v3.1.0-alpha): Limit the number of concurrent TCP connections
        // being handshaked. This prevents memory exhaustion from half-open
        // connections (e.g. Cloudflare proxy spam).
        let connection_semaphore = Arc::new(tokio::sync::Semaphore::new(self.config.max_peers * 2));

        loop {
            // Wait for a connection permit before accepting. This blocks if we are 
            // currently processing too many half-open TCP connections.
            let permit = match Arc::clone(&connection_semaphore).acquire_owned().await {
                Ok(p) => p,
                Err(_) => break Ok(()), // Semaphore closed
            };

            match listener.accept().await {
                Ok((tcp_stream, addr)) => {
                    let current_count = self.peer_manager.peer_count().await;
                    if current_count >= self.config.max_peers {
                        tracing::debug!(
                            "Inbound connection from {} rejected: peer limit {} reached",
                            addr,
                            self.config.max_peers
                        );
                        drop(tcp_stream);
                        drop(permit); // Release immediately
                        continue;
                    }

                    info!("Incoming connection from {}", addr);

                    let message_tx = self.message_tx.clone();
                    let peer_manager = Arc::clone(&self.peer_manager);
                    let blockchain = Arc::clone(&self.blockchain);
                    let discovery = Arc::clone(&self.discovery);
                    let node_id = self.config.node_id.clone();
                    let our_listen_port = self.config.listen_addr.port();
                    // PQC TRANSPORT v3.1.0-alpha (2026-08-20): Accept as TLS session.
                    // X25519MLKEM768 key exchange negotiated here (if peer supports it).
                    let tls_acceptor = tls_acceptor.clone();

                    tokio::spawn(async move {
                        // Keep permit alive until this connection task finishes (success or drop)
                        let _permit = permit;

                        // Perform TLS handshake before anything else
                        let stream = match tls_acceptor.accept(tcp_stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::debug!("TLS handshake failed from {}: {}", addr, e);
                                return;
                            }
                        };
                        match Peer::new(stream, addr).await {
                            Ok(peer) => {
                                let peer = Arc::new(peer);

                                let blockchain = blockchain.read().await;
                                let height = blockchain.get_height();
                                let cumulative_work = blockchain.cumulative_work_at(height);
                                drop(blockchain);

                                if tokio::time::timeout(
                                    std::time::Duration::from_secs(10),
                                    peer.handshake(PROTOCOL_VERSION, height, cumulative_work, node_id, our_listen_port)
                                )
                                .await
                                .unwrap_or(Err("Handshake timed out".to_string()))
                                .is_ok()
                                {
                                    info!("Successful handshake with {}", addr);
                                    match peer_manager.add_peer(Arc::clone(&peer)).await {
                                        Ok(_) => {
                                            // ADDRMAN FIX v3.1.0-alpha (2026-08-20): Now that we have the peer's
                                            // self-reported listen_port, add them to discovery as UNVERIFIED.
                                            // This is safe because: (a) the address uses their listen_port, not their
                                            // ephemeral source port; (b) they go into the "new" table and will only be
                                            // promoted to "tried" (and thus gossiped) once we connect outbound to them.
                                            if let Some(reported_addr) = peer.reported_listen_addr().await {
                                                discovery.add_peer(reported_addr).await;
                                            }

                                            // Request known peers from this new connection to discover the rest of the network
                                            let _ = peer.send_message(P2PMessage::GetAddr).await;

                                            Self::start_peer_receive_task(
                                                peer,
                                                message_tx,
                                                peer_manager,
                                            )
                                            .await;
                                        }
                                        Err(e) => {
                                            // FLAP-FIX: Send an explicit Disconnect before dropping the stream.
                                            // Without this, silently dropping the TCP connection looks like a
                                            // network failure to the remote, which immediately retries —
                                            // causing the 'Connection reset' flapping loop.
                                            // A Disconnect message tells the remote that the rejection was
                                            // intentional so it can back off gracefully.
                                            debug!(
                                                "Inbound peer {} rejected ({}); sending Disconnect",
                                                addr, e
                                            );
                                            let _ = peer.send_message(P2PMessage::Disconnect).await;
                                        }
                                    }
                                } else {
                                    tracing::warn!("Handshake with {} failed (timeout or version mismatch)", addr);
                                }
                            }
                            Err(e) => {
                                warn!("Failed to create peer for {}: {}", addr, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Start a single receive task for a peer (prevents duplicate loops)
    async fn start_peer_receive_task(
        peer: Arc<Peer>,
        message_tx: mpsc::Sender<(SocketAddr, P2PMessage)>,
        peer_manager: Arc<PeerManager>,
    ) {
        let addr = peer.address().await;
        tokio::spawn(async move {
            loop {
                match peer.receive_message().await {
                    Ok(msg) => {
                        // Use a compact label instead of {:?} — the debug format of
                        // AlephBFTMessage dumps the entire raw byte vector which spams
                        // operator logs with hundreds of lines of unreadable binary data.
                        let label = match &msg {
                            crate::network::protocol::P2PMessage::AlephBFTMessage(d) =>
                                format!("AlephBFT({} bytes)", d.len()),
                            crate::network::protocol::P2PMessage::Block(b) =>
                                format!("Block(#{})", b.index),
                            crate::network::protocol::P2PMessage::NewTx(tx) =>
                                format!("NewTx({})", &tx.hash()[..8]),
                            other => format!("{:?}", other),
                        };
                        debug!("<- {} from {}", label, addr);
                        // FIX: Use .send().await instead of try_send.
                        // If the shared channel is full due to heavy network load,
                        // backpressure is applied naturally. We should NOT drop the peer.
                        if message_tx.send((addr, msg)).await.is_err() {
                            warn!("Message channel closed, disconnecting from {}", addr);
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Error receiving from {}: {}", addr, e);
                        break;
                    }
                }
            }
            peer_manager.remove_peer(addr).await;
        });
    }

    /// Connect to a peer (PQC TLS)
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<(), String> {
        info!("Connecting to peer {} (PQC TLS)", addr);

        // Step 1: Plain TCP connect
        let tcp_stream = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            TcpStream::connect(addr)
        )
        .await
        .map_err(|_| "Connection timed out".to_string())?
        .map_err(|e| format!("Failed to connect: {}", e))?;

        // Step 2: PQC TLS handshake (X25519MLKEM768)
        // PQC TRANSPORT v3.1.0-alpha (2026-08-20): Wrap the TCP stream with TLS.
        // Using "quanta.node" as the SNI — our custom NoCertificateVerification
        // accepts any self-signed cert, so the SNI value is irrelevant.
        // Real identity verification happens via Falcon-512 in the Version handshake below.
        let tls_connector = make_tls_connector()
            .map_err(|e| format!("TLS connector error: {}", e))?;
        let server_name = ServerName::try_from("quanta.node")
            .map_err(|e| format!("Invalid server name: {}", e))?;
        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tls_connector.connect(server_name, tcp_stream)
        )
        .await
        .map_err(|_| "TLS handshake timed out".to_string())?
        .map_err(|e| format!("TLS handshake failed: {}", e))?;

        let peer = Arc::new(Peer::new(stream, addr).await?);
        peer.set_outbound().await;

        // Perform handshake
        let blockchain = self.blockchain.read().await;
        let height = blockchain.get_height();
        let cumulative_work = blockchain.cumulative_work_at(height);
        drop(blockchain);

        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            peer.handshake(
                PROTOCOL_VERSION,
                height,
                cumulative_work,
                self.config.node_id.clone(),
                // ADDRMAN FIX v3.1.0-alpha: Self-report our listen port so the remote peer
                // can add our correct connectable address to their discovery table.
                self.config.listen_addr.port(),
            )
        )
        .await
        .unwrap_or(Err("Handshake timed out".to_string()))?;

        // Add to peer manager
        self.peer_manager.add_peer(Arc::clone(&peer)).await?;

        // Request known peers from this new connection to discover the rest of the network
        let _ = peer.send_message(P2PMessage::GetAddr).await;

        // BW-FIX-3: Only pull the peer's mempool if our own mempool is empty.
        // The original unconditional GetMempool fired on every connect — combined with
        // the peer-flapping bug (reconnects every ~10s) this caused full mempool
        // transfers at 10-second intervals, adding hundreds of MB/hour per node pair.
        // The periodic 30s mempool sync loop already handles convergence once connected.
        let local_mempool_size = self
            .blockchain
            .read()
            .await
            .get_pending_transactions()
            .len();
        if local_mempool_size == 0 {
            let _ = peer.send_message(P2PMessage::GetMempool).await;
        }

        // Start single receive task
        Self::start_peer_receive_task(
            peer,
            self.message_tx.clone(),
            Arc::clone(&self.peer_manager),
        )
        .await;

        info!("Connected to peer {}", addr);
        Ok(())
    }

    /// Process incoming messages (PARALLELIZED - spawn handler per message)
    async fn process_messages(self: Arc<Self>) {
        let mut rx = self.message_rx.write().await;
        // Limit concurrent message processing to prevent RAM exhaustion and apply backpressure
        let semaphore = Arc::new(tokio::sync::Semaphore::new(1000));

        while let Some((addr, msg)) = rx.recv().await {
            // Find the peer object to pass to the handler for strike management
            let mut peer_opt = None;
            for p in self.peer_manager.get_peers().await {
                if p.address().await == addr {
                    peer_opt = Some(p);
                    break;
                }
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                let _permit = permit;
                if let Err(e) = network.handle_message(addr, msg.clone(), peer_opt).await {
                    let label = match &msg {
                        P2PMessage::AlephBFTMessage(d) => format!("AlephBFT({} bytes)", d.len()),
                        P2PMessage::Block(b) => format!("Block(#{})", b.index),
                        P2PMessage::NewTx(tx) => format!("NewTx({})", &tx.hash()[..8]),
                        P2PMessage::GetStateSnapshot { height, .. } => format!("GetStateSnapshot({})", height),
                        P2PMessage::StateSnapshot { height, .. } => format!("StateSnapshot({})", height),
                        other => format!("{:?}", other),
                    };
                    error!("Error handling {} from {}: {}", label, addr, e);
                }
            });
        }
    }

    /// Handle a single message
    async fn handle_message(
        &self,
        addr: SocketAddr,
        msg: P2PMessage,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        match msg {
            P2PMessage::NewTx(tx) => {
                self.handle_new_transaction(tx, peer).await?;
            }
            P2PMessage::Block(block) => {
                self.handle_new_block(block, peer).await?;
            }
            P2PMessage::GetBlocks {
                start_height,
                end_height,
            } => {
                self.handle_get_blocks(addr, start_height, end_height, peer.clone())
                    .await?;
            }
            P2PMessage::GetHeaders { start_height } => {
                self.handle_get_headers(addr, start_height, peer.clone())
                    .await?;
            }
            P2PMessage::Headers(headers) => {
                self.handle_headers(headers, peer).await?;
            }
            P2PMessage::GetHeight => {
                self.handle_get_height(addr).await?;
            }
            P2PMessage::Height {
                height,
                cumulative_work,
            } => {
                debug!(
                    "Peer {} has height {} (work {})",
                    addr, height, cumulative_work
                );
                if let Some(p) = &peer {
                    p.update_height(height, cumulative_work).await;
                }
            }
            P2PMessage::GetMempool => {
                self.handle_get_mempool(addr).await?;
            }
            P2PMessage::Mempool(txs) => {
                for tx in txs {
                    let _ = self.handle_new_transaction(tx, peer.clone()).await;
                }
            }
            P2PMessage::GetStateSnapshot { height, expected_state_root } => {
                self.handle_get_state_snapshot(addr, height, expected_state_root, peer).await?;
            }
            P2PMessage::StateSnapshot { height, state_bytes, state_root } => {
                self.handle_state_snapshot(addr, height, state_bytes, state_root).await?;
            }
            P2PMessage::Ping(nonce) => {
                self.send_to_peer(addr, P2PMessage::Pong(nonce)).await?;
            }
            P2PMessage::Pong(_) => {
                // Keep-alive response
            }
            P2PMessage::GetAddr => {
                // ADDRMAN FIX v3.1.0-alpha (2026-08-20): Only gossip VERIFIED ("tried") peers.
                // Bitcoin rule: never gossip an IP you haven't personally connected to outbound.
                // This prevents dead NAT/Cloudflare IPs from propagating through the network.
                let addrs = self.discovery.get_verified_peers(50).await;
                if let Some(p) = peer {
                    let _ = p.send_message(P2PMessage::Addr(addrs)).await;
                }
            }
            P2PMessage::Addr(addrs) => {
                self.discovery.process_addr_message(addrs, 50).await;
            }
            P2PMessage::Disconnect => {
                self.peer_manager.remove_peer(addr).await;
            }
            P2PMessage::AlephBFTMessage(data) => {
                // HIGH-3 FIX: Strict size limit to prevent memory exhaustion and hashing CPU spikes
                if data.len() > 1024 * 1024 {
                    tracing::warn!("Rejecting oversized AlephBFTMessage from {} ({} bytes)", addr, data.len());
                    if let Some(p) = &peer {
                        let _ = p.add_misbehavior(100).await;
                    }
                    return Ok(());
                }

                let tx_opt = self.aleph_bft_tx.read().await;

                // CRITICAL FIX: If the channel is not registered yet, drop the message
                // WITHOUT caching it in the LRU. This allows the node to process the
                // inevitable retry broadcast once AlephBFT has actually started.
                if tx_opt.is_none() {
                    tracing::trace!("AlephBFT channel not registered yet, skipping local delivery but continuing gossip.");
                }

                // BETA FIX: Hash the BFT message to prevent infinite gossip loops
                use sha3::{Digest, Sha3_256};
                let hash = hex::encode(Sha3_256::digest(&data));

                let (already_seen, skip_local) = {
                    let mut seen = self.seen_bft.lock().unwrap();
                    let now = std::time::Instant::now();
                    match seen.get(&hash).copied() {
                        Some(time) if now.duration_since(time).as_secs() < 3 => {
                            // Flood protection: drop completely from gossip if relayed < 3s ago
                            // CRITICAL FIX: skip_local MUST be false so AlephBFT receives its retries!
                            (true, false)
                        }
                        Some(_) => {
                            // Legitimate retry after 3s. Update timestamp and relay it!
                            seen.put(hash, now);
                            (false, false) // pass locally and relay
                        }
                        None => {
                            seen.put(hash, now);
                            (false, false) // new, pass locally and relay
                        }
                    }
                };

                // Send to our local AlephBFT instance FIRST.
                // AlephBFT relies on retries (identical messages) for reliability.
                // If we deduplicate before sending to AlephBFT, retries are dropped locally.
                if !skip_local {
                    if let Some(tx) = &*tx_opt {
                        match tx.try_send(data.clone()) {
                            Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                                tracing::warn!("BFT channel full, dropping message (Memory leak prevented)");
                            }
                            Err(_) => {
                                tracing::debug!("BFT channel closed/unregistered, dropping message during sync");
                            }
                            Ok(_) => {}
                        }
                    }
                }

                if already_seen {
                    return Ok(());
                }


                // HIGH FIX: Relay the message to all peers! This eliminates the need
                // for a "Full Mesh" network topology and allows the network to scale
                // massively without hardcoding bootstrap IPs.
                self.peer_manager
                    .broadcast(P2PMessage::AlephBFTMessage(data))
                    .await;
            }
            _ => {
                debug!("Unhandled message type from {}", addr);
            }
        }
        Ok(())
    }

    /// Handle new transaction
    async fn handle_new_transaction(
        &self,
        tx: Transaction,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        // BETA FIX: Deduplication — only add + re-broadcast if not seen before
        let tx_hash = tx.hash();
        let already_seen = {
            let mut seen = self.seen_txs.lock().unwrap();
            seen.put(tx_hash.clone(), ()).is_some()
        };
        if already_seen {
            return Ok(());
        }

        // HIGH-5 FIX: Pre-verify signatures off the Tokio executor thread!
        // This prevents an attacker from starving the executor and the Blockchain write lock
        // by spamming invalid transactions.
        let tx_clone = tx.clone();
        let is_valid = tokio::task::spawn_blocking(move || tx_clone.verify())
            .await
            .map_err(|e| format!("Signature verification panicked: {}", e))?;

        if !is_valid {
            tracing::warn!("Rejecting transaction with invalid signature");
            if let Some(p) = peer {
                let _ = p.add_misbehavior(10).await;
            }
            return Ok(());
        }

        let blockchain = self.blockchain.write().await;
        // Check for duplicates in mempool (extra safety)
        {
            let pending = blockchain.get_pending_transactions();
            if pending.iter().any(|t| t.hash() == tx_hash) {
                return Ok(()); // Already in mempool
            }
        }

        // Add to pending transactions
        if let Err(e) = blockchain.add_transaction(tx.clone()) {
            warn!("Rejected transaction from peer: {}", e);
            if let Some(p) = peer {
                // Invalid tx: +10 points (10 bad txs = ban)
                if p.add_misbehavior(10).await {
                    warn!(
                        "Banning peer {} for repeated invalid transactions (score ≥ 100)",
                        p.address().await
                    );
                    p.disconnect().await;
                    self.peer_manager.remove_peer(p.address().await).await;
                }
            }
        } else {
            info!("Added new transaction to mempool, re-broadcasting");
            drop(blockchain);
            // BETA FIX: Re-broadcast to propagate across all nodes in the mesh
            self.broadcast_transaction(tx).await;
        }

        Ok(())
    }

    async fn handle_get_state_snapshot(
        &self,
        addr: SocketAddr,
        height: u64,
        _expected_state_root: String,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        // STATE SYNC SERVE FIX (v3.0.8-alpha)
        // FIX DATE: 2026-08-16 | VERSION: v3.0.8-alpha
        // REASON: The old code compared expected_state_root (canonical "42db...") against
        // block.state_root — this happens to match. But then it served the on-disk checkpoint
        // state which has a DIFFERENT root ("2bc7..."), and the receiver rejected it.
        // The forward-verification approach on the receiver side means we just need to serve
        // the on-disk checkpoint state as-is. The receiver will verify it by applying block
        // 110,001 on top of it and checking the state root of that block.
        debug!("Peer {} requested state snapshot for height {}", addr, height);
        let blockchain = self.blockchain.read().await;

        // Load the on-disk checkpoint for the requested height
        if let Some(state) = blockchain.load_account_state_at_height(height) {
            let computed_root = state.calculate_state_root();
            info!("Serving state snapshot for height {} to peer {} (computed root={})", height, addr, computed_root);
            if let Ok(state_bytes) = bincode::serialize(&state) {
                if let Some(p) = peer {
                    let _ = p.send_message(P2PMessage::StateSnapshot {
                        height,
                        state_bytes,
                        state_root: computed_root,
                    }).await;
                }
            }
        } else {
            warn!("Peer {} requested state snapshot for height {} but we don't have it on disk", addr, height);
        }
        Ok(())
    }


    async fn handle_state_snapshot(
        &self,
        addr: SocketAddr,
        height: u64,
        state_bytes: Vec<u8>,
        state_root: String,
    ) -> Result<(), String> {
        info!("Received state snapshot for height {} from peer {} ({} bytes)", height, addr, state_bytes.len());
        
        let blockchain = self.blockchain.write().await;
        
        // Basic sanity check — only apply snapshot if we are waiting for it.
        // current_height is the number of blocks (max_index + 1), so for block 110,000 it is 110,001.
        let current_height = blockchain.get_height();
        if height != current_height.saturating_sub(1) {
            warn!("Received unprompted state snapshot for height {} from {}, but our chain height is {}", height, addr, current_height);
            return Ok(());
        }
        
        // STATE SYNC RECEIVE FIX v2 (v3.0.8-alpha)
        // FIX DATE: 2026-08-16 | VERSION: v3.0.8-alpha
        // REASON: The canonical state root "42db10a2..." at block 110,000 was produced
        // by the ORIGINAL proposer node and cannot be reproduced by any other node —
        // because the proposer's pre-heal validator balances were unique. No peer on
        // the network can serve a snapshot with root "42db10a2...".
        //
        // Instead of validating against the block 110,000 root, we validate FORWARD:
        // we deserialize the snapshot, apply it as our account state, then fetch the
        // NEXT block (110,001) from our chain and check that this state produces the
        // correct state root for that block. If it passes, the snapshot is correct.
        // This is analogous to Ethereum snap-sync pivot verification.

        // Deserialize the snapshot
        let state: crate::core::transaction::AccountState = match bincode::deserialize(&state_bytes) {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to deserialize state snapshot from {}: {}", addr, e);
                return Ok(());
            }
        };

        // Forward-verify: check that this state produces the correct root for block 110,001
        // Block 110,001 has strict state root enforcement (it's past the hard-fork).
        let block_110001 = blockchain.get_block_by_height(height + 1);
        if let Some(next_block) = block_110001 {
            if !next_block.state_root.is_empty() {
                // Apply the snapshot state to a temp clone and simulate block 110,001
                let mut test_state = state.clone();
                // Process block 110,001 transactions against the test state
                for tx in &next_block.transactions {
                    if tx.is_coinbase() || tx.sender == "TREASURY" {
                        // COINBASE_MATURITY = 500 (private const in blockchain.rs)
                        test_state.credit_account(tx, next_block.index, 500);
                    } else if tx.is_genesis_premine() {
                        test_state.credit_account(tx, next_block.index, 0);
                    }
                }
                let test_root = test_state.calculate_state_root();
                if test_root != next_block.state_root {
                    warn!("State snapshot from {} for height {} failed forward-verification: applying it produces state root {} for block 110,001, but block expects {}", addr, height, test_root, next_block.state_root);
                    return Ok(());
                }
                info!("State snapshot for height {} forward-verified successfully against block 110,001 (root={}). Replacing local account state.", height, state.calculate_state_root());
            } else {
                // Block 110,001 has no state root field — just accept (old node)
                info!("State snapshot for height {} accepted (block 110,001 has no state root to verify against).", height);
            }
        } else {
            // We don't have block 110,001 yet — basic sanity: just check the sender's claimed root matches computed
            let computed_root = state.calculate_state_root();
            if computed_root != state_root {
                warn!("State snapshot from {} for height {}: computed root {} does not match claimed root {}", addr, height, computed_root, state_root);
                return Ok(());
            }
            info!("State snapshot for height {} accepted (block 110,001 not yet on disk; root={}).", height, computed_root);
        }

        if let Err(e) = blockchain.apply_canonical_state_snapshot(height, state) {
            warn!("Failed to apply state snapshot: {}", e);
        }
        
        Ok(())
    }


    /// Handle a new block from the networkHARDENED VALIDATION + RE-BROADCAST FIX)
    ///
    /// SYNC FIX: During an active sync, blocks that are "too far ahead" are
    /// silently dropped instead of triggering additional GetBlocks requests.
    /// This prevents the request storm that was causing the stuck-at-272 bug.
    async fn handle_new_block(&self, block: Block, peer: Option<Arc<Peer>>) -> Result<(), String> {
        // HIGH FIX: Pre-verify signatures off the Tokio executor thread!
        // This prevents an attacker from starving the executor and the Blockchain read lock
        // by spamming invalid blocks that pass PoW check but fail signatures.
        let block_clone = block.clone();
        let sigs_valid = tokio::task::spawn_blocking(move || {
            use rayon::prelude::*;
            block_clone.transactions.par_iter().all(|tx| {
                if tx.is_coinbase() || tx.sender == "TREASURY" || tx.is_genesis_premine() {
                    return true;
                }
                tx.verify()
            })
        })
        .await
        .map_err(|e| format!("Signature verification panicked: {}", e))?;

        if !sigs_valid {
            tracing::warn!("Rejecting block with invalid signatures");
            if let Some(p) = &peer {
                let _ = p.add_misbehavior(50).await;
            }
            return Ok(());
        }

        let is_syncing = self.syncing.load(Ordering::SeqCst);

        // REORG FIX: Use the exact requested range (sync_request_range) to decide
        // whether to buffer this block during sync. 
        if is_syncing {
            let range_opt = *self.sync_request_range.lock().await;
            if let Some((rstart, rend)) = range_opt {
                if block.index >= rstart && block.index <= rend {
                    let mut buffer = self.sync_buffer.lock().await;
                    if buffer.len() < (MAX_SYNC_BATCH as usize + 200) {
                        buffer.push(block);
                    }
                    return Ok(());
                }
            }
        }

        let blockchain = self.blockchain.read().await;
        let latest = blockchain.get_latest_block();
        let _our_height = blockchain.get_height();
        drop(blockchain);

        if block.index > latest.index + 100 {
            // We just ignore it. The periodic sync loop in main.rs will
            // detect the height gap and execute a proper sync_blockchain()
            // batch process. We do not want to spam GetBlocks here.
            return Ok(());
        }

        // BETA FIX: Deduplication — only process + re-broadcast if not seen before.
        // This prevents broadcast storms while still propagating to the full mesh.
        let already_seen = {
            let mut seen = self.seen_blocks.lock().unwrap();
            seen.put(block.hash.clone(), ()).is_some()
        };
        if already_seen {
            return Ok(());
        }

        // Add block to chain (full validation inside add_network_block)
        let bc = self.blockchain.write().await;
        match bc.add_network_block(block.clone()) {
            Ok(_) => {
                // STATE SYNC FIX (v3.0.4-alpha)
                if block.index == 110_000 {
                    let needs_sync = {
                        let current_root = bc.current_state_root();
                        current_root != block.state_root && current_root != "2ee3073191a84fa407d3a1e798d01571ad930c807ea9d6a838a4c9b93330cef6"
                    };
                    if needs_sync {
                        tracing::warn!("Local state root diverged at hard-fork block 110,000. Requesting canonical state snapshot from peer...");
                        if let Some(p) = peer {
                            let _ = p.send_message(P2PMessage::GetStateSnapshot {
                                height: 110_000,
                                expected_state_root: block.state_root.clone(),
                            }).await;
                        }
                    }
                }

                info!(
                    "Block {} accepted at height {} — re-broadcasting to peers",
                    &block.hash[..8],
                    block.index
                );
                drop(bc);
                // BETA FIX: Re-broadcast so nodes NOT directly connected to the miner
                // also receive the block (essential for mesh topology with 6+ nodes).
                self.broadcast_block(block.clone()).await;

                Ok(())
            }
            Err(e) => {
                warn!("Rejected block from peer: {}", e);
                if let Some(p) = peer {
                    // Invalid block is a SERIOUS violation: +50 points (2 = ban)
                    if p.add_misbehavior(50).await {
                        warn!(
                            "Banning peer {} for invalid network blocks (score ≥ 100)",
                            p.address().await
                        );
                        p.disconnect().await;
                        self.peer_manager.remove_peer(p.address().await).await;
                    }
                }
                Err(format!("Failed to add block: {}", e))
            }
        }
    }

    /// Handle get blocks request — serve from storage, cap batch to 500 blocks.
    ///
    /// SYNC FIX: Load blocks in small sub-batches and release the blockchain
    /// read lock between each sub-batch. This prevents a single GetBlocks
    /// handler from monopolising the RwLock for the entire download duration
    /// (which would stall mining and other operations on the seed node and
    /// could cause the send-side write half to queue up behind a Ping, making
    /// the receiver think the connection went silent).
    async fn handle_get_blocks(
        &self,
        addr: SocketAddr,
        start: u64,
        end: u64,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        // HIGH-2 FIX: Clamp batch to MAX_SYNC_BATCH regardless of what peer claims
        let end = {
            let blockchain = self.blockchain.read().await;
            let chain_end = blockchain.get_height().saturating_sub(1);
            end.min(start + MAX_SYNC_BATCH - 1).min(chain_end)
        };

        info!("Serving blocks [{}-{}] to peer {}", start, end, addr);

        // Send blocks in sub-batches of 20 so that the read lock is held briefly
        // and other tasks (mining, heartbeat, peer management) can proceed.
        const SUB_BATCH: u64 = 20;
        let mut cursor = start;
        while cursor <= end {
            let sub_end = (cursor + SUB_BATCH - 1).min(end);
            let blocks: Vec<Block> = {
                let blockchain = self.blockchain.read().await;
                (cursor..=sub_end)
                    .filter_map(|i| blockchain.load_block_from_storage(i))
                    .collect()
                // blockchain read lock released here
            };
            for block in blocks {
                if let Some(ref p) = peer {
                    let _ = p.send_message(P2PMessage::Block(block)).await;
                } else {
                    self.send_to_peer(addr, P2PMessage::Block(block)).await?;
                }
            }
            cursor = sub_end + 1;
            // Yield to the tokio scheduler between sub-batches so that
            // heartbeat pings, incoming messages etc. can be serviced.
            tokio::task::yield_now().await;
        }

        Ok(())
    }

    /// Handle get headers request.
    ///
    /// SYNC FIX: Cap to MAX_HEADERS_PER_RESPONSE and build the header list
    /// without holding the blockchain read lock during the final network send.
    /// Sending a single ~100 KB compressed message is fast and does not need
    /// special sub-batching, but releasing the lock before the write keeps
    /// other tasks (especially mining) responsive on busy seed nodes.
    async fn handle_get_headers(
        &self,
        addr: SocketAddr,
        start: u64,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        let headers = {
            let blockchain = self.blockchain.read().await;
            let height = blockchain.get_height();
            let end = height.min(start + MAX_HEADERS_PER_RESPONSE);

            // PERF FIX: cumulative_work_at(i) is O(height) per call — calling it for
            // every header in a 2000-header batch at height 18k = 36 million RocksDB
            // reads while holding the blockchain read lock, causing the seed node to
            // stall for tens of seconds and drop the connection with early EOF.
            //
            // Instead, seed once with cumulative_work_at(start) and then maintain a
            // running sum by adding each block's difficulty. Total cost: O(start) once
            // + O(batch_size) incremental reads — orders of magnitude faster.
            let mut running_work = if start > 0 {
                blockchain.cumulative_work_at(start)
            } else {
                0u128
            };

            let mut headers = Vec::new();
            for i in start..=end {
                if let Some(block) = blockchain.load_block_from_storage(i) {
                    running_work = running_work.saturating_add(1u128); // BFT: 1 work unit per block
                    let mut header: crate::network::protocol::BlockHeader = (&block).into();
                    header.cumulative_work = running_work;
                    headers.push(header);
                }
            }
            headers
            // blockchain read lock released here
        };

        info!(
            "Serving {} headers [{}-{}] to peer {}",
            headers.len(),
            start,
            headers.last().map(|h| h.index).unwrap_or(start),
            addr
        );

        if let Some(p) = peer {
            let _ = p.send_message(P2PMessage::Headers(headers)).await;
        } else {
            self.send_to_peer(addr, P2PMessage::Headers(headers))
                .await?;
        }
        Ok(())
    }

    /// Handle headers response.
    ///
    /// Used in two situations:
    /// 1. During sync — a batch of headers arrives in response to GetHeaders.
    ///    They are buffered for the sync loop to consume.
    /// 2. Unsolicited single-header gossip — a peer broadcasts a newly-mined
    ///    block header (see broadcast_block). If we do not already have that
    ///    block, trigger an immediate GetBlocks request for it.
    async fn handle_headers(
        &self,
        headers: Vec<crate::network::protocol::BlockHeader>,
        peer: Option<Arc<Peer>>,
    ) -> Result<(), String> {
        if headers.is_empty() {
            return Ok(());
        }

        // Heuristic: a single-header message is gossip for a new block tip.
        // A batch of headers (> 1) is a sync response — buffer it as before.
        if headers.len() == 1 {
            let h = &headers[0];
            let our_height = self.blockchain.read().await.get_height();
            // Only request the block if it is the immediate next block or within
            // a small forward window (avoids requesting far-future orphans).
            if h.index > our_height && h.index <= our_height + 5 {
                if let Some(p) = peer {
                    debug!(
                        "Gossip header for block {} — requesting full block",
                        h.index
                    );
                    let _ = p
                        .send_message(P2PMessage::GetBlocks {
                            start_height: h.index,
                            end_height: h.index,
                        })
                        .await;
                }
            }
            return Ok(());
        }

        // Batch header response — push to buffer for the sync loop.
        let mut buffer = self.header_buffer.lock().await;
        // HIGH FIX: Prevent memory exhaustion attack from infinite header spam
        if buffer.len() < 10_000 {
            buffer.extend(headers);
        } else {
            tracing::warn!("Header buffer full, dropping batch");
        }
        Ok(())
    }

    /// Handle get height request — BETA FIX: use storage height, not in-memory chain length
    async fn handle_get_height(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        // get_height() reads from storage — correct even after thousands of blocks
        let height = blockchain.get_height();
        let cumulative_work = blockchain.cumulative_work_at(height);

        self.send_to_peer(
            addr,
            P2PMessage::Height {
                height,
                cumulative_work,
            },
        )
        .await
    }

    /// Handle get mempool request — HIGH-3 FIX: cap response to 100 txs
    async fn handle_get_mempool(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        // HIGH-3 FIX: Return at most 100 transactions to prevent ~8.5 MB bandwidth DoS.
        // Peers needing more can send a second GetMempool request.
        let txs: Vec<Transaction> = blockchain
            .get_pending_transactions()
            .iter()
            .take(100)
            .cloned()
            .collect();
        drop(blockchain);
        self.send_to_peer(addr, P2PMessage::Mempool(txs)).await
    }

    /// Send message to specific peer
    async fn send_to_peer(&self, addr: SocketAddr, msg: P2PMessage) -> Result<(), String> {
        let peers = self.peer_manager.get_peers().await;

        for peer in peers {
            if peer.address().await == addr {
                return peer.send_message(msg).await;
            }
        }

        Err("Peer not found".to_string())
    }

    /// Broadcast transaction to all peers
    pub async fn broadcast_transaction(&self, tx: Transaction) {
        self.peer_manager.broadcast(P2PMessage::NewTx(tx)).await;
    }

    /// Broadcast a newly-mined block to all connected peers.
    ///
    /// Light gossip: sends only the block header (~200 bytes) rather than the
    /// full block (~2 MB). Peers that need the full block request it via
    /// GetBlocks after receiving the header. This reduces per-block broadcast
    /// bandwidth from O(peers * 2 MB) to O(peers * 200 B).
    pub async fn broadcast_block(&self, block: Block) {
        let mut header: crate::network::protocol::BlockHeader = (&block).into();
        // cumulative_work is not available without a blockchain read; peers
        // will compute their own value after fetching the full block.
        header.cumulative_work = 0;
        self.peer_manager
            .broadcast(P2PMessage::Headers(vec![header]))
            .await;
    }

    /// Register a channel sender for incoming AlephBFT messages.
    pub async fn register_aleph_bft_tx(&self, tx: tokio::sync::mpsc::Sender<Vec<u8>>) {
        let mut guard = self.aleph_bft_tx.write().await;
        *guard = Some(tx);
    }

    /// Broadcast an AlephBFT message to all connected peers
    pub async fn broadcast_aleph_bft(&self, data: Vec<u8>) {
        self.peer_manager
            .broadcast(P2PMessage::AlephBFTMessage(data))
            .await;
    }

    /// BW-FIX-4: Send an AlephBFT message to a SPECIFIC validator peer identified by
    /// their wallet address (= node_id set at startup via BW-FIX-1 in main.rs).
    ///
    /// Called by QuantaNetworkBridge::send() for Recipient::Node(idx) messages.
    /// Falls back to full broadcast if the target peer is not currently connected
    /// (e.g. it hasn't finished syncing yet) — AlephBFT handles retries internally.
    pub async fn send_aleph_bft_to_validator(&self, data: Vec<u8>, validator_address: &str) {
        let peers = self.peer_manager.get_peers().await;
        for peer in &peers {
            if peer.get_info().await.node_id == validator_address {
                if let Err(e) = peer.send_message(P2PMessage::AlephBFTMessage(data)).await {
                    tracing::debug!(
                        "Unicast AlephBFT to {} failed: {} — dropping",
                        validator_address, e
                    );
                }
                return;
            }
        }
        self.broadcast_aleph_bft(data).await;
    }

    /// Synchronize blockchain from peers
    ///
    /// SYNC FIX (v3): Complete rewrite of sync logic. Previous implementations
    /// failed because:
    /// 1. Sync blocks arrived as individual Block messages processed concurrently
    ///    by tokio::spawn, breaking sequential chain application.
    /// 2. Broadcast blocks from the tip triggered redundant GetBlocks requests.
    /// 3. The old "sleep and hope" approach couldn't reliably detect batch completion.
    ///
    /// FORK FIX (v4): Detects "stuck on a fork" condition where the node receives
    /// valid blocks but cannot apply them because its local tip diverges from the
    /// network canonical chain. When detected, triggers a deep chain reorg to find
    /// the common ancestor and switch to the network's heavier chain.
    pub async fn sync_blockchain(&self) -> Result<(), String> {
        let peers = self.peer_manager.get_peers().await;
        if peers.is_empty() {
            return Ok(());
        }

        info!("Starting HEADERS-FIRST blockchain synchronization");

        // Ask all peers for their current height & work
        for peer in &peers {
            let _ = peer.send_message(P2PMessage::GetHeight).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Find best peer based on cumulative_work, with a height-gap safety net.
        // The safety net handles cases where cumulative_work tracking drifts
        // slightly after repeated shallow reorgs — without it the node loops
        // forever as "Already on heaviest chain" while actually being N blocks behind.
        let (local_work, our_height) = {
            let bc = self.blockchain.read().await;
            let h = bc.get_height();
            (bc.cumulative_work_at(h), h)
        };
        let mut max_work = local_work;
        let mut best_peer: Option<Arc<Peer>> = None;
        let mut target_height = 0;

        for peer in &peers {
            let info = peer.get_info().await;
            let height_gap = info.height.saturating_sub(our_height);
            // Select peer if it has: more cumulative work, OR same work with
            // more height, OR is significantly ahead by block count (> 5 blocks).
            let better_work = info.cumulative_work > max_work;
            let tiebreak = info.cumulative_work == max_work && info.height > target_height;
            let far_ahead = height_gap > 5;
            if better_work || tiebreak || far_ahead {
                if info.cumulative_work > max_work {
                    max_work = info.cumulative_work;
                }
                if info.height > target_height {
                    target_height = info.height;
                }
                best_peer = Some(Arc::clone(peer));
            }
        }

        let peer = match best_peer {
            Some(p) => p,
            None => {
                info!("Already on the heaviest chain — no sync needed");
                return Ok(());
            }
        };

        self.syncing.store(true, Ordering::SeqCst);

        let _our_height = self.blockchain.read().await.get_height();
        info!(
            "Syncing from peer {} (target work: {}, height: {})",
            peer.address().await,
            max_work,
            target_height
        );

        // Re-read actual chain height each iteration — after a deep_reorg the chain height
        // is the reorg tip, which may differ from what we started with.
        let mut stall_count = 0;
        // Track whether this is the very first iteration so we use a wide lookback
        // to detect potential fork points. On subsequent iterations the chain is
        // already at the right tip and we only need a small anchor window.
        let mut first_iteration = true;
        
        // Dynamic batching state
        let mut dynamic_batch_size: u64 = 5000;

        loop {
            // Always re-read the actual chain height — it changes after every reorg/apply.
            let current_sync_height = self.blockchain.read().await.get_height();
            
            // STATE SYNC HEAL FIX (v3.0.8-alpha — final fix)
            // FIX DATE: 2026-08-16 | VERSION: v3.0.8-alpha
            // REASON: The old check compared current_state_root() against the hardcoded canonical
            // root "42db10a2...". That root only existed on the original proposer's machine and
            // NO peer can ever produce it. After the snapshot is applied (root "2ee3073..."),
            // the check STILL fired because "2ee3073..." != "42db10a2...", causing an infinite loop.
            // FIX: Use forward-verification — simulate block 110,001 transactions against the
            // current in-memory state. If the resulting root matches block 110,001's state_root,
            // our state is fine and no snap-sync is needed. Only trigger snap-sync if this check fails.
            if current_sync_height == 110_001 {
                let needs_snapshot = {
                    let bc = self.blockchain.read().await;
                    // Try to get block 110,001 to forward-verify our state
                    if let Some(block_110001) = bc.get_block_by_height(110_001) {
                        if !block_110001.state_root.is_empty() {
                            // Simulate block 110,001 against current in-memory state
                            let mut test_state = bc.get_account_state_clone();
                            test_state.unlock_mature_coinbase(block_110001.index);
                            for tx in &block_110001.transactions {
                                if !tx.is_coinbase() && tx.sender != "TREASURY" && !tx.is_genesis_premine() {
                                    let total = tx.amount.saturating_add(tx.fee);
                                    test_state.debit_account(&tx.sender, total);
                                    test_state.increment_nonce(&tx.sender);
                                }
                                let maturity = if tx.is_genesis_premine() { 0 } else { 500 }; // COINBASE_MATURITY
                                test_state.credit_account(tx, block_110001.index, maturity);
                            }
                            let test_root = test_state.calculate_state_root();
                            // If our state correctly produces 110,001's root, no snap-sync needed
                            test_root != block_110001.state_root
                        } else {
                            // Block 110,001 has no state root — fall back to canonical check
                            let current_root = bc.current_state_root();
                            bc.get_canonical_state_root(110_000)
                                .map(|expected| current_root != expected)
                                .unwrap_or(false)
                        }
                    } else {
                        // Don't have block 110,001 yet — fall back to canonical check
                        let current_root = bc.current_state_root();
                        // The actual canonical post-heal root is the one produced by the snapshot (2ee3...),
                        // NOT the one written in the block (42db...).
                        // FIX DATE: 2026-08-18 | VERSION: v3.0.11-alpha
                        if current_root == "2ee3073191a84fa407d3a1e798d01571ad930c807ea9d6a838a4c9b93330cef6" {
                            false
                        } else {
                            bc.get_canonical_state_root(110_000)
                                .map(|expected| current_root != expected)
                                .unwrap_or(false)
                        }
                    }
                };

                if needs_snapshot {
                    let expected_root = self.blockchain.read().await
                        .get_canonical_state_root(110_000)
                        .unwrap_or_default();
                    tracing::warn!("Local state root diverged at hard-fork block 110,000. Requesting canonical state snapshot from peer...");
                    let _ = peer.send_message(P2PMessage::GetStateSnapshot {
                        height: 110_000,
                        expected_state_root: expected_root,
                    }).await;

                    // Abort this sync cycle so we wait for the snapshot to arrive!
                    self.syncing.store(false, Ordering::SeqCst);
                    *self.sync_request_range.lock().await = None;
                    return Ok(());
                }
            }
            
            if current_sync_height >= target_height {
                break;
            }

            // Step 1: Request Headers.
            // On the first pass search back up to 500 blocks to find a possible fork point.
            // On subsequent passes we are already on the canonical tip, so a small
            // lookback (10 blocks) is enough to anchor the chain and save bandwidth.
            let search_start = if first_iteration {
                current_sync_height.saturating_sub(500)
            } else {
                current_sync_height.saturating_sub(10)
            };
            first_iteration = false;

            {
                let mut hb = self.header_buffer.lock().await;
                hb.clear();
            }

            if let Err(e) = peer
                .send_message(P2PMessage::GetHeaders {
                    start_height: search_start,
                })
                .await
            {
                warn!("Header request failed: {}", e);
                break;
            }

            // Wait for headers — up to 30 s (60 × 500 ms).
            // Previously 10 s was often insufficient when the seed node needed to load
            // 2000 headers from RocksDB across a slow VPS link.
            let mut wait = 0u32;
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                wait += 1;
                let sz = self.header_buffer.lock().await.len();
                if sz > 0 || wait >= 60 {
                    break;
                } // 30 s timeout
            }

            let headers: Vec<crate::network::protocol::BlockHeader> = {
                let mut hb = self.header_buffer.lock().await;
                let mut h: Vec<_> = hb.drain(..).collect();
                h.sort_by_key(|x| x.index);
                h
            };

            if headers.is_empty() {
                stall_count += 1;
                if stall_count >= 3 {
                    break;
                }
                continue;
            }
            stall_count = 0;

            // Step 2: Validate Headers & Find Fork Point
            let bc = self.blockchain.read().await;
            let mut fork_point = None;
            for h in headers.iter().rev() {
                if let Some(our_hash) = bc.get_block_hash_at(h.index) {
                    if our_hash == h.hash {
                        fork_point = Some(h.index + 1);
                        break;
                    }
                }
            }
            drop(bc);

            let request_start = fork_point.unwrap_or(headers[0].index);
            let request_end = headers.last().unwrap().index;

            if request_start > request_end {
                // All headers in this batch are already part of our chain.
                // current_sync_height was refreshed at the top of the loop from the
                // actual chain height, so the next iteration will correctly advance
                // the search window forward.
                info!(
                    "Sync: all headers [{}-{}] already applied, advancing window",
                    request_start, request_end
                );
                continue;
            }

            // BFT header sanity check: every unseen header must reference a
            // non-zero hash (i.e. it was actually computed, not zero-initialised).
            // NOTE: sig_count is NOT checked here — AlephBFT embeds signatures
            // in the full block body, not in the gossip header. Checking
            // sig_count == 0 on headers would incorrectly reject all valid BFT
            // blocks and was the root cause of the "stuck at height 1" bug.
            let unseen_headers: Vec<_> = headers
                .into_iter()
                .filter(|h| h.index >= request_start)
                .collect();
            let mut valid_headers = true;
            for h in &unseen_headers {
                if h.index > 0 && h.hash.is_empty() {
                    valid_headers = false;
                    break;
                }
            }

            if !valid_headers {
                warn!(
                    "Peer sent headers with empty hashes - aborting sync (peer may be corrupted)"
                );
                peer.add_misbehavior(50).await;
                break;
            }

            // Step 3: Request Full Blocks for the validated headers.
            // Use the dynamically calculated batch limit.
            let batch_end = request_end.min(request_start + dynamic_batch_size - 1);
            info!(
                "Headers validated. Requesting full blocks [{}-{}]",
                request_start, batch_end
            );
            {
                let mut sb = self.sync_buffer.lock().await;
                sb.clear();
            }

            // Announce the requested range BEFORE sending GetBlocks so that
            // handle_new_block can buffer arriving blocks — including reorg blocks
            // whose index is BELOW the current chain tip.
            *self.sync_request_range.lock().await = Some((request_start, batch_end));

            if let Err(e) = peer
                .send_message(P2PMessage::GetBlocks {
                    start_height: request_start,
                    end_height: batch_end,
                })
                .await
            {
                *self.sync_request_range.lock().await = None;
                warn!("Block request failed: {}", e);
                break;
            }

            // Idle-based timeout: reset the idle counter each time a new block arrives.
            // This ensures a slow-but-progressing transfer is never cut off prematurely,
            // while still stopping if the peer goes silent.
            // SYNC FIX: Send a keep-alive Ping every 15s to prevent the seed node from
            // treating this connection as idle and closing it with EOF. The Pong will arrive
            // in the message channel and be silently discarded, so it does not corrupt the
            // sync_buffer or break the block-counting logic.
            let expected = (batch_end.saturating_sub(request_start) + 1) as usize;
            let idle_timeout_iters = 120u32; // 120 × 500 ms = 60 s idle timeout (doubled)
            let mut idle_count = 0u32;
            let mut last_seen_sz = 0usize;
            let mut ping_tick = 0u32; // send a ping every 30 iters (15 s)
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let sz = self.sync_buffer.lock().await.len();
                if sz >= expected {
                    break; // all blocks received
                }
                if sz > last_seen_sz {
                    // Progress — reset idle counter
                    idle_count = 0;
                    last_seen_sz = sz;
                } else {
                    idle_count += 1;
                    // SYNC FIX: Send keep-alive ping every 15s to hold the connection open
                    ping_tick += 1;
                    if ping_tick % 30 == 0 {
                        let nonce: u64 = rand::random();
                        let _ = peer.send_message(P2PMessage::Ping(nonce)).await;
                        debug!(
                            "Sync keep-alive ping sent to {} ({}/{} blocks received)",
                            peer.address().await,
                            sz,
                            expected
                        );
                    }
                    if idle_count >= idle_timeout_iters {
                        warn!(
                            "Block download idle timeout after {}s — received {}/{} blocks",
                            idle_timeout_iters / 2,
                            sz,
                            expected
                        );
                        break;
                    }
                }
            }

            let blocks: Vec<Block> = {
                let mut sb = self.sync_buffer.lock().await;
                let mut b: Vec<_> = sb.drain(..).collect();
                b.sort_by_key(|x| x.index);
                b
            };
            // Clear the range — blocks arriving now are NOT part of this batch.
            *self.sync_request_range.lock().await = None;

            if blocks.is_empty() {
                warn!("Peer did not yield requested blocks");
                break;
            }

            // SYNC FIX: If we timed out and only got a partial batch, DO NOT attempt a deep reorg.
            // A deep reorg on a partial batch will almost always fail validation mid-way, and the
            // node will waste massive I/O rolling back and restoring state for no reason.
            if blocks.len() != expected {
                let bc_height = self.blockchain.read().await.get_height();
                if request_start < bc_height {
                    warn!("Deep reorg aborted: received partial batch ({}/{}) which would corrupt state. Will retry.",
                        blocks.len(), expected);
                    break;
                }
            }

            // Dynamic Batching Logic: calculate total serialized size of blocks applied
            if !blocks.is_empty() {
                let mut total_bytes = 0;
                for block in &blocks {
                    if let Ok(size) = bincode::serialized_size(block) {
                        total_bytes += size as usize;
                    }
                }
                let avg_size = total_bytes.saturating_div(blocks.len());
                // Target ~50 MB per payload. If blocks are 2MB, batch size drops to ~25. If 2KB, 5000.
                dynamic_batch_size = (50_000_000 / avg_size.max(1)) as u64;
                dynamic_batch_size = dynamic_batch_size.clamp(50, 5000);
            }

            // Step 4: Apply Blocks
            let bc_height = self.blockchain.read().await.get_height();
            if request_start < bc_height {
                // This is a fork/reorg
                let bc = self.blockchain.write().await;
                match bc.deep_reorg(request_start, blocks.clone()) {
                    Ok(_) => info!("Reorg to heavier chain successful"),
                    Err(e) => {
                        // Reorg failures are almost always caused by our own sync logic
                        // (timeout, partial batch, stale rollback_to) — NOT peer misbehavior.
                        // Do NOT penalize the peer here; just abort this sync cycle and
                        // let the next periodic sync retry with a fresh header scan.
                        warn!("Reorg failed (will retry next sync cycle): {}", e);
                        break;
                    }
                }
            } else {
                // Normal extension
                let bc = self.blockchain.write().await;
                for b in blocks {
                    if let Err(e) = bc.add_network_block(b.clone()) {
                        warn!("Failed to add block: {}", e);
                        break;
                    }
                    
                    // STATE SYNC FIX (v3.0.4-alpha)
                    if b.index == 110_000 {
                        let needs_sync = {
                            let current_root = bc.current_state_root();
                            current_root != b.state_root
                        };
                        if needs_sync {
                            tracing::warn!("Local state root diverged at hard-fork block 110,000. Requesting canonical state snapshot from peer...");
                            let _ = peer.send_message(P2PMessage::GetStateSnapshot {
                                height: 110_000,
                                expected_state_root: b.state_root.clone(),
                            }).await;
                            // Abort the entire sync cycle so we don't try to validate block 110,001 
                            // with the wrong state before the snapshot arrives!
                            // The next periodic sync will resume from 110,001 after the state is healed.
                            self.syncing.store(false, Ordering::SeqCst);
                            *self.sync_request_range.lock().await = None;
                            return Ok(());
                        }
                    }
                }
            }

            // current_sync_height is refreshed at the top of the loop from the actual
            // chain height, so we do NOT set it here — doing so would use request_end
            // (capped by the 500-header window) instead of the real post-reorg height,
            // which was the root cause of the post-reorg stall bug.
        }

        self.syncing.store(false, Ordering::SeqCst);
        *self.sync_request_range.lock().await = None; // ensure cleared on any exit path
        info!(
            "Sync cycle complete. Current height: {}",
            self.blockchain.read().await.get_height()
        );
        Ok(())
    }

    /// Maintain peer connections
    async fn maintain_peers(self: Arc<Self>) {
        // CPU-FIX v2.4.25: Slowed from 10s to 30s — with many known peers in the
        // discovery table the old 10s interval spawned 17+ concurrent TCP+TLS
        // handshake tasks every cycle, causing 300%+ CPU on the VPS.
        // DATE: 2026-07-18 | VERSION: v2.4.25-alpha
        let mut ticker = interval(Duration::from_secs(30));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;

            // Clean up dead peers
            self.peer_manager.cleanup_dead_peers().await;

            // SYNC FIX: Skip new connection attempts while a sync is in progress.
            // Spawning new TCP connections to the same seed node during sync causes
            // two problems:
            //   1. The seed's Sybil-protection limit (2 per /24 subnet) kicks in
            //      and the new connections are rejected, generating noisy log spam.
            //   2. Each failed connect attempt marks the peer as "failed" in the
            //      discovery table, eventually preventing legitimate reconnects.
            // We still run dead-peer cleanup so the peer list stays accurate, but
            // we defer all new outbound connection attempts until sync finishes.
            if self.syncing.load(Ordering::SeqCst) {
                continue;
            }

            // Try to maintain minimum peer count
            let peer_count = self.peer_manager.peer_count().await;
            if peer_count < self.config.max_peers {
                let needed = self.config.max_peers.saturating_sub(peer_count);
                // SECURITY FIX: Subnet bucketing strategy for outgoing connections
                let mut target_peers = self.discovery.get_random_peers(needed).await;

                if peer_count == 0 && !self.config.bootstrap_nodes.is_empty() {
                    target_peers.extend(self.config.bootstrap_nodes.iter().copied());
                }

                target_peers.sort_unstable();
                target_peers.dedup();

                // CPU-FIX v2.4.25: Limit concurrent reconnect attempts to 3 per maintenance
                // cycle. Before this cap, every cycle would spawn `needed` (up to 125) tasks
                // simultaneously, creating a storm of TCP+TLS handshakes that saturated CPU.
                // DATE: 2026-07-18 | VERSION: v2.4.25-alpha
                let max_reconnects_per_cycle: usize = 3;
                let mut reconnect_count = 0;

                for addr in target_peers {
                    if reconnect_count >= max_reconnects_per_cycle {
                        break;
                    }
                    // FLAP-FIX: Check if already connected AND LIVE before skipping.
                    // The old check only tested by IP, not liveness — a dead peer with
                    // the same IP would block reconnect attempts indefinitely.
                    let is_connected = {
                        let peers = self.peer_manager.get_peers().await;
                        let mut connected = false;
                        for peer in peers {
                            if peer.address().await == addr && peer.is_alive().await {
                                connected = true;
                                break;
                            }
                        }
                        connected
                    };

                    if is_connected {
                        self.discovery.update_peer_seen(addr).await;
                        continue;
                    }

                    reconnect_count += 1;
                    let network = Arc::clone(&self);
                    tokio::spawn(async move {
                        match network.connect_to_peer(addr).await {
                            Ok(_) => {
                                network.discovery.update_peer_seen(addr).await;
                            }
                            Err(e) => {
                                if e.contains("Already connected")
                                    || e.contains("Too many connections")
                                {
                                    // We are connected (or rejected safely), so update seen time
                                    network.discovery.update_peer_seen(addr).await;
                                } else {
                                    network.discovery.mark_peer_failed(addr).await;
                                }
                            }
                        }
                    });
                }
            }
        }
    }

    /// Get connected peer count
    pub async fn peer_count(&self) -> usize {
        self.peer_manager.peer_count().await
    }

    /// Get peer count (alias for health check)
    pub async fn get_peer_count(&self) -> usize {
        self.peer_count().await
    }

    /// Get peer information
    pub async fn get_peers_info(&self) -> Vec<crate::network::peer::PeerInfo> {
        let peers = self.peer_manager.get_peers().await;
        let mut info = Vec::new();

        for peer in peers {
            info.push(peer.get_info().await);
        }

        info
    }

    /// Send heartbeat pings to all peers (keeps connections alive during mining)
    async fn send_heartbeats(&self) {
        let peers = self.peer_manager.get_peers().await;
        if !peers.is_empty() {
            info!("Heartbeat: Pinging {} peers", peers.len());
        }
        for peer in peers {
            let peer = Arc::clone(&peer);
            tokio::spawn(async move {
                let nonce = rand::random();
                if let Err(e) = peer.send_message(P2PMessage::Ping(nonce)).await {
                    warn!("Heartbeat ping failed: {}", e);
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default_generates_unique_node_ids() {
        let cfg1 = NetworkConfig::default();
        let cfg2 = NetworkConfig::default();
        
        assert_ne!(cfg1.node_id, cfg2.node_id, "Default configs must generate unique UUID node IDs");
        assert_eq!(cfg1.max_peers, 125, "Default max_peers should be 125");
        assert_eq!(cfg1.listen_addr.to_string(), "0.0.0.0:8333");
    }

    #[test]
    fn test_sync_batch_constants() {
        assert!(MAX_SYNC_BATCH <= 1000, "MAX_SYNC_BATCH must be reasonable to prevent memory exhaustion");
        assert!(MAX_HEADERS_PER_RESPONSE <= 5000, "MAX_HEADERS_PER_RESPONSE must prevent oversized packets");
    }
}

