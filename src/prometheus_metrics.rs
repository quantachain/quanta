use prometheus::{
    Counter, Gauge, Histogram, HistogramOpts, Opts, Registry, TextEncoder, Encoder,
};
use axum::{routing::get, Router};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref REGISTRY: Registry = Registry::new();
    
    // Chain metrics
    pub static ref CHAIN_HEIGHT: Gauge = Gauge::with_opts(
        Opts::new("quanta_chain_height", "Current blockchain height")
    ).unwrap();
    
    pub static ref TOTAL_TRANSACTIONS: Counter = Counter::with_opts(
        Opts::new("quanta_total_transactions", "Total transactions processed")
    ).unwrap();
    
    pub static ref MEMPOOL_SIZE: Gauge = Gauge::with_opts(
        Opts::new("quanta_mempool_size", "Number of pending transactions")
    ).unwrap();
    
    pub static ref TOTAL_SUPPLY: Gauge = Gauge::with_opts(
        Opts::new("quanta_total_supply", "Total coin supply")
    ).unwrap();
    
    pub static ref DIFFICULTY: Gauge = Gauge::with_opts(
        Opts::new("quanta_difficulty", "Current mining difficulty")
    ).unwrap();
    
    // Mining metrics
    pub static ref BLOCKS_MINED: Counter = Counter::with_opts(
        Opts::new("quanta_blocks_mined", "Total blocks mined")
    ).unwrap();
    
    pub static ref MINING_REWARD: Gauge = Gauge::with_opts(
        Opts::new("quanta_mining_reward", "Current mining reward")
    ).unwrap();
    
    pub static ref BLOCK_MINING_TIME: Histogram = Histogram::with_opts(
        HistogramOpts::new("quanta_block_mining_time_seconds", "Time to mine a block")
    ).unwrap();
    
    // Network metrics
    pub static ref PEER_COUNT: Gauge = Gauge::with_opts(
        Opts::new("quanta_peer_count", "Number of connected peers")
    ).unwrap();
    
    pub static ref NETWORK_MESSAGES_SENT: Counter = Counter::with_opts(
        Opts::new("quanta_network_messages_sent_total", "Total network messages sent")
    ).unwrap();
    
    pub static ref NETWORK_MESSAGES_RECEIVED: Counter = Counter::with_opts(
        Opts::new("quanta_network_messages_received_total", "Total network messages received")
    ).unwrap();
    
    // Transaction metrics
    pub static ref TRANSACTIONS_VALIDATED: Counter = Counter::with_opts(
        Opts::new("quanta_transactions_validated_total", "Total transactions validated")
    ).unwrap();
    
    pub static ref TRANSACTIONS_REJECTED: Counter = Counter::with_opts(
        Opts::new("quanta_transactions_rejected_total", "Total transactions rejected")
    ).unwrap();
    
    pub static ref TRANSACTION_FEES: Counter = Counter::with_opts(
        Opts::new("quanta_transaction_fees_total", "Total transaction fees collected")
    ).unwrap();
}

pub fn register_metrics() {
    REGISTRY.register(Box::new(CHAIN_HEIGHT.clone())).ok();
    REGISTRY.register(Box::new(TOTAL_TRANSACTIONS.clone())).ok();
    REGISTRY.register(Box::new(MEMPOOL_SIZE.clone())).ok();
    REGISTRY.register(Box::new(TOTAL_SUPPLY.clone())).ok();
    REGISTRY.register(Box::new(DIFFICULTY.clone())).ok();
    REGISTRY.register(Box::new(BLOCKS_MINED.clone())).ok();
    REGISTRY.register(Box::new(MINING_REWARD.clone())).ok();
    REGISTRY.register(Box::new(BLOCK_MINING_TIME.clone())).ok();
    REGISTRY.register(Box::new(PEER_COUNT.clone())).ok();
    REGISTRY.register(Box::new(NETWORK_MESSAGES_SENT.clone())).ok();
    REGISTRY.register(Box::new(NETWORK_MESSAGES_RECEIVED.clone())).ok();
    REGISTRY.register(Box::new(TRANSACTIONS_VALIDATED.clone())).ok();
    REGISTRY.register(Box::new(TRANSACTIONS_REJECTED.clone())).ok();
    REGISTRY.register(Box::new(TRANSACTION_FEES.clone())).ok();
}

/// Export metrics in Prometheus format
async fn metrics_handler() -> String {
    let encoder = TextEncoder::new();
    let metric_families = REGISTRY.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}

/// Create metrics server router
pub fn create_metrics_router() -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
}

/// Start Prometheus metrics server
pub async fn start_metrics_server(port: u16) {
    register_metrics();
    
    let app = create_metrics_router();
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("📊 Prometheus metrics server starting on {}", addr);
    tracing::info!("   Endpoint: http://localhost:{}/metrics", port);
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind metrics server");
    
    axum::serve(listener, app)
        .await
        .expect("Metrics server error");
}

/// Update blockchain metrics
pub fn update_blockchain_metrics(
    height: u64,
    total_txs: usize,
    mempool: usize,
    supply: f64,
    difficulty: u32,
    reward: f64,
) {
    CHAIN_HEIGHT.set(height as f64);
    TOTAL_TRANSACTIONS.inc_by(total_txs as f64);
    MEMPOOL_SIZE.set(mempool as f64);
    TOTAL_SUPPLY.set(supply);
    DIFFICULTY.set(difficulty as f64);
    MINING_REWARD.set(reward);
}

/// Update network metrics
pub fn update_network_metrics(peers: usize) {
    PEER_COUNT.set(peers as f64);
}

/// Record transaction validation
pub fn record_transaction_validation(accepted: bool, fee: f64) {
    if accepted {
        TRANSACTIONS_VALIDATED.inc();
        TRANSACTION_FEES.inc_by(fee);
    } else {
        TRANSACTIONS_REJECTED.inc();
    }
}

/// Record block mined
pub fn record_block_mined(mining_time_secs: f64) {
    BLOCKS_MINED.inc();
    BLOCK_MINING_TIME.observe(mining_time_secs);
}

/// Record network message
pub fn record_network_message(sent: bool) {
    if sent {
        NETWORK_MESSAGES_SENT.inc();
    } else {
        NETWORK_MESSAGES_RECEIVED.inc();
    }
}
