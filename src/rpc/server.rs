use super::types::*;
use crate::consensus::Blockchain;
use crate::network::Network;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::post,
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

pub struct RpcServer {
    pub blockchain: Arc<RwLock<Blockchain>>,
    pub network: Option<Arc<Network>>,
    pub start_time: Arc<RwLock<Instant>>,

    pub api_port: u16,
    pub network_port: u16,
    pub rpc_port: u16,
}



#[derive(Clone)]
struct AppState {
    blockchain: Arc<RwLock<Blockchain>>,
    network: Option<Arc<Network>>,
    start_time: Arc<RwLock<Instant>>,

    api_port: u16,
    network_port: u16,
    rpc_port: u16,
}

impl RpcServer {
    pub fn new(
        blockchain: Arc<RwLock<Blockchain>>,
        network: Option<Arc<Network>>,
        api_port: u16,
        network_port: u16,
        rpc_port: u16,
    ) -> Self {
        Self {
            blockchain,
            network,
            start_time: Arc::new(RwLock::new(Instant::now())),

            api_port,
            network_port,
            rpc_port,
        }
    }

    pub async fn start(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let state = AppState {
            blockchain: self.blockchain,
            network: self.network,

            start_time: self.start_time,
            api_port: self.api_port,
            network_port: self.network_port,
            rpc_port: self.rpc_port,
        };

        let app = Router::new()
            .route("/", post(handle_rpc_request))
            .with_state(state);

        let addr = format!("127.0.0.1:{}", port);
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        
        tracing::info!("RPC server listening on {}", addr);
        
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn handle_rpc_request(
    State(state): State<AppState>,
    Json(request): Json<JsonRpcRequest>,
) -> (StatusCode, Json<JsonRpcResponse>) {
    tracing::debug!("RPC request: method={}, id={}", request.method, request.id);

    let response = match request.method.as_str() {
        "node_status" => handle_node_status(&state).await,

        "get_block" => handle_get_block(&state, &request.params).await,
        "get_balance" => handle_get_balance(&state, &request.params).await,
        "get_peers" => handle_get_peers(&state).await,
        "get_mempool" => handle_get_mempool(&state).await,
        "shutdown" => handle_shutdown(&state).await,
        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    };

    (StatusCode::OK, Json(response))
}

async fn handle_node_status(state: &AppState) -> JsonRpcResponse {
    let blockchain = state.blockchain.read().await;
    let chain_height = blockchain.get_height();
    let mempool_size = blockchain.get_pending_transactions().len();
    drop(blockchain);

    let peer_count = if let Some(ref network) = state.network {
        network.peer_count().await
    } else {
        0
    };

    let start_time = state.start_time.read().await;
    let uptime = start_time.elapsed().as_secs();

    let status = NodeStatus {
        running: true,
        chain_height,
        peer_count,
        mempool_size,
        api_port: state.api_port,
        network_port: state.network_port,
        rpc_port: state.rpc_port,
        uptime_seconds: uptime,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    JsonRpcResponse::success(1, serde_json::to_value(status).unwrap())
}


async fn handle_get_block(state: &AppState, params: &serde_json::Value) -> JsonRpcResponse {
    let height: u64 = match params.get("height").and_then(|v| v.as_u64()) {
        Some(h) => h,
        None => {
            return JsonRpcResponse::error(
                1,
                -32602,
                "Invalid params: height required".to_string(),
            )
        }
    };

    let blockchain = state.blockchain.read().await;
    
    if let Some(block) = blockchain.get_block_by_height(height) {
        let block_info = BlockInfo {
            height: block.index,
            hash: block.hash.clone(),
            timestamp: block.timestamp,
            transactions: block.transactions.len(),
            bft_round: block.bft_round,
            epoch: block.epoch,
            proposer: block.proposer.clone(),
            sig_count: block.bft_signatures.len(),
        };
        JsonRpcResponse::success(1, serde_json::to_value(block_info).unwrap())
    } else {
        JsonRpcResponse::error(1, -32000, format!("Block not found at height {}", height))
    }
}

async fn handle_get_balance(state: &AppState, params: &serde_json::Value) -> JsonRpcResponse {
    let address = match params.get("address").and_then(|v| v.as_str()) {
        Some(addr) => addr,
        None => {
            return JsonRpcResponse::error(
                1,
                -32602,
                "Invalid params: address required".to_string(),
            )
        }
    };

    let blockchain = state.blockchain.read().await;
    let balance = blockchain.get_balance(address);

    JsonRpcResponse::success(
        1,
        serde_json::json!({
            "address": address,
            "balance": balance,
            "balance_qua": balance as f64 / 1_000_000.0
        }),
    )
}

async fn handle_get_peers(state: &AppState) -> JsonRpcResponse {
    if let Some(ref network) = state.network {
        let peers = network.get_peers_info().await;
        let peer_infos: Vec<PeerInfo> = peers
            .iter()
            .map(|p| PeerInfo {
                address: p.address.to_string(),
                connected_since: p.connected_at,
                last_seen: p.last_seen,
            })
            .collect();
        JsonRpcResponse::success(1, serde_json::to_value(peer_infos).unwrap())
    } else {
        JsonRpcResponse::success(1, serde_json::json!([]))
    }
}

async fn handle_get_mempool(state: &AppState) -> JsonRpcResponse {
    let blockchain = state.blockchain.read().await;
    let transactions = blockchain.get_pending_transactions();
    
    let tx_data: Vec<serde_json::Value> = transactions
        .iter()
        .map(|tx| {
            serde_json::json!({
                "sender": tx.sender,
                "recipient": tx.recipient,
                "amount": tx.amount,
                "fee": tx.fee,
                "nonce": tx.nonce,
                "timestamp": tx.timestamp,
            })
        })
        .collect();

    JsonRpcResponse::success(1, serde_json::json!({ "transactions": tx_data }))
}

async fn handle_shutdown(_state: &AppState) -> JsonRpcResponse {
    tracing::info!("Shutdown requested via RPC");
    
    // Spawn a task to shutdown after a brief delay
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        std::process::exit(0);
    });

    JsonRpcResponse::success(1, serde_json::json!({ "message": "Shutting down..." }))
}
