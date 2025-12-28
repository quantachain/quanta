use axum::{
    extract::{State, Json, Path},
    routing::{get, post},
    Router, http::StatusCode,
    http::Method,
};
use tower_http::cors::{CorsLayer, Any};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::consensus::blockchain::{Blockchain, BlockchainStats};
use crate::core::transaction::Transaction;
use crate::crypto::wallet::QuantumWallet;
use crate::consensus::mempool::NodeMetrics;
use crate::core::block::Block;
use std::sync::atomic::{AtomicBool, Ordering};

/// API state
pub struct ApiState {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    pub network: Option<Arc<crate::network::Network>>,
    pub mining_active: Arc<AtomicBool>,
}

/// Request to create a transaction
/// ⚠️ SECURITY WARNING: This endpoint accepts wallet passwords over HTTP.
/// This is ONLY safe for:
/// - Local development (localhost)
/// - Single-user nodes
/// - Trusted networks
/// 
/// For production/public RPC:
/// - Client should sign transactions locally
/// - API should only accept pre-signed transactions
/// - Server should NEVER see private keys or passwords
#[derive(Deserialize)]
pub struct CreateTransactionRequest {
    pub wallet_file: String,
    pub wallet_password: String,
    pub recipient: String,
    pub amount_microunits: u64, // Amount in microunits (1 QUA = 1_000_000)
}

/// Response with transaction hash
#[derive(Serialize)]
pub struct TransactionResponse {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
}

/// Get blockchain stats
async fn get_stats(
    State(state): State<Arc<ApiState>>,
) -> Json<BlockchainStats> {
    let blockchain = state.blockchain.read().await;
    Json(blockchain.get_stats())
}

/// Get balance for an address
#[derive(Deserialize)]
pub struct BalanceRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance_microunits: u64, // Balance in microunits (1 QUA = 1_000_000)
}

async fn get_balance(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BalanceRequest>,
) -> Json<BalanceResponse> {
    let blockchain = state.blockchain.read().await;
    let balance = blockchain.get_balance(&req.address);
    Json(BalanceResponse {
        address: req.address,
        balance_microunits: balance,
    })
}

/// Create and submit a transaction
async fn create_transaction(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateTransactionRequest>,
) -> (StatusCode, Json<TransactionResponse>) {
    // Load quantum-safe wallet
    let wallet = match QuantumWallet::load_quantum_safe(&req.wallet_file, &req.wallet_password) {
        Ok(w) => w,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(TransactionResponse {
                    success: false,
                    tx_hash: None,
                    error: Some(format!("Failed to load wallet: {}", e)),
                }),
            );
        }
    };

    // Get current nonce
    let blockchain = state.blockchain.read().await;
    let current_nonce = blockchain.get_utxo_set_mut().get_nonce(&wallet.address);
    let next_nonce = current_nonce + 1;
    drop(blockchain);

    // Create transaction with microunits
    let mut tx = Transaction::new(
        wallet.address.clone(),
        req.recipient,
        req.amount_microunits,
        chrono::Utc::now().timestamp(),
    );
    tx.nonce = next_nonce;

    // Sign transaction
    let signature = wallet.keypair.sign_transaction_data(&tx.get_signing_data());
    
    tx.signature = signature;
    tx.public_key = wallet.keypair.public_key.clone();

    // Submit to blockchain
    let blockchain = state.blockchain.write().await;
    match blockchain.add_transaction(tx.clone()) {
        Ok(_) => {
            let tx_hash = tx.hash();
            
            // Broadcast to network if available
            drop(blockchain);
            if let Some(ref network) = state.network {
                network.broadcast_transaction(tx).await;
            }
            
            (
                StatusCode::OK,
                Json(TransactionResponse {
                    success: true,
                    tx_hash: Some(tx_hash),
                    error: None,
                }),
            )
        }
        Err(e) => {
            (
                StatusCode::BAD_REQUEST,
                Json(TransactionResponse {
                    success: false,
                    tx_hash: None,
                    error: Some(format!("Transaction failed: {}", e)),
                }),
            )
        }
    }
}

/// Mine request
#[derive(Deserialize)]
pub struct MineRequest {
    pub miner_address: String,
}

#[derive(Serialize)]
pub struct MineResponse {
    pub success: bool,
    pub block_index: Option<u64>,
    pub error: Option<String>,
}

async fn mine_block(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<MineRequest>,
) -> (StatusCode, Json<MineResponse>) {
    let blockchain = state.blockchain.write().await;
    match blockchain.mine_pending_transactions(req.miner_address) {
        Ok(_) => {
            let stats = blockchain.get_stats();
            let block = blockchain.get_chain().last().cloned();
            drop(blockchain);
            
            // Get the mined block
            if let Some(block) = block {
                
                // Broadcast to network if available
                if let Some(ref network) = state.network {
                    network.broadcast_block(block).await;
                }
            }
            
            (
                StatusCode::OK,
                Json(MineResponse {
                    success: true,
                    block_index: Some(stats.chain_length as u64 - 1),
                    error: None,
                }),
            )
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MineResponse {
                    success: false,
                    block_index: None,
                    error: Some(format!("Mining failed: {}", e)),
                }),
            )
        }
    }
}

/// Start continuous mining
async fn start_continuous_mining(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<MineRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    if state.mining_active.load(Ordering::Relaxed) {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "already_running", "message": "Mining already active" }))
        );
    }
    
    state.mining_active.store(true, Ordering::Relaxed);
    let blockchain = state.blockchain.clone();
    let network = state.network.clone();
    let mining_active = state.mining_active.clone();
    let miner_address = req.miner_address.clone();
    
    tokio::spawn(async move {
        while mining_active.load(Ordering::Relaxed) {
            // Check if there are transactions to mine
            let has_txs = {
                let bc = blockchain.read().await;
                let result = !bc.get_pending_transactions().is_empty();
                result
            };
            
            if !has_txs {
                // No transactions - sleep longer to avoid CPU waste
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            
            let bc = blockchain.write().await;
            match bc.mine_pending_transactions(miner_address.clone()) {
                Ok(_) => {
                    let block = bc.get_chain().last().cloned();
                    drop(bc);
                    
                    if let Some(block) = block {
                        if let Some(ref net) = network {
                            net.broadcast_block(block).await;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Continuous mining error: {}", e);
                    break;
                }
            }
            
            // Small delay between blocks
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });
    
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "started", "message": "Continuous mining started" }))
    )
}

/// Stop continuous mining
async fn stop_continuous_mining(
    State(state): State<Arc<ApiState>>,
) -> Json<serde_json::Value> {
    state.mining_active.store(false, Ordering::Relaxed);
    Json(serde_json::json!({ "status": "stopped", "message": "Continuous mining stopped" }))
}

/// Get mining status
#[derive(Serialize)]
pub struct MiningStatus {
    pub active: bool,
}

async fn get_mining_status(
    State(state): State<Arc<ApiState>>,
) -> Json<MiningStatus> {
    Json(MiningStatus {
        active: state.mining_active.load(Ordering::Relaxed),
    })
}

/// Validate blockchain
#[derive(Serialize)]
pub struct ValidateResponse {
    pub is_valid: bool,
}

async fn validate_chain(
    State(state): State<Arc<ApiState>>,
) -> Json<ValidateResponse> {
    let blockchain = state.blockchain.read().await;
    Json(ValidateResponse {
        is_valid: blockchain.is_valid(),
    })
}

/// Get network peers
#[derive(Serialize)]
pub struct PeersResponse {
    pub peer_count: usize,
    pub peers: Vec<PeerInfoResponse>,
}

#[derive(Serialize)]
pub struct PeerInfoResponse {
    pub address: String,
    pub node_id: String,
    pub height: u64,
    pub connected_for: i64,
}

async fn get_peers(
    State(state): State<Arc<ApiState>>,
) -> Json<PeersResponse> {
    if let Some(ref network) = state.network {
        let peers_info = network.get_peers_info().await;
        let peers: Vec<PeerInfoResponse> = peers_info
            .into_iter()
            .map(|p| PeerInfoResponse {
                address: p.address.to_string(),
                node_id: p.node_id,
                height: p.height,
                connected_for: chrono::Utc::now().timestamp() - p.connected_at,
            })
            .collect();
        
        Json(PeersResponse {
            peer_count: peers.len(),
            peers,
        })
    } else {
        Json(PeersResponse {
            peer_count: 0,
            peers: Vec::new(),
        })
    }
}

/// Get node metrics
async fn get_metrics(
    State(state): State<Arc<ApiState>>,
) -> Json<NodeMetrics> {
    if let Some(ref metrics) = state.metrics {
        Json(metrics.get_metrics().await)
    } else {
        Json(NodeMetrics::default())
    }
}

/// Get specific block by height
async fn get_block(
    State(state): State<Arc<ApiState>>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, StatusCode> {
    let blockchain = state.blockchain.read().await;
    let block = blockchain.get_chain().get(height as usize).cloned();
    drop(blockchain);
    
    if let Some(block) = block {
        Ok(Json(block))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Get mempool transactions
#[derive(Serialize)]
pub struct MempoolResponse {
    pub transaction_count: usize,
    pub transactions: Vec<Transaction>,
}

async fn get_mempool(
    State(state): State<Arc<ApiState>>,
) -> Json<MempoolResponse> {
    let blockchain = state.blockchain.read().await;
    let transactions = blockchain.get_pending_transactions().clone();
    
    Json(MempoolResponse {
        transaction_count: transactions.len(),
        transactions,
    })
}

/// Health check endpoint
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub chain_height: u64,
    pub mempool_size: usize,
    pub connected_peers: usize,
    pub uptime_seconds: u64,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

async fn health_check(
    State(state): State<Arc<ApiState>>,
) -> Json<HealthResponse> {
    let blockchain = state.blockchain.read().await;
    let stats = blockchain.get_stats();
    
    let peers_count = if let Some(ref network) = state.network {
        network.get_peer_count().await
    } else {
        0
    };
    
    let uptime = START_TIME
        .get_or_init(|| std::time::Instant::now())
        .elapsed()
        .as_secs();
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        chain_height: stats.chain_length as u64,
        mempool_size: stats.pending_transactions,
        connected_peers: peers_count,
        uptime_seconds: uptime,
    })
}

/// Create the API router
pub fn create_router(
    blockchain: Arc<RwLock<Blockchain>>,
    metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    network: Option<Arc<crate::network::Network>>,
) -> Router {
    let state = Arc::new(ApiState { 
        blockchain,
        metrics,
        network,
        mining_active: Arc::new(AtomicBool::new(false)),
    });

    // Configure CORS to allow requests from any origin
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .route("/api/balance", post(get_balance))
        .route("/api/transaction", post(create_transaction))
        .route("/api/mine", post(mine_block))
        .route("/api/mine/start", post(start_continuous_mining))
        .route("/api/mine/stop", post(stop_continuous_mining))
        .route("/api/mine/status", get(get_mining_status))
        .route("/api/validate", get(validate_chain))
        .route("/api/peers", get(get_peers))
        .route("/api/metrics", get(get_metrics))
        .route("/api/block/:height", get(get_block))
        .route("/api/mempool", get(get_mempool))
        .layer(cors)
        .with_state(state)
}

/// Start the API server
pub async fn start_server(
    blockchain: Arc<RwLock<Blockchain>>,
    port: u16,
    metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    network: Option<Arc<crate::network::Network>>,
) {
    let app = create_router(blockchain, metrics, network);
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("QUANTA API server starting on {}", addr);
    tracing::info!("Endpoints:");
    tracing::info!("   GET  /health - Health check");
    tracing::info!("   GET  /api/stats - Get blockchain statistics");
    tracing::info!("   POST /api/balance - Get address balance");
    tracing::info!("   POST /api/transaction - Create transaction");
    tracing::info!("   POST /api/mine - Mine a block");
    tracing::info!("   GET  /api/validate - Validate blockchain");
    tracing::info!("   GET  /api/peers - Get connected peers");
    tracing::info!("   GET  /api/metrics - Get node metrics");
    tracing::info!("   GET  /api/block/:height - Get specific block");
    tracing::info!("   GET  /api/mempool - Get pending transactions");
    tracing::info!("   POST /api/merkle/proof - Get Merkle proof for transaction");
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind server");
    
    axum::serve(listener, app)
        .await
        .expect("Server error");
}
