use crate::block::Block;
use crate::blockchain::Blockchain;
use crate::p2p::peer::{Peer, PeerManager};
use crate::p2p::protocol::{P2PMessage, PROTOCOL_VERSION};
use crate::transaction::Transaction;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Network configuration
#[derive(Clone, Debug)]
pub struct NetworkConfig {
    pub listen_addr: SocketAddr,
    pub max_peers: usize,
    pub node_id: String,
    pub bootstrap_nodes: Vec<SocketAddr>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0:8333".parse().unwrap(),
            max_peers: 125,
            node_id: Uuid::new_v4().to_string(),
            bootstrap_nodes: Vec::new(),
        }
    }
}

/// Network manager for P2P blockchain network
pub struct Network {
    config: NetworkConfig,
    blockchain: Arc<RwLock<Blockchain>>,
    peer_manager: Arc<PeerManager>,
    message_tx: mpsc::UnboundedSender<(SocketAddr, P2PMessage)>,
    message_rx: Arc<RwLock<mpsc::UnboundedReceiver<(SocketAddr, P2PMessage)>>>,
}

impl Network {
    /// Create a new network instance
    pub fn new(config: NetworkConfig, blockchain: Arc<RwLock<Blockchain>>) -> Self {
        let (message_tx, message_rx) = mpsc::unbounded_channel();
        
        Self {
            config,
            blockchain,
            peer_manager: Arc::new(PeerManager::new(125)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
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
        let _ = tokio::join!(listen_handle, processor_handle, maintenance_handle);
        
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
                    let network = Arc::new(Self {
                        config: self.config.clone(),
                        blockchain: Arc::clone(&self.blockchain),
                        peer_manager: Arc::clone(&self.peer_manager),
                        message_tx: self.message_tx.clone(),
                        message_rx: Arc::clone(&self.message_rx),
                    });
                    
                    tokio::spawn(async move {
                        if let Err(e) = network.handle_incoming_connection(stream, addr).await {
                            warn!("Failed to handle incoming connection from {}: {}", addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Handle an incoming connection
    async fn handle_incoming_connection(&self, stream: TcpStream, addr: SocketAddr) -> Result<(), String> {
        let peer = Arc::new(Peer::new(stream, addr).await?);
        
        // Perform handshake
        let blockchain = self.blockchain.read().await;
        let height = blockchain.get_chain().len() as u64;
        drop(blockchain);
        
        peer.handshake(PROTOCOL_VERSION, height, self.config.node_id.clone()).await?;
        
        // Add to peer manager
        self.peer_manager.add_peer(Arc::clone(&peer)).await?;
        
        // Start receiving messages from this peer
        let network = Arc::new(Self {
            config: self.config.clone(),
            blockchain: Arc::clone(&self.blockchain),
            peer_manager: Arc::clone(&self.peer_manager),
            message_tx: self.message_tx.clone(),
            message_rx: Arc::clone(&self.message_rx),
        });
        
        tokio::spawn(async move {
            network.receive_from_peer(peer).await;
        });
        
        Ok(())
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
        
        // Start receiving messages
        let network = Arc::new(Self {
            config: self.config.clone(),
            blockchain: Arc::clone(&self.blockchain),
            peer_manager: Arc::clone(&self.peer_manager),
            message_tx: self.message_tx.clone(),
            message_rx: Arc::clone(&self.message_rx),
        });
        
        tokio::spawn(async move {
            network.receive_from_peer(peer).await;
        });
        
        info!("Connected to peer {}", addr);
        Ok(())
    }

    /// Receive messages from a peer
    async fn receive_from_peer(&self, peer: Arc<Peer>) {
        let addr = peer.address().await;
        
        loop {
            match peer.receive_message().await {
                Ok(msg) => {
                    debug!("Received message from {}: {:?}", addr, msg);
                    if let Err(e) = self.message_tx.send((addr, msg)) {
                        error!("Failed to queue message: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    warn!("Error receiving from {}: {}", addr, e);
                    break;
                }
            }
        }
        
        // Connection lost, remove peer
        self.peer_manager.remove_peer(addr).await;
    }

    /// Process incoming messages
    async fn process_messages(&self) {
        let mut rx = self.message_rx.write().await;
        
        while let Some((addr, msg)) = rx.recv().await {
            if let Err(e) = self.handle_message(addr, msg).await {
                error!("Error handling message from {}: {}", addr, e);
            }
        }
    }

    /// Handle a single message
    async fn handle_message(&self, addr: SocketAddr, msg: P2PMessage) -> Result<(), String> {
        match msg {
            P2PMessage::NewTx(tx) => {
                self.handle_new_transaction(tx).await?;
            }
            P2PMessage::Block(block) => {
                self.handle_new_block(block).await?;
            }
            P2PMessage::GetBlocks { start_height, end_height } => {
                self.handle_get_blocks(addr, start_height, end_height).await?;
            }
            P2PMessage::GetHeight => {
                self.handle_get_height(addr).await?;
            }
            P2PMessage::Height(height) => {
                debug!("Peer {} has height {}", addr, height);
            }
            P2PMessage::GetMempool => {
                self.handle_get_mempool(addr).await?;
            }
            P2PMessage::Mempool(txs) => {
                for tx in txs {
                    let _ = self.handle_new_transaction(tx).await;
                }
            }
            P2PMessage::Ping(nonce) => {
                self.send_to_peer(addr, P2PMessage::Pong(nonce)).await?;
            }
            P2PMessage::Pong(_) => {
                // Keep-alive response
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
    async fn handle_new_transaction(&self, tx: Transaction) -> Result<(), String> {
        let blockchain = self.blockchain.write().await;
        
        // Add to pending transactions
        if blockchain.add_transaction(tx.clone()).is_ok() {
            info!("Added new transaction to mempool");
            
            // Broadcast to other peers
            drop(blockchain);
            self.broadcast_transaction(tx).await;
        }
        
        Ok(())
    }

    /// Handle new block
    async fn handle_new_block(&self, block: Block) -> Result<(), String> {
        let blockchain = self.blockchain.write().await;
        
        // Validate and add block
        let latest = blockchain.get_latest_block();
        if block.is_valid(Some(&latest)) {
            // Check if we already have this block
            if blockchain.get_chain().iter().any(|b| b.hash == block.hash) {
                return Ok(());
            }
            
            // Add block to chain
            blockchain.get_chain_mut().push(block.clone());
            
            // Clear mined transactions from pending
            let block_txs = block.transactions.clone();
            blockchain.get_pending_transactions_mut().retain(|t| !block_txs.contains(t));
            
            // Update UTXO set
            for tx in &block.transactions {
                if !tx.is_coinbase() {
                    let _ = blockchain.get_utxo_set_mut().spend_utxos(&tx.sender, tx.amount + tx.fee);
                }
                blockchain.get_utxo_set_mut().add_utxo(tx);
            }
            
            info!("Added new block {} at height {}", block.hash[..8].to_string(), block.index);
            
            // Broadcast to other peers
            drop(blockchain);
            self.broadcast_block(block).await;
        } else {
            return Err("Invalid block".to_string());
        }
        
        Ok(())
    }

    /// Handle get blocks request
    async fn handle_get_blocks(&self, addr: SocketAddr, start: u64, end: u64) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        let blocks: Vec<Block> = (start..=end)
            .filter_map(|i| blockchain.get_chain().get(i as usize).cloned())
            .collect();
        drop(blockchain);
        
        for block in blocks {
            self.send_to_peer(addr, P2PMessage::Block(block)).await?;
        }
        
        Ok(())
    }

    /// Handle get height request
    async fn handle_get_height(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        let height = blockchain.get_chain().len() as u64;
        
        self.send_to_peer(addr, P2PMessage::Height(height)).await
    }

    /// Handle get mempool request
    async fn handle_get_mempool(&self, addr: SocketAddr) -> Result<(), String> {
        let blockchain = self.blockchain.read().await;
        let txs = blockchain.get_pending_transactions().clone();
        
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
    pub async fn sync_blockchain(&self) -> Result<(), String> {
        let peers = self.peer_manager.get_peers().await;
        
        if peers.is_empty() {
            return Ok(());
        }
        
        info!("Starting blockchain synchronization");
        
        // Get our height
        let our_height = self.blockchain.read().await.get_chain().len() as u64;
        
        // Ask all peers for their height
        for peer in &peers {
            let _ = peer.send_message(P2PMessage::GetHeight).await;
        }
        
        tokio::time::sleep(Duration::from_secs(2)).await;
        
        // Find peer with highest height
        let mut max_height = our_height;
        let mut best_peer: Option<Arc<Peer>> = None;
        
        for peer in &peers {
            let info = peer.get_info().await;
            if info.height > max_height {
                max_height = info.height;
                best_peer = Some(Arc::clone(peer));
            }
        }
        
        if let Some(peer) = best_peer {
            info!("Syncing from peer with height {}", max_height);
            
            // Request missing blocks
            let _ = peer.send_message(P2PMessage::GetBlocks {
                start_height: our_height,
                end_height: max_height,
            }).await;
            
            // Wait for blocks to arrive
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            info!("Blockchain sync complete");
        }
        
        Ok(())
    }

    /// Maintain peer connections
    async fn maintain_peers(&self) {
        let mut ticker = interval(Duration::from_secs(30));
        
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
            if peer_count < 3 && !self.config.bootstrap_nodes.is_empty() {
                // Try reconnecting to bootstrap nodes
                for addr in &self.config.bootstrap_nodes {
                    let network = Arc::new(Self {
                        config: self.config.clone(),
                        blockchain: Arc::clone(&self.blockchain),
                        peer_manager: Arc::clone(&self.peer_manager),
                        message_tx: self.message_tx.clone(),
                        message_rx: Arc::clone(&self.message_rx),
                    });
                    let addr = *addr;
                    tokio::spawn(async move {
                        let _ = network.connect_to_peer(addr).await;
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
    pub async fn get_peers_info(&self) -> Vec<crate::p2p::peer::PeerInfo> {
        let peers = self.peer_manager.get_peers().await;
        let mut info = Vec::new();
        
        for peer in peers {
            info.push(peer.get_info().await);
        }
        
        info
    }
}
