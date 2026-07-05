use crate::consensus::blockchain::{AddressTransaction, Blockchain, BlockchainStats};
use crate::core::transaction::Transaction;
use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::{
    extract::{Json, Path, Query, State},
    http::Method,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use lru::LruCache;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

use crate::consensus::mempool::NodeMetrics;
use crate::core::block::Block;
use std::sync::atomic::{AtomicBool, Ordering};

/// API state
pub struct ApiState {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    pub network: Option<Arc<crate::network::Network>>,
}

/// Response with transaction hash
#[derive(Serialize)]
pub struct TransactionResponse {
    pub success: bool,
    pub tx_hash: Option<String>,
    pub error: Option<String>,
}

// -----------------------------------------------------------------------
// Core stats
// -----------------------------------------------------------------------

/// Get blockchain stats
async fn get_stats(State(state): State<Arc<ApiState>>) -> Json<BlockchainStats> {
    let blockchain = state.blockchain.read().await;
    Json(blockchain.get_stats())
}

// -----------------------------------------------------------------------
// Balance (legacy POST — kept for backward compat)
// -----------------------------------------------------------------------

/// Get balance for an address (POST body)
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

// -----------------------------------------------------------------------
// Block Explorer — Address endpoints (GET-based, scanner-friendly)
// -----------------------------------------------------------------------

/// Full address information returned by GET /api/address/:address
#[derive(Serialize)]
pub struct AddressInfoResponse {
    pub address: String,
    /// Immediately spendable balance (microunits)
    pub balance_microunits: u64,
    /// Spendable balance in QUA
    pub balance_qua: f64,
    /// Total balance including all locked/vesting entries (microunits)
    pub total_balance_microunits: u64,
    /// Total balance in QUA
    pub total_balance_qua: f64,
    /// Number of transactions sent FROM this address (on-chain nonce)
    pub nonce: u64,
    /// Active locked / vesting entries
    pub locked_balances: Vec<LockedBalanceResponse>,
}

#[derive(Serialize)]
pub struct LockedBalanceResponse {
    pub amount_microunits: u64,
    pub amount_qua: f64,
    pub unlock_height: u64,
}

/// GET /api/address/:address
/// Returns full account information: balance, total balance (locked + spendable),
/// nonce, and locked-balance details. Never returns 404 — unknown addresses
/// return zeroed fields (address might receive a future tx).
async fn get_address_info(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
) -> Json<AddressInfoResponse> {
    let blockchain = state.blockchain.read().await;
    let account_state = blockchain.get_account_state_read();

    let balance_microunits = account_state.get_balance(&address);
    let total_microunits = account_state.get_total_balance(&address);
    let nonce = account_state.get_nonce(&address);

    let locked_balances = account_state
        .get_account(&address)
        .map(|acc| {
            acc.locked_balances
                .iter()
                .map(|lb| LockedBalanceResponse {
                    amount_microunits: lb.amount,
                    amount_qua: lb.amount as f64 / 1_000_000.0,
                    unlock_height: lb.unlock_height,
                })
                .collect()
        })
        .unwrap_or_default();

    Json(AddressInfoResponse {
        address,
        balance_microunits,
        balance_qua: balance_microunits as f64 / 1_000_000.0,
        total_balance_microunits: total_microunits,
        total_balance_qua: total_microunits as f64 / 1_000_000.0,
        nonce,
        locked_balances,
    })
}

/// GET /api/balance/:address  (GET alias — allows direct link from scanner UI)
async fn get_balance_by_path(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
) -> Json<BalanceResponse> {
    let blockchain = state.blockchain.read().await;
    let balance = blockchain.get_balance(&address);
    let nonce = blockchain.get_account_state_read().get_nonce(&address);
    Json(BalanceResponse {
        address,
        balance_microunits: balance,
        nonce,
    })
}

/// Query parameters for address tx history
#[derive(Deserialize)]
pub struct AddressTxsQuery {
    /// How many blocks to scan backwards. Defaults to 10_000. Max enforced at 50_000.
    pub max_blocks: Option<u64>,
}

/// GET /api/address/:address/txs?max_blocks=N
/// Returns all confirmed transactions (sent or received) for an address,
/// scanning backward from the chain tip. Capped at 500 transactions.
async fn get_address_transactions(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
    Query(params): Query<AddressTxsQuery>,
) -> Json<AddressTxsResponse> {
    let max_blocks = params.max_blocks.unwrap_or(10_000).min(50_000);
    let blockchain = state.blockchain.read().await;
    let txs = blockchain.get_address_transactions(&address, max_blocks);
    let count = txs.len();
    Json(AddressTxsResponse {
        address,
        transaction_count: count,
        transactions: txs,
    })
}

#[derive(Serialize)]
pub struct AddressTxsResponse {
    pub address: String,
    pub transaction_count: usize,
    pub transactions: Vec<AddressTransaction>,
}

// -----------------------------------------------------------------------
// Block Explorer — Transaction lookup by hash
// -----------------------------------------------------------------------

/// GET /api/tx/:hash
/// Look up a confirmed transaction by its hash using the O(1) storage index.
/// Falls back to the mempool if not yet confirmed.
/// Returns 404 only when the hash is completely unknown.
async fn get_tx_handler(
    State(state): State<Arc<ApiState>>,
    Path(hash): Path<String>,
) -> Result<Json<TxDetailResponse>, StatusCode> {
    let blockchain = state.blockchain.read().await;

    // 1. Check confirmed chain first via O(1) storage index
    if let Some(tx) = blockchain.find_transaction_by_hash(&hash) {
        return Ok(Json(TxDetailResponse {
            tx_hash: hash,
            status: "confirmed".to_string(),
            transaction: tx,
            block_height: None,
        }));
    }

    // 2. Fall back to mempool
    let pending = blockchain.get_pending_transactions();
    if let Some(tx) = pending.iter().find(|t| t.hash() == hash) {
        return Ok(Json(TxDetailResponse {
            tx_hash: hash,
            status: "pending".to_string(),
            transaction: tx.clone(),
            block_height: None,
        }));
    }

    Err(StatusCode::NOT_FOUND)
}

/// Response for GET /api/tx/:hash
#[derive(Serialize)]
pub struct TxDetailResponse {
    pub tx_hash: String,
    /// "confirmed" | "pending"
    pub status: String,
    pub transaction: Transaction,
    /// Present when status == "confirmed" and block info is recoverable
    pub block_height: Option<u64>,
}

// -----------------------------------------------------------------------
// Block Explorer — Latest blocks feed
// -----------------------------------------------------------------------

/// Query parameters for /api/blocks/latest
#[derive(Deserialize)]
pub struct LatestBlocksQuery {
    /// Number of blocks to return. Defaults to 10. Capped at 100.
    pub count: Option<usize>,
}

/// GET /api/blocks/latest?count=N
/// Returns the N most recent blocks (newest first), including all transactions.
/// Useful for the block explorer live feed. Count is capped at 100.
async fn get_latest_blocks(
    State(state): State<Arc<ApiState>>,
    Query(params): Query<LatestBlocksQuery>,
) -> Json<LatestBlocksResponse> {
    let count = params.count.unwrap_or(10).min(100);
    let blockchain = state.blockchain.read().await;
    let blocks = blockchain.get_latest_blocks(count);
    let block_count = blocks.len();
    Json(LatestBlocksResponse {
        block_count,
        blocks,
    })
}

#[derive(Serialize)]
pub struct LatestBlocksResponse {
    pub block_count: usize,
    pub blocks: Vec<Block>,
}

// -----------------------------------------------------------------------
// Transaction submission (pre-signed)
// -----------------------------------------------------------------------

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
                error: Some(
                    "Transaction must be pre-signed (signature and public_key required)"
                        .to_string(),
                ),
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

// -----------------------------------------------------------------------
// Mining
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// Validation / Peers / Metrics
// -----------------------------------------------------------------------

/// Validate blockchain
#[derive(Serialize)]
pub struct ValidateResponse {
    pub is_valid: bool,
}

async fn validate_chain(State(state): State<Arc<ApiState>>) -> Json<ValidateResponse> {
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

async fn get_peers(State(state): State<Arc<ApiState>>) -> Json<PeersResponse> {
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

// -----------------------------------------------------------------------
// Validators
// -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct ValidatorInfoResponse {
    pub address: String,
    pub falcon_pk_hex: String,
    pub stake_microunits: u64,
    pub registered_epoch: u64,
    pub active: bool,
}

#[derive(Serialize)]
pub struct ValidatorsResponse {
    pub active_count: usize,
    pub validators: Vec<ValidatorInfoResponse>,
}

async fn get_validators(State(state): State<Arc<ApiState>>) -> Json<ValidatorsResponse> {
    let blockchain = state.blockchain.read().await;
    let account_state = blockchain.get_account_state_read();
    let validators_map = account_state.get_validators();

    let mut validators: Vec<ValidatorInfoResponse> = validators_map
        .iter()
        .map(|(addr, info)| ValidatorInfoResponse {
            address: addr.clone(),
            falcon_pk_hex: hex::encode(&info.falcon_pk),
            stake_microunits: info.stake,
            registered_epoch: info.registered_epoch,
            active: info.active,
        })
        .collect();

    // Sort by stake descending
    validators.sort_by(|a, b| b.stake_microunits.cmp(&a.stake_microunits));

    let active_count = validators.iter().filter(|v| v.active).count();

    Json(ValidatorsResponse {
        active_count,
        validators,
    })
}

/// Get node metrics (Prometheus format)
async fn get_metrics(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    let metrics = if let Some(ref metrics) = state.metrics {
        metrics.get_metrics().await
    } else {
        NodeMetrics::default()
    };

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
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        s,
    )
}

// -----------------------------------------------------------------------
// Block / Mempool
// -----------------------------------------------------------------------

/// Get specific block by height — CRIT-5 FIX: reads from storage, not in-memory chain
async fn get_block(
    State(state): State<Arc<ApiState>>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, StatusCode> {
    let blockchain = state.blockchain.read().await;
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

async fn get_mempool(State(state): State<Arc<ApiState>>) -> Json<MempoolResponse> {
    let blockchain = state.blockchain.read().await;
    let transactions = blockchain.get_pending_transactions().clone();

    Json(MempoolResponse {
        transaction_count: transactions.len(),
        transactions,
    })
}

// -----------------------------------------------------------------------
// Health check
// -----------------------------------------------------------------------

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub chain_height: u64,
    pub mempool_size: usize,
    pub connected_peers: usize,
    pub uptime_seconds: u64,
}

static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

async fn health_check(State(state): State<Arc<ApiState>>) -> Json<HealthResponse> {
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

// -----------------------------------------------------------------------
// Rate limiting middleware
// -----------------------------------------------------------------------

static RATE_LIMITS: std::sync::OnceLock<Mutex<LruCache<std::net::IpAddr, (u32, Instant)>>> =
    std::sync::OnceLock::new();

/// Custom Rate Limiter (CRIT-2 FIX) — 10 requests/sec per IP burst limit.
/// SECURITY: Wrapped in LruCache (max 100,000 IPs) instead of DashMap to
/// prevent memory exhaustion (OOM) under distributed botnet attacks.
async fn rate_limiter(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let limits =
        RATE_LIMITS.get_or_init(|| Mutex::new(LruCache::new(NonZeroUsize::new(100_000).unwrap())));

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

// -----------------------------------------------------------------------
// Router + Server
// -----------------------------------------------------------------------

/// Create the API router with all endpoints and middleware.
pub fn create_router(
    blockchain: Arc<RwLock<Blockchain>>,
    metrics: Option<Arc<crate::consensus::mempool::MetricsCollector>>,
    network: Option<Arc<crate::network::Network>>,
) -> Router {
    let state = Arc::new(ApiState {
        blockchain,
        metrics,
        network,
    });

    // Allow both localhost dev and the public block explorer origins.
    // Axum requires exact HeaderValue entries (no wildcard subdomains).
    let cors = CorsLayer::new()
        .allow_origin([
            "http://localhost:3000"
                .parse::<axum::http::HeaderValue>()
                .expect("valid CORS origin"),
            "https://scan.quantachain.org"
                .parse::<axum::http::HeaderValue>()
                .expect("valid CORS origin"),
            "https://www.scan.quantachain.org"
                .parse::<axum::http::HeaderValue>()
                .expect("valid CORS origin"),
        ])
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([axum::http::header::CONTENT_TYPE]);

    Router::new()
        // ── Health / Monitoring ──────────────────────────────────────────
        .route("/health", get(health_check))
        .route("/api/stats", get(get_stats))
        .route("/api/validate", get(validate_chain))
        .route("/api/peers", get(get_peers))
        .route("/api/validators", get(get_validators))
        .route("/api/metrics", get(get_metrics))
        // ── Blocks ──────────────────────────────────────────────────────
        .route("/api/block/:height", get(get_block))
        .route("/api/blocks/latest", get(get_latest_blocks))
        // ── Mempool ─────────────────────────────────────────────────────
        .route("/api/mempool", get(get_mempool))
        // ── Transactions ────────────────────────────────────────────────
        .route("/api/transactions/submit", post(submit_signed_transaction))
        .route("/api/tx/:hash", get(get_tx_handler))
        // ── Addresses / Balances ─────────────────────────────────────────
        // POST form kept for backward-compat with existing wallets
        .route("/api/balance", post(get_balance))
        // GET-style routes for block explorer deep-links
        .route("/api/balance/:address", get(get_balance_by_path))
        .route("/api/address/:address", get(get_address_info))
        .route("/api/address/:address/txs", get(get_address_transactions))
        // ── AI Contracts ────────────────────────────────────────────────
        .route("/api/contracts/:address", get(get_contract))
        .route("/api/contracts/:address/events", get(get_contract_events))
        .route("/api/agents", get(list_agents))
        .layer(
            ServiceBuilder::new()
                .layer(middleware::from_fn(rate_limiter))
                .layer(cors),
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

    tracing::info!(
        "QUANTA API server starting on {} (TLS={})",
        addr,
        tls_enabled
    );
    tracing::info!("Endpoints:");
    tracing::info!("   GET  /health                        - Health check");
    tracing::info!("   GET  /api/stats                     - Blockchain statistics");
    tracing::info!("   GET  /api/block/:height             - Block by height");
    tracing::info!("   GET  /api/blocks/latest?count=N     - Latest N blocks");
    tracing::info!("   GET  /api/tx/:hash                  - Transaction by hash");
    tracing::info!("   GET  /api/balance/:address          - Balance by address (GET)");
    tracing::info!("   POST /api/balance                   - Balance by address (POST, legacy)");
    tracing::info!("   GET  /api/address/:address          - Full address info + locked balances");
    tracing::info!("   GET  /api/address/:address/txs      - Address transaction history");
    tracing::info!("   GET  /api/mempool                   - Pending transactions");
    tracing::info!("   POST /api/transactions/submit       - Submit pre-signed transaction");
    tracing::info!("   GET  /api/mine/template?address=..  - Get block template for mining");
    tracing::info!("   POST /api/blocks/submit             - Submit solved block (pool use)");
    tracing::info!("   GET  /api/validate                  - Validate blockchain");
    tracing::info!("   GET  /api/peers                     - Connected peers");
    tracing::info!("   GET  /api/validators                - Registered validators");
    tracing::info!("   GET  /api/metrics                   - Prometheus metrics");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind server");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}

// ---------------------------------------------------------------------------
// AI Contract API Handlers
// ---------------------------------------------------------------------------

/// GET /api/contracts/:address
/// Returns the full contract state + event log. Powers QuaScan contract pages.
async fn get_contract(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    let blockchain = state.blockchain.read().await;
    let acc = blockchain.get_account_state_read();
    match acc.get_contract(&address) {
        Some(c) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "address":     address,
                "owner":       c.owner,
                "template_id": c.template_id,
                "deployed_at": c.deployed_at,
                "storage":     c.storage,
                "event_count": c.events.len(),
                "events":      c.events,
            })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Contract not found" })),
        )
            .into_response(),
    }
}

/// GET /api/contracts/:address/events
/// Returns only the event log - lightweight for QuaScan feeds.
async fn get_contract_events(
    State(state): State<Arc<ApiState>>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    let blockchain = state.blockchain.read().await;
    let acc = blockchain.get_account_state_read();
    match acc.get_contract(&address) {
        Some(c) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "address": address,
                "events":  c.events,
            })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Contract not found" })),
        )
            .into_response(),
    }
}

/// GET /api/agents?service_type=llm-inference
/// Lists all Agent Registry contracts, optionally filtered by service_type.
/// Powers the AI agent marketplace discovery page on QuaScan.
#[derive(Deserialize)]
struct AgentQuery {
    service_type: Option<String>,
}

async fn list_agents(
    State(state): State<Arc<ApiState>>,
    Query(q): Query<AgentQuery>,
) -> impl IntoResponse {
    let blockchain = state.blockchain.read().await;
    let acc = blockchain.get_account_state_read();
    let agents: Vec<serde_json::Value> = acc
        .contracts
        .iter()
        .filter(|(_, c)| c.template_id == crate::core::contracts::TEMPLATE_AGENT_REGISTRY)
        .filter(|(_, c)| {
            if let Some(ref stype) = q.service_type {
                c.storage
                    .get("service_type")
                    .map(|s| s == stype)
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .map(|(addr, c)| {
            serde_json::json!({
                "contract_address": addr,
                "agent_address":    c.storage.get("agent_address"),
                "name":             c.storage.get("name"),
                "service_type":     c.storage.get("service_type"),
                "price_per_call":   c.storage.get("price_per_call"),
                "endpoint_hash":    c.storage.get("endpoint_hash"),
                "active":           c.storage.get("active"),
                "registered_at":    c.storage.get("registered_at"),
            })
        })
        .collect();
    let count = agents.len();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "agents": agents, "count": count })),
    )
}
