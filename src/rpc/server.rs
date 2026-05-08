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
    pub mining_state: Arc<RwLock<Option<MiningState>>>,
    pub api_port: u16,
    pub network_port: u16,
    pub rpc_port: u16,
}

pub struct MiningState {
    pub address: String,
    pub is_active: bool,
    pub cancel_token: CancellationToken,
    pub blocks_mined: Arc<RwLock<u64>>,
}

#[derive(Clone)]
struct AppState {
    blockchain: Arc<RwLock<Blockchain>>,
    network: Option<Arc<Network>>,
    start_time: Arc<RwLock<Instant>>,
    mining_state: Arc<RwLock<Option<MiningState>>>,
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
            mining_state: Arc::new(RwLock::new(None)),
            api_port,
            network_port,
            rpc_port,
        }
    }

    pub async fn start(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let state = AppState {
            blockchain: self.blockchain,
            network: self.network,
            mining_state: self.mining_state,
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
        "start_mining" => handle_start_mining(&state, &request.params).await,
        "stop_mining" => handle_stop_mining(&state).await,
        "mining_status" => handle_mining_status(&state).await,
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

async fn handle_mining_status(state: &AppState) -> JsonRpcResponse {
    let blockchain = state.blockchain.read().await;
    let latest_block = blockchain.get_latest_block();
    let stats = blockchain.get_stats();
    drop(blockchain);

    let mining_state = state.mining_state.read().await;
    let is_mining = mining_state.as_ref().map(|m| m.is_active).unwrap_or(false);
    let mining_address = mining_state.as_ref().map(|m| m.address.clone());

    let mining_status = MiningStatus {
        is_mining,
        mining_address,
        last_block_time: Some(latest_block.timestamp),
        blocks_mined: stats.chain_length as u64,
        difficulty: stats.current_difficulty as u64,
        mining_reward: stats.mining_reward,
    };

    JsonRpcResponse::success(1, serde_json::to_value(mining_status).unwrap())
}

async fn handle_start_mining(state: &AppState, params: &serde_json::Value) -> JsonRpcResponse {
    let address = match params.get("address").and_then(|v| v.as_str()) {
        Some(addr) => addr.to_string(),
        None => {
            return JsonRpcResponse::error(
                1,
                -32602,
                "Invalid params: address required".to_string(),
            )
        }
    };

    let mut mining_state = state.mining_state.write().await;
    
    // Check if already mining
    if let Some(ref current) = *mining_state {
        if current.is_active {
            return JsonRpcResponse::error(
                1,
                -32000,
                format!("Mining already active for address: {}. Stop current mining first.", current.address),
            );
        }
    }

    // Create cancellation token for mining task
    let cancel_token = CancellationToken::new();
    let blocks_mined = Arc::new(RwLock::new(0u64));
    
    // Start mining
    *mining_state = Some(MiningState {
        address: address.clone(),
        is_active: true,
        cancel_token: cancel_token.clone(),
        blocks_mined: blocks_mined.clone(),
    });
    drop(mining_state);

    // Spawn mining task
    let blockchain = state.blockchain.clone();
    let mining_address = address.clone();
    let network = state.network.clone();
    
    tokio::spawn(async move {
        tracing::info!("Mining task started for address: {}", mining_address);

        // Subscribe to new-block notifications ONCE at task start.
        // Every time a block is accepted by the network layer, blockchain
        // fires this channel — we use it to abort stale PoW immediately.
        let mut new_block_rx = blockchain.read().await.subscribe_new_blocks();
        // Mark the current value as seen so the first `changed()` only fires
        // when a genuinely NEW block arrives (not the tip at startup).
        new_block_rx.borrow_and_update();

        let mut consecutive_failures = 0;
        const MAX_CONSECUTIVE_FAILURES: u32 = 10;

        loop {
            // Check if mining should stop
            if cancel_token.is_cancelled() {
                tracing::info!("Mining task stopped by cancellation");
                break;
            }

            // 1. Create template (brief read lock)
            let template_result = blockchain.read().await.create_block_template(mining_address.clone());

            match template_result {
                Ok(block) => {
                    // 2. Mine with cancellation support.
                    //    AtomicBool cancel flag shared between the async watcher
                    //    (which sets it when new_block_rx fires) and the blocking
                    //    PoW thread (which reads it every 10k hashes, ~10 ms).
                    let cancel_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
                    let cancel_flag_clone = cancel_flag.clone();
                    let cancel_token_inner = cancel_token.clone();

                    // Spawn blocking PoW on a dedicated thread (does NOT hold any lock).
                    let pow_handle = tokio::task::spawn_blocking(move || {
                        let mut b = block;
                        let found = b.mine_with_cancel(&cancel_flag_clone);
                        (b, found)
                    });

                    // Wait for EITHER: PoW completes  OR  a new block arrives.
                    let (mined_block, found_nonce) = tokio::select! {
                        // PoW finished (found nonce or was already cancelled)
                        res = pow_handle => {
                            match res {
                                Ok(pair) => pair,
                                Err(_) => {
                                    tracing::error!("Mining thread panicked");
                                    break;
                                }
                            }
                        }
                        // A new block arrived from the network — abort current PoW
                        _ = new_block_rx.changed() => {
                            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            tracing::info!(
                                "New network block detected -- aborting stale PoW, restarting with fresh template"
                            );
                            // The spawn_blocking thread will exit within ~10 ms;
                            // we don't await it to avoid blocking the async loop.
                            continue;
                        }
                        // Global stop requested
                        _ = cancel_token_inner.cancelled() => {
                            cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                            tracing::info!("Mining task stopped by cancellation (during PoW)");
                            break;
                        }
                    };

                    if !found_nonce {
                        // mine_with_cancel returned false = cancelled mid-work
                        continue;
                    }

                    // 3. Submit the solved block (brief read lock — all mutation
                    //    is done inside Blockchain via its internal Arc locks)
                    match blockchain.read().await.add_network_block(mined_block.clone()) {
                        Ok(_) => {
                            consecutive_failures = 0;
                            let mut count = blocks_mined.write().await;
                            *count += 1;
                            tracing::info!("Successfully mined block #{}", *count);

                            // Broadcast to peers
                            if let Some(ref net) = network {
                                net.broadcast_block(mined_block).await;
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to add mined block: {}", e);
                            consecutive_failures += 1;
                        }
                    }
                }
                Err(e) => {
                    consecutive_failures += 1;
                    tracing::warn!("Failed to create block template ({}): {}", consecutive_failures, e);

                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        tracing::error!(
                            "Mining failed {} times consecutively, stopping task",
                            MAX_CONSECUTIVE_FAILURES
                        );
                        break;
                    }

                    let backoff_ms = std::cmp::min(1000 * consecutive_failures as u64, 30000);
                    tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                    continue;
                }
            }

            // Tiny yield to let the async runtime breathe between attempts
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    });

    JsonRpcResponse::success(
        1,
        serde_json::json!({
            "message": "Mining started",
            "address": address
        }),
    )
}

async fn handle_stop_mining(state: &AppState) -> JsonRpcResponse {
    let mut mining_state = state.mining_state.write().await;
    
    if mining_state.is_none() {
        return JsonRpcResponse::error(
            1,
            -32000,
            "No active mining to stop".to_string(),
        );
    }

    // Cancel mining task
    if let Some(ref ms) = *mining_state {
        ms.cancel_token.cancel();
        let blocks = *ms.blocks_mined.read().await;
        tracing::info!("Mining stopped. Total blocks mined: {}", blocks);
    }
    
    *mining_state = None;

    JsonRpcResponse::success(
        1,
        serde_json::json!({
            "message": "Mining stopped"
        }),
    )
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
            difficulty: block.difficulty as u64,
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
