use axum::{
    extract::{State, Json},
    routing::{get, post},
    Router, http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::blockchain::{Blockchain, BlockchainStats};
use crate::transaction::Transaction;
use crate::secure_wallet::SecureWallet;

/// API state
pub struct ApiState {
    pub blockchain: Arc<Blockchain>,
}

/// Request to create a transaction
#[derive(Deserialize)]
pub struct CreateTransactionRequest {
    pub wallet_file: String,
    pub wallet_password: String,
    pub recipient: String,
    pub amount: f64,
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
    Json(state.blockchain.get_stats())
}

/// Get balance for an address
#[derive(Deserialize)]
pub struct BalanceRequest {
    pub address: String,
}

#[derive(Serialize)]
pub struct BalanceResponse {
    pub address: String,
    pub balance: f64,
}

async fn get_balance(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<BalanceRequest>,
) -> Json<BalanceResponse> {
    let balance = state.blockchain.get_balance(&req.address);
    Json(BalanceResponse {
        address: req.address,
        balance,
    })
}

/// Create and submit a transaction
async fn create_transaction(
    State(state): State<Arc<ApiState>>,
    Json(req): Json<CreateTransactionRequest>,
) -> (StatusCode, Json<TransactionResponse>) {
    // Load wallet
    let wallet = match SecureWallet::load_encrypted(&req.wallet_file, &req.wallet_password) {
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

    // Create transaction
    let tx = Transaction::new(
        wallet.address.clone(),
        req.recipient,
        req.amount,
        chrono::Utc::now().timestamp(),
    );

    // Sign transaction
    let signing_data = tx.get_signing_data();
    let signature = wallet.keypair.sign(&signing_data);
    
    let mut signed_tx = tx;
    signed_tx.signature = signature;
    signed_tx.public_key = wallet.keypair.public_key.clone();

    // Submit to blockchain
    match state.blockchain.add_transaction(signed_tx.clone()) {
        Ok(_) => {
            let tx_hash = signed_tx.hash();
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
    match state.blockchain.mine_pending_transactions(req.miner_address) {
        Ok(_) => {
            let stats = state.blockchain.get_stats();
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

/// Validate blockchain
#[derive(Serialize)]
pub struct ValidateResponse {
    pub is_valid: bool,
}

async fn validate_chain(
    State(state): State<Arc<ApiState>>,
) -> Json<ValidateResponse> {
    Json(ValidateResponse {
        is_valid: state.blockchain.is_valid(),
    })
}

/// Create the API router
pub fn create_router(blockchain: Arc<Blockchain>) -> Router {
    let state = Arc::new(ApiState { blockchain });

    Router::new()
        .route("/api/stats", get(get_stats))
        .route("/api/balance", post(get_balance))
        .route("/api/transaction", post(create_transaction))
        .route("/api/mine", post(mine_block))
        .route("/api/validate", get(validate_chain))
        .with_state(state)
}

/// Start the API server
pub async fn start_server(blockchain: Arc<Blockchain>, port: u16) {
    let app = create_router(blockchain);
    let addr = format!("0.0.0.0:{}", port);
    
    tracing::info!("🚀 QUANTA API server starting on {}", addr);
    tracing::info!("📡 Endpoints:");
    tracing::info!("   GET  /api/stats - Get blockchain statistics");
    tracing::info!("   POST /api/balance - Get address balance");
    tracing::info!("   POST /api/transaction - Create transaction");
    tracing::info!("   POST /api/mine - Mine a block");
    tracing::info!("   GET  /api/validate - Validate blockchain");
    
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind server");
    
    axum::serve(listener, app)
        .await
        .expect("Server error");
}
