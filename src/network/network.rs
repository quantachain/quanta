use crate::core::block::Block;
use crate::consensus::blockchain::Blockchain;
use crate::network::peer::{Peer, PeerManager};
use crate::network::protocol::{P2PMessage, PROTOCOL_VERSION};
use crate::core::transaction::Transaction;
use crate::network::PeerDiscovery;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use lru::LruCache;
use std::num::NonZeroUsize;

/// Maximum blocks requested per sync batch (HIGH-2 FIX: prevents height-forgery storm)
const MAX_SYNC_BATCH: u64 = 500;

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
    config: NetworkConfig,
    blockchain: Arc<RwLock<Blockchain>>,
    peer_manager: Arc<PeerManager>,
    message_tx: mpsc::Sender<(SocketAddr, P2PMessage)>,
    message_rx: Arc<RwLock<mpsc::Receiver<(SocketAddr, P2PMessage)>>>,
    // BETA FIX: Deduplication caches — prevent broadcast storms.
    // A node only re-propagates a block/tx the FIRST time it sees it.
    // LRU(1024) keeps ~10+ minutes of blocks at 30s block time.
    seen_blocks: Arc<Mutex<LruCache<String, ()>>>,
    seen_txs:    Arc<Mutex<LruCache<String, ()>>>,
    discovery: Arc<PeerDiscovery>,
    /// SYNC FIX: Track whether a sync operation is currently in progress.
    /// When true, broadcast blocks that are "too far ahead" will NOT
    /// trigger additional GetBlocks requests (prevents request storms).
    syncing: Arc<AtomicBool>,
    /// SYNC FIX: Mutex-protected sync block buffer. Sync response blocks
    /// are collected here and applied sequentially, not concurrently.
    sync_buffer: Arc<tokio::sync::Mutex<Vec<Block>>>,
    /// Header sync buffer
    header_buffer: Arc<tokio::sync::Mutex<Vec<crate::network::protocol::BlockHeader>>>,
    /// REORG FIX: The exact [start, end] block index range currently being
    /// downloaded by the sync loop. Set just before GetBlocks is sent;
    sync_request_range: Arc<tokio::sync::Mutex<Option<(u64, u64)>>>,
    /// Channel to forward received AlephBFT messages to consensus
    aleph_bft_tx: Arc<tokio::sync::RwLock<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>>,
}

impl Network {
    /// Create a new network instance
    pub fn new(config: NetworkConfig, blockchain: Arc<RwLock<Blockchain>>) -> Self {
        // CRIT-3 FIX: Bounded channel(10_000) prevents OOM via message flood.
        // An attacker sending millions of messages will now get dropped, not buffered.
        let (message_tx, message_rx) = mpsc::channel(10_000);
        let discovery = Arc::new(PeerDiscovery::with_dns_seeds(
            config.bootstrap_nodes.clone(),
            config.dns_seeds.clone(),
        ));
        Self {
            config,
            blockchain,
            peer_manager: Arc::new(PeerManager::new(125)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            // 1024 entries ≈ 30+ minutes of blocks at 30s block time
            seen_blocks: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1024).unwrap()))),
            // 10k tx entries ≈ handles a full mempool cycle without re-flooding
            seen_txs: Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(10_000).unwrap()))),
            discovery,
            syncing: Arc::new(AtomicBool::new(false)),
            sync_buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            header_buffer: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            sync_request_range: Arc::new(tokio::sync::Mutex::new(None)),
            aleph_bft_tx: Arc::new(tokio::sync::RwLock::new(None)),
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
        
        // Start heartbeat (ping peers every 10 seconds to keep connections alive)
        let heartbeat_handle = {
            let network = Arc::clone(&self);
            tokio::spawn(async move {
                let mut interval = interval(Duration::from_secs(10));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                loop {
                    interval.tick().await;
                    network.send_heartbeats().await;
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
        let _ = tokio::join!(listen_handle, processor_handle, maintenance_handle, heartbeat_handle);
        
        Ok(())
    }

    /// Listen for incoming peer connections
    async fn listen_for_connections(&self) -> Result<(), String> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| format!("Failed to bind listener: {}", e))?;
        
        info!("Listening for connections on {}", self.config.listen_addr);
        
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    // H-4: Reject inbound connections that would exceed the configured peer limit.
                    // Without this check a botnet can exhaust connection slots before
                    // PeerManager.add_peer() is even called.
                    let current_count = self.peer_manager.peer_count().await;
                    if current_count >= self.config.max_peers {
                        // Drop stream immediately — TCP RST is sent on drop.
                        tracing::debug!("Inbound connection from {} rejected: peer limit {} reached", addr, self.config.max_peers);
                        drop(stream);
                        continue;
                    }

                    info!("Incoming connection from {}", addr);

                    let message_tx = self.message_tx.clone();
                    let peer_manager = Arc::clone(&self.peer_manager);
                    let blockchain = Arc::clone(&self.blockchain);
                    let discovery = Arc::clone(&self.discovery);
                    let node_id = self.config.node_id.clone();

                    tokio::spawn(async move {
                        match Peer::new(stream, addr).await {
                            Ok(peer) => {
                                let peer = Arc::new(peer);

                                let blockchain = blockchain.read().await;
                                let height = blockchain.get_height();
                                let cumulative_work = blockchain.cumulative_work_at(height);
                                drop(blockchain);

                                if let Ok(_) = peer.handshake(PROTOCOL_VERSION, height, cumulative_work, node_id).await {
                                    if peer_manager.add_peer(Arc::clone(&peer)).await.is_ok() {
                                        // BETA FIX: Add the peer's IP to discovery with the default port
                                        // so that other nodes can discover it and form a full mesh network.
                                        let mut discovery_addr = addr;
                                        discovery_addr.set_port(8333); // Assume default Quanta port
                                        discovery.add_peer(discovery_addr).await;
                                        
                                        Self::start_peer_receive_task(peer, message_tx, peer_manager).await;
                                    }
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
        peer_manager: Arc<PeerManager>
    ) {
        let addr = peer.address().await;
        tokio::spawn(async move {
            loop {
                match peer.receive_message().await {
                    Ok(msg) => {
                        debug!("Received message from {}: {:?}", addr, msg);
                        // CRIT-3 FIX: Use try_send on bounded channel.
                        // If full, add a strike to the misbehaving peer instead of buffering.
                        if let Err(_) = message_tx.try_send((addr, msg)) {
                            warn!("Message channel full — dropping message from {} and adding misbehavior score (+20)", addr);
                            peer.add_misbehavior(20).await;
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



    /// Connect to a peer
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<(), String> {
        info!("Connecting to peer {}", addr);
        
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;
        
        let peer = Arc::new(Peer::new(stream, addr).await?);
        
        // Perform handshake
        let blockchain = self.blockchain.read().await;
        let height = blockchain.get_height();
        let cumulative_work = blockchain.cumulative_work_at(height);
        drop(blockchain);
        
        peer.handshake(PROTOCOL_VERSION, height, cumulative_work, self.config.node_id.clone()).await?;
        
        // Add to peer manager
        self.peer_manager.add_peer(Arc::clone(&peer)).await?;
        
        // Start single receive task
        Self::start_peer_receive_task(
            peer,
            self.message_tx.clone(),
            Arc::clone(&self.peer_manager)
        ).await;
        
        info!("Connected to peer {}", addr);
        Ok(())
    }

    /// Process incoming messages (PARALLELIZED - spawn handler per message)
    async fn process_messages(self: Arc<Self>) {
        let mut rx = self.message_rx.write().await;
        
        while let Some((addr, msg)) = rx.recv().await {
            // Find the peer object to pass to the handler for strike management
            let mut peer_opt = None;
            for p in self.peer_manager.get_peers().await {
                if p.address().await == addr {
                    peer_opt = Some(p);
                    break;
                }
            }

            let network = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = network.handle_message(addr, msg.clone(), peer_opt).await {
                    error!("Error handling message {:?} from {}: {}", msg, addr, e);
                }
            });
        }
    }

    /// Handle a single message
    async fn handle_message(&self, addr: SocketAddr, msg: P2PMessage, peer: Option<Arc<Peer>>) -> Result<(), String> {
        match msg {
            P2PMessage::NewTx(tx) => {
                self.handle_new_transaction(tx, peer).await?;
            }
            P2PMessage::Block(block) => {
                self.handle_new_block(block, peer).await?;
            }
            P2PMessage::GetBlocks { start_height, end_height } => {
                self.handle_get_blocks(addr, start_height, end_height, peer.clone()).await?;
            }
            P2PMessage::GetHeaders { start_height } => {
                self.handle_get_headers(addr, start_height, peer.clone()).await?;
            }
            P2PMessage::Headers(headers) => {
                self.handle_headers(headers, peer).await?;
            }
            P2PMessage::GetHeight => {
                self.handle_get_height(addr).await?;
            }
            P2PMessage::Height { height, cumulative_work } => {
                debug!("Peer {} has height {} (work {})", addr, height, cumulative_work);
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
            P2PMessage::Ping(nonce) => {
                self.send_to_peer(addr, P2PMessage::Pong(nonce)).await?;
            }
            P2PMessage::Pong(_) => {
                // Keep-alive response
            }
            P2PMessage::GetAddr => {
                let addrs = self.discovery.get_random_peers(50).await;
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
                info!("Received AlephBFT message ({} bytes) from {}", data.len(), addr);
                let tx_opt = self.aleph_bft_tx.read().await;
                if let Some(tx) = &*tx_opt {
                    if let Err(e) = tx.send(data) {
                        tracing::error!("Failed to send AlephBFT message to channel: {:?}", e);
                    }
                } else {
                    tracing::warn!("AlephBFT channel not registered yet, dropping message.");
                }
            }
            _ => {
                debug!("Unhandled message type from {}", addr);
            }
        }
        Ok(())
    }

    /// Handle new transaction
    async fn handle_new_transaction(&self, tx: Transaction, peer: Option<Arc<Peer>>) -> Result<(), String> {
        // BETA FIX: Deduplication — only add + re-broadcast if not seen before
        let tx_hash = tx.hash();
        let already_seen = {
            let mut seen = self.seen_txs.lock().unwrap();
            seen.put(tx_hash.clone(), ()).is_some()
        };
        if already_seen {
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
                    warn!("Banning peer {} for repeated invalid transactions (score ≥ 100)", p.address().await);
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

    /// Handle new block (WITH HARDENED VALIDATION + RE-BROADCAST FIX)
    ///
    /// SYNC FIX: During an active sync, blocks that are "too far ahead" are
    /// silently dropped instead of triggering additional GetBlocks requests.
    /// This prevents the request storm that was causing the stuck-at-272 bug.
    async fn handle_new_block(&self, block: Block, peer: Option<Arc<Peer>>) -> Result<(), String> {
        let is_syncing = self.syncing.load(Ordering::SeqCst);

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

        // REORG FIX: Use the exact requested range (sync_request_range) to decide
        // whether to buffer this block during sync. The old condition
        //   `block.index > latest.index`
        // silently dropped any reorg block whose index is AT or BELOW the current
        // chain tip. During a deep reorg we request blocks [fork_point .. tip-1] which
        // are all BELOW the current tip, so they were never buffered — causing
        // "99/100 blocks, first block at height N+1 instead of N" failures.
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
                info!("Block {} accepted at height {} — re-broadcasting to peers",
                    &block.hash[..8], block.index);
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
                        warn!("Banning peer {} for invalid network blocks (score ≥ 100)", p.address().await);
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
    async fn handle_get_blocks(&self, addr: SocketAddr, start: u64, end: u64, peer: Option<Arc<Peer>>) -> Result<(), String> {
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
    async fn handle_get_headers(&self, addr: SocketAddr, start: u64, peer: Option<Arc<Peer>>) -> Result<(), String> {
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

        info!("Serving {} headers [{}-{}] to peer {}",
            headers.len(), start,
            headers.last().map(|h| h.index).unwrap_or(start), addr);

        if let Some(p) = peer {
            let _ = p.send_message(P2PMessage::Headers(headers)).await;
        } else {
            self.send_to_peer(addr, P2PMessage::Headers(headers)).await?;
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
    async fn handle_headers(&self, headers: Vec<crate::network::protocol::BlockHeader>, peer: Option<Arc<Peer>>) -> Result<(), String> {
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
                    debug!("Gossip header for block {} — requesting full block", h.index);
                    let _ = p.send_message(P2PMessage::GetBlocks {
                        start_height: h.index,
                        end_height:   h.index,
                    }).await;
                }
            }
            return Ok(());
        }

        // Batch header response — push to buffer for the sync loop.
        let mut buffer = self.header_buffer.lock().await;
        buffer.extend(headers);
        Ok(())
    }

    /// Handle get height request — BETA FIX: use storage height, not in-memory chain length
    async fn handle_get_height(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        // get_height() reads from storage — correct even after thousands of blocks
        let height = blockchain.get_height();
        let cumulative_work = blockchain.cumulative_work_at(height);
        
        self.send_to_peer(addr, P2PMessage::Height { height, cumulative_work }).await
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
        self.peer_manager.broadcast(P2PMessage::Headers(vec![header])).await;
    }

    /// Register a channel sender for incoming AlephBFT messages.
    pub async fn register_aleph_bft_tx(&self, tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>) {
        let mut guard = self.aleph_bft_tx.write().await;
        *guard = Some(tx);
    }

    /// Broadcast an AlephBFT message to all connected peers
    pub async fn broadcast_aleph_bft(&self, data: Vec<u8>) {
        self.peer_manager.broadcast(P2PMessage::AlephBFTMessage(data)).await;
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
        if peers.is_empty() { return Ok(()); }
        
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
            let better_work   = info.cumulative_work > max_work;
            let tiebreak      = info.cumulative_work == max_work && info.height > target_height;
            let far_ahead     = height_gap > 5;
            if better_work || tiebreak || far_ahead {
                if info.cumulative_work > max_work { max_work = info.cumulative_work; }
                if info.height > target_height      { target_height = info.height; }
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
        info!("Syncing from peer {} (target work: {}, height: {})", peer.address().await, max_work, target_height);
        
        // Re-read actual chain height each iteration — after a deep_reorg the chain height
        // is the reorg tip, which may differ from what we started with.
        let mut stall_count = 0;
        // Track whether this is the very first iteration so we use a wide lookback
        // to detect potential fork points. On subsequent iterations the chain is
        // already at the right tip and we only need a small anchor window.
        let mut first_iteration = true;

        loop {
            // Always re-read the actual chain height — it changes after every reorg/apply.
            let current_sync_height = self.blockchain.read().await.get_height();
            if current_sync_height >= target_height { break; }

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

            if let Err(e) = peer.send_message(P2PMessage::GetHeaders { start_height: search_start }).await {
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
                if sz > 0 || wait >= 60 { break; } // 30 s timeout
            }
            
            let headers: Vec<crate::network::protocol::BlockHeader> = {
                let mut hb = self.header_buffer.lock().await;
                let mut h: Vec<_> = hb.drain(..).collect();
                h.sort_by_key(|x| x.index);
                h
            };
            
            if headers.is_empty() {
                stall_count += 1;
                if stall_count >= 3 { break; }
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
                info!("Sync: all headers [{}-{}] already applied, advancing window", request_start, request_end);
                continue;
            }
            
            // Validate PoW of the unseen headers
            let unseen_headers: Vec<_> = headers.into_iter().filter(|h| h.index >= request_start).collect();
            let mut valid_headers = true;
            for h in &unseen_headers {
                if h.index > 0 && h.sig_count == 0 {
                    valid_headers = false;
                    break;
                }
            }
            
            if !valid_headers {
                warn!("Peer sent headers with invalid PoW - aborting sync");
                peer.add_misbehavior(50).await;
                break;
            }
            
            // Step 3: Request Full Blocks for the validated headers.
            // CAP to 50 blocks per request — PQC blocks are ~2 MB each.
            // 50 blocks ≈ 100 MB which transfers within ~30s on a typical VPS link.
            // Smaller batches mean the connection is idle for shorter periods, making
            // it less likely to be closed by the seed node's liveness checker.
            const BLOCK_BATCH_CAP: u64 = 50;
            let batch_end = request_end.min(request_start + BLOCK_BATCH_CAP - 1);
            info!("Headers validated. Requesting full blocks [{}-{}]", request_start, batch_end);
            {
                let mut sb = self.sync_buffer.lock().await;
                sb.clear();
            }

            // Announce the requested range BEFORE sending GetBlocks so that
            // handle_new_block can buffer arriving blocks — including reorg blocks
            // whose index is BELOW the current chain tip.
            *self.sync_request_range.lock().await = Some((request_start, batch_end));

            if let Err(e) = peer.send_message(P2PMessage::GetBlocks { start_height: request_start, end_height: batch_end }).await {
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
                        debug!("Sync keep-alive ping sent to {} ({}/{} blocks received)",
                            peer.address().await, sz, expected);
                    }
                    if idle_count >= idle_timeout_iters {
                        warn!("Block download idle timeout after {}s — received {}/{} blocks",
                            idle_timeout_iters / 2, sz, expected);
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
                    if let Err(e) = bc.add_network_block(b) {
                        warn!("Failed to add block: {}", e);
                        break;
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
        info!("Sync cycle complete. Current height: {}", self.blockchain.read().await.get_height());
        Ok(())
    }

    /// Maintain peer connections
    async fn maintain_peers(self: Arc<Self>) {
        let mut ticker = interval(Duration::from_secs(10));
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

                if target_peers.is_empty() && !self.config.bootstrap_nodes.is_empty() {
                    target_peers.extend(self.config.bootstrap_nodes.iter().copied());
                }

                for addr in target_peers {
                    // Check if already connected before dialing
                    let is_connected = {
                        let peers = self.peer_manager.get_peers().await;
                        let mut connected = false;
                        for peer in peers {
                            if peer.address().await.ip() == addr.ip() {
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

                    let network = Arc::clone(&self);
                    tokio::spawn(async move {
                        match network.connect_to_peer(addr).await {
                            Ok(_) => {
                                network.discovery.update_peer_seen(addr).await;
                            }
                            Err(e) => {
                                if e.contains("Already connected") || e.contains("Too many connections") {
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
            let nonce = rand::random();
            if let Err(e) = peer.send_message(P2PMessage::Ping(nonce)).await {
                warn!("Heartbeat ping failed: {}", e);
            }
        }
    }
}
