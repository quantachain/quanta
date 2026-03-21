use axum::{
    extract::{State, Json, Path},
    routing::{get, post},
    Router, http::StatusCode,
    http::Method,
    response::IntoResponse,
};
use tower_http::cors::CorsLayer;
use tower::ServiceBuilder;
use axum::middleware::{self, Next};
use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::response::Response;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use std::num::NonZeroUsize;
use lru::LruCache;
use parking_lot::Mutex;
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
///  CRITICAL SECURITY WARNING 
/// This endpoint accepts wallet passwords over HTTP - EXTREMELY DANGEROUS!
/// 
///  DO NOT USE IN PRODUCTION WITHOUT CHANGES 
/// 
/// SAFE USE CASES ONLY:
/// - Local development (127.0.0.1 ONLY)
/// - Single-user personal nodes (not public RPC)
/// - Behind reverse proxy with TLS + authentication
/// 
/// FOR PRODUCTION TESTNET/MAINNET:
///  Client-side signing (users sign locally, submit pre-signed tx)
///  Hardware wallet integration
///  Never transmit private keys or passwords
///  Use POST /api/transactions/submit with pre-signed transactions
/// 
/// TODO: Disable this endpoint for public RPC nodes
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
    pub nonce: u64,              // Current confirmed nonce for this address
}

async fn get_balance(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BalanceRequest>,
) -> Json<BalanceResponse> {
    let blockchain = state.blockchain.read().await;
    let balance = blockchain.get_balance(&req.address);
    let nonce = blockchain.get_account_state_read().get_nonce(&req.address);
    Json(BalanceResponse {
        address: req.address,
        balance_microunits: balance,
        nonce,
    })
}

/// CRIT-1 FIX: Password-over-HTTP endpoint replaced with pre-signed submission.
///
/// `POST /api/transactions/submit` — accepts a fully-signed Transaction JSON.
/// Clients must sign locally (CLI: `quanta-wallet sign`, or use a hardware wallet).
/// No password or private key ever leaves the user's machine.
async fn submit_signed_transaction(
    State(state): State<Arc<ApiState>>,
    Json(tx): Json<Transaction>,
) -> (StatusCode, Json<TransactionResponse>) {
    // Validate the transaction has a non-empty signature before accepting
    if tx.signature.is_empty() || tx.public_key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(TransactionResponse {
                success: false,
                tx_hash: None,
                error: Some("Transaction must be pre-signed (signature and public_key required)".to_string()),
            }),
        );
    }

    let blockchain = state.blockchain.read().await;
    match blockchain.add_transaction(tx.clone()) {
        Ok(_) => {
            let tx_hash = tx.hash();
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
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(TransactionResponse {
                success: false,
                tx_hash: None,
                error: Some(format!("Transaction failed: {}", e)),
            }),
        ),
    }
}

// REMOVED: create_transaction_local_only — path-traversal risk (wallet_file from POST body) +
// password-over-HTTP. Replaced by POST /api/transactions/submit (pre-signed, no password).


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
    // HIGH-5 FIX: Validate miner_address format before mining.
    // Accepts two address formats:
    //   - Standard wallet:  0x<40 hex chars>     (e.g. 0xabcdef...)
    //   - Multisig wallet:  ms<40+ hex chars>    (e.g. ms69216b1d10...)
    fn valid_quanta_addr(addr: &str) -> bool {
        if addr.starts_with("0x") {
            addr.len() == 42 && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
        } else if addr.starts_with("ms") {
            addr.len() >= 42 && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
        } else {
            false
        }
    }
    if !valid_quanta_addr(&req.miner_address) {
        return (
            StatusCode::BAD_REQUEST,
            Json(MineResponse {
                success: false,
                block_index: None,
                error: Some("Invalid miner_address: must be 0x<40 hex> or ms<40+ hex>".to_string()),
            }),
        );
    }

    // 1. Create template (Lock held briefly)
    let template_res = state.blockchain.read().await.create_block_template(req.miner_address.clone());

    match template_res {
        Ok(mut block) => {
            // 2. Mine (NO LOCK held on blockchain)
            // Run CPU-intensive mining in a blocking task
            let mined_block_res = tokio::task::spawn_blocking(move || {
                block.mine();
                block
            }).await;

            if let Ok(mined_block) = mined_block_res {
                 // 3. Submit (Lock held briefly to commit)
                 let blockchain = state.blockchain.read().await;
                 match blockchain.add_network_block(mined_block.clone()) {
                     Ok(_) => {
                         let index = mined_block.index;
                         
                         // Broadcast to network if available
                         if let Some(ref network) = state.network {
                             network.broadcast_block(mined_block).await;
                         }
                         drop(blockchain);

                         (
                             StatusCode::OK,
                             Json(MineResponse {
                                 success: true,
                                 block_index: Some(index),
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
                                error: Some(format!("Failed to add block: {}", e)),
                            }),
                        )
                     }
                 }
            } else {
                 (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(MineResponse {
                        success: false,
                        block_index: None,
                        error: Some("Mining task panicked".to_string()),
                    }),
                )
            }
        }
        Err(e) => {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(MineResponse {
                    success: false,
                    block_index: None,
                    error: Some(format!("Failed to create template: {}", e)),
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
            
            // 1. Create template
            let template_res = blockchain.read().await.create_block_template(miner_address.clone());
            
            match template_res {
                Ok(mut block) => {
                    // 2. Mine (NO LOCK)
                    let mined_block_res = tokio::task::spawn_blocking(move || {
                        block.mine();
                        block
                    }).await;
                    
                    if let Ok(mined_block) = mined_block_res {
                        // Check if still active
                        if !mining_active.load(Ordering::Relaxed) {
                            break;
                        }
                        
                        // 3. Submit
                        let bc = blockchain.read().await;
                        match bc.add_network_block(mined_block.clone()) {
                            Ok(_) => {
                                if let Some(ref net) = network {
                                    net.broadcast_block(mined_block).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to submit mined block: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Continuous mining error (create template): {}", e);
                    // Sleep to avoid tight loop on error
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
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

/// Get node metrics (Prometheus format)
async fn get_metrics(
    State(state): State<Arc<ApiState>>,
) -> impl IntoResponse {
    let metrics = if let Some(ref metrics) = state.metrics {
        metrics.get_metrics().await
    } else {
        NodeMetrics::default()
    };
    
    // Convert to Prometheus Text Format
    let s = format!(
        "# HELP quanta_peer_count Number of connected peers\n\
         # TYPE quanta_peer_count gauge\n\
         quanta_peer_count {}\n\
         \n\
         # HELP quanta_blocks_mined Total blocks mined by this node\n\
         # TYPE quanta_blocks_mined counter\n\
         quanta_blocks_mined {}\n\
         \n\
         # HELP quanta_chain_height Current blockchain height\n\
         # TYPE quanta_chain_height gauge\n\
         quanta_chain_height {}\n\
         \n\
         # HELP quanta_mempool_size Number of transactions in mempool\n\
         # TYPE quanta_mempool_size gauge\n\
         quanta_mempool_size {}\n\
         \n\
         # HELP quanta_node_uptime_seconds Node uptime in seconds\n\
         # TYPE quanta_node_uptime_seconds gauge\n\
         quanta_node_uptime_seconds {}\n\
         \n\
         # HELP quanta_blocks_received Total blocks received from network\n\
         # TYPE quanta_blocks_received counter\n\
         quanta_blocks_received {}\n\
         \n\
         # HELP quanta_transactions_received Total transactions received\n\
         # TYPE quanta_transactions_received counter\n\
         quanta_transactions_received {}\n\
        ",
        metrics.connected_peers,
        metrics.blocks_mined,
        metrics.chain_height,
        metrics.mempool_size,
        metrics.node_uptime_secs,
        metrics.blocks_received,
        metrics.transactions_received
    );

    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        s
    )
}

/// Get specific block by height — CRIT-5 FIX: reads from storage, not in-memory chain
async fn get_block(
    State(state): State<Arc<ApiState>>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, StatusCode> {
    let blockchain = state.blockchain.read().await;
    // CRIT-5 FIX: Previous implementation called get_chain().get(height) which
    // only accessed the genesis block — every height > 0 returned 404.
    // load_block_from_storage reads from sled disk DB (the actual chain).
    match blockchain.load_block_from_storage(height) {
        Some(block) => Ok(Json(block)),
        None => Err(StatusCode::NOT_FOUND),
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

static RATE_LIMITS: std::sync::OnceLock<Mutex<LruCache<std::net::IpAddr, (u32, Instant)>>> = std::sync::OnceLock::new();

/// Custom Rate Limiter (CRIT-2 FIX) — 10 requests/sec per IP burst limit.
/// SECURITY: Wrapped in LruCache (max 100,000 IPs) instead of DashMap to 
/// prevent memory exhaustion (OOM) under distributed botnet attacks.
async fn rate_limiter(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let limits = RATE_LIMITS.get_or_init(|| Mutex::new(LruCache::new(NonZeroUsize::new(100_000).unwrap())));
    
    let allow = {
        let mut cache = limits.lock();
        let ip = addr.ip();
        let now = Instant::now();
        
        match cache.get_mut(&ip) {
            Some((count, time)) => {
                if now.duration_since(*time) > Duration::from_secs(1) {
                    *count = 1;
                    *time = now;
                    true
                } else {
                    *count += 1;
                    *count <= 10
                }
            }
            None => {
                cache.put(ip, (1, now));
                true
            }
        }
    };

    if !allow {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    Ok(next.run(request).await)
}

/// Create the API router with rate limiting (DoS protection)
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

    // CRIT-1 FIX: CORS restricted to localhost only (no wildcard origin).
    // C-2 FIX: allow_headers restricted to Content-Type only (not Any).
    //   Any allows Authorization / X-Admin headers cross-origin — CSRF risk.
    let cors = CorsLayer::new()
        .allow_origin(
            "http://localhost:3000"
                .parse::<axum::http::HeaderValue>()
                .expect("valid CORS origin"),
        )
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    Router::new()
        .route("/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .route("/api/balance", post(get_balance))
        // CRIT-1: New pre-signed tx submission endpoint (no password needed)
        .route("/api/transactions/submit", post(submit_signed_transaction))
        // Old endpoint kept but hidden — disabled in public builds
        // .route("/api/transaction", post(create_transaction_local_only))
        .route("/api/mine", post(mine_block))
        .route("/api/mine/start", post(start_continuous_mining))
        .route("/api/mine/stop", post(stop_continuous_mining))
        .route("/api/mine/status", get(get_mining_status))
        .route("/api/validate", get(validate_chain))
        .route("/api/peers", get(get_peers))
        .route("/api/metrics", get(get_metrics))
        .route("/api/block/:height", get(get_block))
        .route("/api/mempool", get(get_mempool))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(rate_limiter))
                .layer(cors)
        )
        .with_state(state)
}


/// Start the API server.
///
/// CRIT-1 FIX: Binds to `127.0.0.1` (localhost only) unless TLS is configured.
/// Prevents password interception by blocking external access to the API port.
pub async fn start_server(
    blockchain: Arc<RwLock<Blockchain>>,
    port: u16,
    metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    network: Option<Arc<crate::network::Network>>,
    tls_enabled: bool,
) {
    let app = create_router(blockchain, metrics, network);

    // CRIT-1 FIX: Only bind to 0.0.0.0 when TLS is active; otherwise localhost only.
    let bind_host = if tls_enabled { "0.0.0.0" } else { "127.0.0.1" };
    let addr = format!("{}:{}", bind_host, port);
    
    tracing::info!("QUANTA API server starting on {} (TLS={})", addr, tls_enabled);
    tracing::info!("Endpoints:");
    tracing::info!("   GET  /health - Health check");
    tracing::info!("   GET  /api/stats - Get blockchain statistics");
    tracing::info!("   POST /api/balance - Get address balance");
    tracing::info!("   POST /api/transactions/submit - Submit pre-signed transaction");
    tracing::info!("   POST /api/mine - Mine a block");
    tracing::info!("   GET  /api/validate - Validate blockchain");
    tracing::info!("   GET  /api/peers - Get connected peers");
    tracing::info!("   GET  /api/metrics - Get node metrics");
    tracing::info!("   GET  /api/block/:height - Get specific block");
    tracing::info!("   GET  /api/mempool - Get pending transactions");
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind server");
    
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .expect("Server error");
}
