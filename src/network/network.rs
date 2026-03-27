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
                    info!("Incoming connection from {}", addr);
                    
                    let message_tx = self.message_tx.clone();
                    let peer_manager = Arc::clone(&self.peer_manager);
                    let blockchain = Arc::clone(&self.blockchain);
                    let node_id = self.config.node_id.clone();
                    
                    tokio::spawn(async move {
                        match Peer::new(stream, addr).await {
                            Ok(peer) => {
                                let peer = Arc::new(peer);
                                
                                // Perform handshake
                                let height = blockchain.read().await.get_chain().len() as u64;
                                if let Ok(_) = peer.handshake(PROTOCOL_VERSION, height, node_id).await {
                                    // Add peer and start receive task
                                    if peer_manager.add_peer(Arc::clone(&peer)).await.is_ok() {
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
        let height = blockchain.get_chain().len() as u64;
        drop(blockchain);
        
        peer.handshake(PROTOCOL_VERSION, height, self.config.node_id.clone()).await?;
        
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
            let mut peer = None;
            for p in self.peer_manager.get_peers().await {
                if p.address().await == addr {
                    peer = Some(p);
                    break;
                }
            }

            let network = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = network.handle_message(addr, msg, peer).await {
                    error!("Error handling message from {}: {}", addr, e);
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
                self.handle_get_blocks(addr, start_height, end_height).await?;
            }
            P2PMessage::GetHeight => {
                self.handle_get_height(addr).await?;
            }
            P2PMessage::Height(height) => {
                debug!("Peer {} has height {}", addr, height);
                if let Some(p) = &peer {
                    p.update_height(height).await;
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
        let our_height = blockchain.get_height();
        drop(blockchain);

        if block.index > latest.index + 100 {
            // We just ignore it. The periodic sync loop in main.rs will
            // detect the height gap and execute a proper sync_blockchain()
            // batch process. We do not want to spam GetBlocks here.
            return Ok(());
        }

        // SYNC FIX: If syncing and block is in the sync range, buffer it
        // for ordered sequential application.
        // We do this BEFORE the seen_blocks check, because previously-failed blocks
        // might be in the seen cache and we need to process them during a sync!
        if is_syncing && block.index > latest.index && block.index <= latest.index + 1500 {
            let mut buffer = self.sync_buffer.lock().await;
            if buffer.len() < (MAX_SYNC_BATCH as usize + 200) {
                buffer.push(block);
            }
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
                info!("Block {} accepted at height {} — re-broadcasting to peers",
                    &block.hash[..8], block.index);
                drop(bc);
                // BETA FIX: Re-broadcast so nodes NOT directly connected to the miner
                // also receive the block (essential for mesh topology with 6+ nodes).
                self.broadcast_block(block.clone()).await;

                // SYNC FIX: Don't do recursive sync requests here.
                // The main sync_blockchain loop handles this properly.
                // Only request more blocks if NOT in a sync operation.
                if !is_syncing {
                    if let Some(p) = peer {
                        let peer_info = p.get_info().await;
                        if peer_info.height > block.index + 1 {
                            let end_height = peer_info.height.min(block.index + MAX_SYNC_BATCH);
                            info!("Requesting next blocks: {} → {}", block.index + 1, end_height);
                            let _ = p.send_message(P2PMessage::GetBlocks {
                                start_height: block.index + 1,
                                end_height,
                            }).await;
                        }
                    }
                }

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

    /// Handle get blocks request — serve from storage, cap batch to 500 blocks
    async fn handle_get_blocks(&self, addr: SocketAddr, start: u64, end: u64) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        // HIGH-2 FIX: Clamp batch to MAX_SYNC_BATCH regardless of what peer claims
        let end = end.min(start + MAX_SYNC_BATCH - 1);
        let blocks: Vec<Block> = (start..=end)
            .filter_map(|i| blockchain.load_block_from_storage(i))
            .collect();
        drop(blockchain);
        
        info!("Serving {} blocks [{}-{}] to peer {}", blocks.len(), start, end, addr);
        for block in blocks {
            self.send_to_peer(addr, P2PMessage::Block(block)).await?;
        }
        
        Ok(())
    }

    /// Handle get height request — BETA FIX: use storage height, not in-memory chain length
    async fn handle_get_height(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        // get_height() reads from storage — correct even after thousands of blocks
        let height = blockchain.get_height();
        
        self.send_to_peer(addr, P2PMessage::Height(height)).await
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

    /// Broadcast block to all peers
    pub async fn broadcast_block(&self, block: Block) {
        self.peer_manager.broadcast(P2PMessage::Block(block)).await;
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
    /// New approach:
    /// - Set syncing=true to suppress broadcast-triggered sync requests
    /// - Clear sync buffer, send GetBlocks, wait for blocks to arrive in buffer
    /// - Sort buffered blocks by index and apply sequentially
    /// - Repeat until caught up
    pub async fn sync_blockchain(&self) -> Result<(), String> {
        let peers = self.peer_manager.get_peers().await;
        
        if peers.is_empty() {
            return Ok(());
        }
        
        info!("Starting blockchain synchronization");
        
        // Ask all peers for their current height
        for peer in &peers {
            let _ = peer.send_message(P2PMessage::GetHeight).await;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Find peer with highest height
        let mut max_height = self.blockchain.read().await.get_height();
        let mut best_peer: Option<Arc<Peer>> = None;
        
        for peer in &peers {
            let info = peer.get_info().await;
            if info.height > max_height {
                max_height = info.height;
                best_peer = Some(Arc::clone(peer));
            }
        }
        
        let peer = match best_peer {
            Some(p) => p,
            None => {
                info!("Already at chain tip — no sync needed");
                return Ok(());
            }
        };
        
        info!("Syncing from peer {} at height {} (we are at {})", 
            peer.address().await, max_height, self.blockchain.read().await.get_height());
        
        // SYNC FIX: Set syncing flag to suppress broadcast-triggered GetBlocks requests
        self.syncing.store(true, Ordering::SeqCst);
        
        let mut stall_count = 0u32;
        const MAX_STALLS: u32 = 5; // Give up after 5 consecutive stalls
        
        // Batch loop: keep requesting MAX_SYNC_BATCH blocks until caught up.
        loop {
            let our_height = self.blockchain.read().await.get_height();
            if our_height >= max_height {
                break;
            }

            // Clear the sync buffer before requesting a new batch
            {
                let mut buffer = self.sync_buffer.lock().await;
                buffer.clear();
            }

            let end_height = max_height.min(our_height + MAX_SYNC_BATCH);
            info!("Sync batch: requesting blocks [{}-{}] (target: {})", our_height, end_height, max_height);

            if let Err(e) = peer.send_message(P2PMessage::GetBlocks {
                start_height: our_height,
                end_height,
            }).await {
                warn!("Sync request failed: {} — aborting sync", e);
                break;
            }

            // SYNC FIX: Wait for blocks to arrive in the sync buffer.
            // Poll the buffer until no new blocks have arrived for a timeout period.
            let expected_blocks = (end_height - our_height) as usize;
            let mut wait_cycles = 0u32;
            let max_wait_cycles = 30; // 30 × 500ms = 15 seconds max wait per batch
            let mut last_buffer_size = 0usize;
            let mut no_progress_count = 0u32;
            
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                wait_cycles += 1;
                
                let buffer_size = self.sync_buffer.lock().await.len();
                
                if buffer_size >= expected_blocks {
                    info!("Sync buffer full: {}/{} blocks received", buffer_size, expected_blocks);
                    break;
                }
                
                if buffer_size == last_buffer_size {
                    no_progress_count += 1;
                } else {
                    no_progress_count = 0;
                    last_buffer_size = buffer_size;
                }
                
                // If no new blocks for 3 seconds or max wait reached, process what we have
                if no_progress_count >= 6 || wait_cycles >= max_wait_cycles {
                    if buffer_size > 0 {
                        info!("Sync buffer partial: {}/{} blocks after {}ms", 
                            buffer_size, expected_blocks, wait_cycles * 500);
                    } else {
                        warn!("No sync blocks received after {}ms", wait_cycles * 500);
                    }
                    break;
                }
            }

            // SYNC FIX: Extract blocks from buffer, sort by index, and apply sequentially
            let blocks_to_apply: Vec<Block> = {
                let mut buffer = self.sync_buffer.lock().await;
                let mut blocks: Vec<Block> = buffer.drain(..).collect();
                blocks.sort_by_key(|b| b.index);
                blocks
            };

            if blocks_to_apply.is_empty() {
                stall_count += 1;
                warn!("Sync stall {}/{}: no blocks received, retrying...", stall_count, MAX_STALLS);
                if stall_count >= MAX_STALLS {
                    warn!("Sync stalled {} times — aborting. Will retry on next peer cycle.", MAX_STALLS);
                    break;
                }
                // Wait a bit before retrying (connection might need to reconnect)
                tokio::time::sleep(Duration::from_secs(3)).await;
                continue;
            }
            stall_count = 0; // Reset on progress

            let batch_count = blocks_to_apply.len();
            let first_idx = blocks_to_apply.first().map(|b| b.index).unwrap_or(0);
            let last_idx = blocks_to_apply.last().map(|b| b.index).unwrap_or(0);
            info!("Applying {} sync blocks [{}-{}] sequentially...", batch_count, first_idx, last_idx);

            let mut applied = 0u64;
            let mut errors = 0u64;
            for block in blocks_to_apply {
                let bc = self.blockchain.write().await;
                match bc.add_network_block(block.clone()) {
                    Ok(_) => {
                        applied += 1;
                        if applied % 100 == 0 {
                            info!("Sync progress: applied {}/{} blocks (height: {})", 
                                applied, batch_count, block.index + 1);
                        }
                    }
                    Err(e) => {
                        errors += 1;
                        if errors <= 5 {
                            warn!("Sync: failed to apply block {}: {}", block.index, e);
                        }
                        // Don't break — later blocks might still apply (orphan processing)
                    }
                }
                drop(bc);
            }

            let new_height = self.blockchain.read().await.get_height();
            info!("Sync batch complete: applied {}, errors {}, new height: {}", 
                applied, errors, new_height);

            // If no blocks were applied at all, we're stuck
            if applied == 0 {
                stall_count += 1;
                warn!("Sync stall {}/{}: received blocks but none applied", stall_count, MAX_STALLS);
                if stall_count >= MAX_STALLS {
                    warn!("Sync permanently stalled — aborting.");
                    break;
                }
            }

            // Refresh peer tip in case it grew while we were syncing
            let _ = peer.send_message(P2PMessage::GetHeight).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            let info = peer.get_info().await;
            if info.height > max_height {
                max_height = info.height;
            }
        }
        
        // SYNC FIX: Clear syncing state
        self.syncing.store(false, Ordering::SeqCst);
        
        let final_height = self.blockchain.read().await.get_height();
        info!("Blockchain sync complete — height: {}", final_height);
        
        // If we're still behind, schedule another sync attempt
        if final_height < max_height {
            info!("Still behind (at {} vs target {}), will continue syncing on next cycle", 
                final_height, max_height);
        }
        
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
            
            // Send ping to all peers
            let peers = self.peer_manager.get_peers().await;
            for peer in peers {
                let nonce = rand::random();
                let _ = peer.send_message(P2PMessage::Ping(nonce)).await;
            }
            
            // Try to maintain minimum peer count
            let peer_count = self.peer_manager.peer_count().await;
            if peer_count < self.config.max_peers {
                let needed = self.config.max_peers.saturating_sub(peer_count);
                // SECURITY FIX: Subnet bucketing strategy for outgoing connections
                let mut target_peers = self.discovery.get_random_peers(needed).await;
                
                if target_peers.is_empty() && !self.config.bootstrap_nodes.is_empty() {
                    target_peers.extend(self.config.bootstrap_nodes.iter().copied()); // Attempt bootstrap nodes if discovery empty
                }

                for addr in target_peers {
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
