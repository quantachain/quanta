mod core;
mod consensus;
mod crypto;
mod storage;
mod network;
mod api;
mod config;
mod rpc;



use consensus::Blockchain;
use crypto::QuantumWallet;
use storage::BlockchainStorage;
use network::{Network, NetworkConfig};
use consensus::MetricsCollector;
use config::QuantaConfig;
use rpc::{RpcServer, RpcClient};
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber;

// CONSENSUS CONSTANTS: 1 QUA = 1_000_000 microunits
const MICROUNITS_PER_QUA: u64 = 1_000_000;

/// Convert QUA (f64 for CLI UX) to microunits (u64 for consensus)
fn qua_to_microunits(qua: f64) -> u64 {
    (qua * MICROUNITS_PER_QUA as f64) as u64
}

/// Convert microunits (u64) to QUA (f64 for display)
fn microunits_to_qua(microunits: u64) -> f64 {
    microunits as f64 / MICROUNITS_PER_QUA as f64
}

#[derive(Parser)]
#[command(name = "quanta")]
#[command(about = "QUANTA - Quantum-Resistant Blockchain with Falcon Signatures", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "snake_case")]
enum Commands {
    /// Start the blockchain node with REST API, RPC, and P2P networking
    Start {
        /// Configuration file path
        #[arg(short = 'c', long)]
        config: Option<String>,
        
        /// Network type (mainnet or testnet)
        #[arg(long, default_value = "mainnet")]
        network: String,
        
        /// API server port (overrides config)
        #[arg(short, long)]
        port: Option<u16>,
        
        /// P2P network port (overrides config)
        #[arg(short = 'n', long = "network-port")]
        network_port: Option<u16>,
        
        /// RPC server port (overrides config)
        #[arg(short = 'r', long = "rpc-port")]
        rpc_port: Option<u16>,

        /// Database path (overrides config)
        #[arg(short, long)]
        db: Option<String>,
        
        /// Bootstrap peer addresses (comma-separated host:port)
        #[arg(short = 'b', long)]
        bootstrap: Option<String>,
        
        /// Disable P2P networking (single node mode)
        #[arg(long = "no-network")]
        no_network: bool,
        
        /// Run in background as daemon
        #[arg(long)]
        detach: bool,
    },
    
    /// Check node status (requires running node)
    Status {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Check mining status (requires running node)
    MiningStatus {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Start mining blocks to a specific address (requires running node)
    StartMining {
        /// Address to receive mining rewards
        address: String,
        
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Stop mining (requires running node)
    StopMining {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Print current blockchain height (requires running node)
    PrintHeight {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Get information about a specific block (requires running node)
    GetBlock {
        /// Block height
        height: u64,
        
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Get list of connected peers (requires running node)
    Peers {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Stop the running node gracefully (requires running node)
    Stop {
        /// RPC port (default: 7782)
        #[arg(short = 'r', long = "rpc-port", default_value = "7782")]
        rpc_port: u16,
    },
    
    /// Create a new encrypted wallet
    NewWallet {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },
    
    /// Create a new HD wallet with 24-word mnemonic
    NewHdWallet {
        /// Wallet file name
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        
        /// Number of accounts to generate
        #[arg(short, long, default_value = "3")]
        accounts: u32,
    },
    
    /// Show HD wallet information
    HdWallet {
        /// Wallet file name
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
    },
    
    /// Show wallet information
    Wallet {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
        
        /// Network type (mainnet or testnet)
        #[arg(long, default_value = "mainnet")]
        network: String,
        
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Show wallet address only (no balance check)
    WalletAddress {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },
    
    /// Mine a new block
    Mine {
        /// Miner wallet file
        #[arg(short, long, default_value = "wallet.qua")]
        wallet: String,
        
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Send coins to another address
    Send {
        /// Sender wallet file
        #[arg(short, long, default_value = "wallet.qua")]
        wallet: String,
        /// Recipient address
        #[arg(short, long)]
        to: String,
        /// Amount to send
        #[arg(short, long)]
        amount: f64,
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Show blockchain statistics
    Stats {
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Validate the blockchain
    Validate {
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Run demo with sample transactions
    Demo {
        /// Database path
        #[arg(short, long, default_value = "./quanta_demo")]
        db: String,
    },
}

#[tokio::main]
async fn main() {
    println!("");
    println!("        QUANTA - Quantum-Resistant Blockchain                  ");
    println!("         Falcon Signatures | Post-Quantum Cryptography         ");
    println!("\n");

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config, network, port, network_port, rpc_port, db, bootstrap, no_network, detach } => {
            // Load configuration with RPC port override
            let mut cfg = QuantaConfig::load_with_overrides(
                config,
                port,
                network_port,
                db.clone(),
                bootstrap.clone(),
                Some(network),
                no_network
            ).expect("Failed to load configuration");
            
            // Set RPC port from CLI or default
            let rpc_port = rpc_port.unwrap_or(7782);
            
            // Setup logging based on detach mode
            if detach {
                // Fork to background on Unix-like systems
                #[cfg(unix)]
                {
                    use std::process::Command;
                    
                    // Check if already running as daemon
                    if std::env::var("QUANTA_DAEMON").is_err() {
                        println!("Starting QUANTA node in daemon mode...");
                        
                        // Create logs directory
                        std::fs::create_dir_all("logs").expect("Failed to create logs directory");
                        
                        // Prepare arguments (include --detach to trigger file logging in child)
                        let mut args = vec![
                            "start".to_string(),
                            "--detach".to_string(),
                            "--port".to_string(),
                            cfg.node.api_port.to_string(),
                            "--network-port".to_string(),
                            cfg.node.network_port.to_string(),
                            "--rpc-port".to_string(),
                            rpc_port.to_string(),
                            "--db".to_string(),
                            cfg.node.db_path.clone(),
                        ];
                        
                        if cfg.node.no_network {
                            args.push("--no-network".to_string());
                        }
                        
                        // Open log file for the child process
                        let log_file_path = format!("logs/quanta_node_{}.log", cfg.node.network_port);
                        let log_file = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&log_file_path)
                            .expect("Failed to open log file");
                        
                        // Fork process with proper file descriptors
                        let child = Command::new(std::env::current_exe().unwrap())
                            .args(&args)
                            .env("QUANTA_DAEMON", "1")
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::from(log_file.try_clone().unwrap()))
                            .stderr(std::process::Stdio::from(log_file))
                            .spawn()
                            .expect("Failed to start daemon process");
                        
                        let child_pid = child.id();
                        
                        // Write PID file
                        let pid_file = format!("logs/quanta_{}.pid", cfg.node.network_port);
                        std::fs::write(&pid_file, child_pid.to_string())
                            .expect("Failed to write PID file");
                        
                        println!(" QUANTA node started as daemon (PID: {})", child_pid);
                        println!("  API Port: {}", cfg.node.api_port);
                        println!("  Network Port: {}", cfg.node.network_port);
                        println!("  RPC Port: {}", rpc_port);
                        println!("  Database: {}", cfg.node.db_path);
                        println!("  Log file: {}", log_file_path);
                        println!("  PID file: {}", pid_file);
                        println!("\nWaiting for node to initialize...");
                        std::thread::sleep(std::time::Duration::from_secs(3));
                        println!("\nUse './quanta status --rpc-port {}' to check node status", rpc_port);
                        println!("Use './quanta stop --rpc-port {}' to stop the node", rpc_port);
                        
                        return;
                    }
                }
                
                #[cfg(not(unix))]
                {
                    eprintln!("Warning: Daemon mode is not fully supported on Windows. Running in foreground with file logging.");
                }
                
                // Setup file-based logging for daemon
                std::fs::create_dir_all("logs").expect("Failed to create logs directory");
                
                let log_file = format!("logs/quanta_node_{}.log", cfg.node.network_port);
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file)
                    .expect("Failed to open log file");
                
                tracing_subscriber::fmt()
                    .with_writer(Arc::new(file))
                    .with_ansi(false)
                    .with_target(false)
                    .with_level(true)
                    .init();
                
                // Write PID file
                let pid_file = format!("logs/quanta_{}.pid", cfg.node.network_port);
                std::fs::write(&pid_file, std::process::id().to_string())
                    .expect("Failed to write PID file");
                
                tracing::info!("Starting QUANTA node in DAEMON mode");
                tracing::info!("Log file: {}", log_file);
                tracing::info!("PID file: {}", pid_file);
            } else {
                // Initialize console logging for non-detached mode
                tracing_subscriber::fmt()
                    .with_target(false)
                    .with_level(true)
                    .init();
            }
            
            tracing::info!("Starting QUANTA node with configuration:");
            tracing::info!("  API Port: {}", cfg.node.api_port);
            tracing::info!("  Network Port: {}", cfg.node.network_port);
            tracing::info!("  RPC Port: {}", rpc_port);
            tracing::info!("  Database: {}", cfg.node.db_path);
            
            let storage = Arc::new(BlockchainStorage::new(&cfg.node.db_path).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, cfg.network_type).expect("Failed to initialize blockchain")));
            
            let metrics = Arc::new(MetricsCollector::new());
            
            // Start Prometheus metrics server if enabled
            if cfg.metrics.enabled {
                let _metrics_port = cfg.metrics.port;
                tokio::spawn(async move {
                // Metrics server removed - add back when needed
                });
            }
            
            let network = if !cfg.node.no_network {
                // Parse bootstrap nodes
                let bootstrap_nodes: Vec<std::net::SocketAddr> = cfg.network.bootstrap_nodes
                    .iter()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                
                let listen_addr = format!("0.0.0.0:{}", cfg.node.network_port).parse().unwrap();
                
                let network_config = NetworkConfig {
                    listen_addr,
                    max_peers: cfg.network.max_peers,
                    node_id: uuid::Uuid::new_v4().to_string(),
                    bootstrap_nodes,
                    dns_seeds: cfg.network.dns_seeds.clone(),
                };
                
                let network = Arc::new(Network::new(network_config, Arc::clone(&blockchain)));
                
                // Start P2P network
                let network_clone = Arc::clone(&network);
                tokio::spawn(async move {
                    if let Err(e) = network_clone.start().await {
                        tracing::error!("Network error: {}", e);
                    }
                });
                
                // Start blockchain sync
                let network_clone = Arc::clone(&network);
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    if let Err(e) = network_clone.sync_blockchain().await {
                        tracing::error!("Sync error: {}", e);
                    }
                });  
                
                println!("P2P Network started on port {}", cfg.node.network_port);
                Some(network)
            } else {
                println!("Running in single-node mode (P2P disabled)");
                None
            };
            
            // Start metrics updater
            let metrics_clone = Arc::clone(&metrics);
            let blockchain_clone = Arc::clone(&blockchain);
            let network_clone = network.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
                loop {
                    interval.tick().await;
                    let blockchain = blockchain_clone.read().await;
                    let height = blockchain.get_height();
                    let mempool_size = blockchain.get_pending_transactions().len();
                    let last_block = blockchain.get_latest_block();
                    drop(blockchain);
                    
                    metrics_clone.update_blockchain_stats(height, mempool_size, Some(last_block.timestamp)).await;
                    
                    if let Some(ref net) = network_clone {
                        let peer_count = net.peer_count().await;
                        metrics_clone.update_peer_count(peer_count).await;
                    }
                }
            });
            
            // Setup graceful shutdown
            let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
            
            // Handle Ctrl+C
            tokio::spawn(async move {
                tokio::signal::ctrl_c()
                    .await
                    .expect("Failed to listen for Ctrl+C");
                tracing::info!("Shutdown signal received, stopping node...");
                let _ = shutdown_tx.send(()).await;
            });
            
            // Start RPC server
            let rpc_server = RpcServer::new(
                Arc::clone(&blockchain),
                network.clone(),
                cfg.node.api_port,
                cfg.node.network_port,
                rpc_port,
            );
            
            let rpc_handle = {
                let rpc_port_clone = rpc_port;
                tokio::spawn(async move {
                    if let Err(e) = rpc_server.start(rpc_port_clone).await {
                        tracing::error!("RPC server error: {}", e);
                    }
                })
            };
            
            // Start API server
            let server_handle = {
                let blockchain_clone = Arc::clone(&blockchain);
                let metrics_clone = Some(metrics.clone());
                let network_clone = network.clone();
                let port = cfg.node.api_port;
                tokio::spawn(async move {
                    api::start_server(blockchain_clone, port, metrics_clone, network_clone).await;
                })
            };
            
            // Wait for shutdown signal or server exit
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    tracing::info!("Gracefully shutting down...");
                    
                    // Save final state
                    let blockchain_lock = blockchain.read().await;
                    let chain_height = blockchain_lock.get_height();
                    tracing::info!("Final chain height: {}", chain_height);
                    drop(blockchain_lock);
                    
                    tracing::info!("Node stopped successfully");
                }
                _ = server_handle => {
                    tracing::info!("Server stopped");
                }
                _ = rpc_handle => {
                    tracing::info!("RPC server stopped");
                }
            }
        }
        
        Commands::Status { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            match client.get_node_status().await {
                Ok(status) => {
                    println!("\n");
                    println!("            QUANTA NODE STATUS                            ");
                    println!("");
                    println!("  Status:         {}                              ", if status.running { "RUNNING " } else { "STOPPED" });
                    println!("  Version:        {}                                     ", status.version);
                    println!("  Uptime:         {} seconds                             ", status.uptime_seconds);
                    println!("                                                          ");
                    println!("  Chain Height:   {} blocks                              ", status.chain_height);
                    println!("  Mempool:        {} pending transactions                ", status.mempool_size);
                    println!("  Peers:          {} connected                           ", status.peer_count);
                    println!("                                                          ");
                    println!("  API Port:       {}                                     ", status.api_port);
                    println!("  Network Port:   {}                                     ", status.network_port);
                    println!("  RPC Port:       {}                                     ", status.rpc_port);
                    println!("\n");
                }
                Err(e) => {
                    eprintln!(" Failed to connect to node RPC server on port {}", rpc_port);
                    eprintln!("  Error: {}", e);
                    eprintln!("\n  Is the node running? Start it with:");
                    eprintln!("  ./quanta start --detach --rpc-port {}", rpc_port);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::MiningStatus { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            match client.get_mining_status().await {
                Ok(status) => {
                    println!("\n");
                    println!("            QUANTA MINING STATUS                          ");
                    println!("");
                    println!("  Mining Active:  {}                              ", if status.is_mining { "YES " } else { "NO" });
                    if let Some(ref addr) = status.mining_address {
                        println!("  Mining To:      {}...", &addr[..32]);
                    }
                    println!("  Blocks Mined:   {}                                      ", status.blocks_mined);
                    println!("  Difficulty:     {}                                      ", status.difficulty);
                    println!("  Block Reward:   {} microunits ({:.6} QUA)         ", 
                        status.mining_reward, 
                        status.mining_reward as f64 / 1_000_000.0
                    );
                    if let Some(last_time) = status.last_block_time {
                        use chrono::{DateTime, Utc as ChronoUtc};
                        let dt = DateTime::<ChronoUtc>::from_timestamp(last_time, 0)
                            .unwrap_or_else(|| ChronoUtc::now());
                        println!("  Last Block:     {}                        ", dt.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                    println!("\n");
                }
                Err(e) => {
                    eprintln!(" Failed to get mining status: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::GetBlock { height, rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            match client.get_block(height).await {
                Ok(block) => {
                    use chrono::{DateTime, Utc as ChronoUtc};
                    let dt = DateTime::<ChronoUtc>::from_timestamp(block.timestamp, 0)
                        .unwrap_or_else(|| ChronoUtc::now());
                    
                    println!("\n");
                    println!("            BLOCK INFORMATION                             ");
                    println!("");
                    println!("  Height:         {}                                      ", block.height);
                    println!("  Hash:           {}...", &block.hash[..24]);
                    println!("  Timestamp:      {}                        ", dt.format("%Y-%m-%d %H:%M:%S UTC"));
                    println!("  Transactions:   {}                                      ", block.transactions);
                    println!("  Difficulty:     {}                                      ", block.difficulty);
                    println!("\n");
                }
                Err(e) => {
                    eprintln!(" Failed to get block: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::Peers { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            match client.get_peers().await {
                Ok(peers) => {
                    println!("\n");
                    println!("            CONNECTED PEERS ({} total)                      ", peers.len());
                    println!("");
                    
                    if peers.is_empty() {
                        println!("  No peers connected                                      ");
                    } else {
                        for (i, peer) in peers.iter().enumerate() {
                            println!("  {}. {}                                    ", i + 1, peer.address);
                        }
                    }
                    
                    println!("\n");
                }
                Err(e) => {
                    eprintln!(" Failed to get peers: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::StartMining { address, rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            println!("Starting mining to address: {}", address);
            
            match client.start_mining(&address).await {
                Ok(_) => {
                    println!(" Mining started successfully");
                    println!("  Rewards will be sent to: {}", address);
                    println!("  Use './quanta mining_status' to check status");
                }
                Err(e) => {
                    eprintln!(" Failed to start mining: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::StopMining { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            println!("Stopping mining...");
            
            match client.stop_mining().await {
                Ok(_) => {
                    println!(" Mining stopped successfully");
                }
                Err(e) => {
                    eprintln!(" Failed to stop mining: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::PrintHeight { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            match client.get_node_status().await {
                Ok(status) => {
                    println!("\n");
                    println!("            BLOCKCHAIN HEIGHT                             ");
                    println!("");
                    println!("  Current Height: {} blocks                              ", status.chain_height);
                    println!("\n");
                }
                Err(e) => {
                    eprintln!(" Failed to get blockchain height: {}", e);
                    std::process::exit(1);
                }
            }
        }
        
        Commands::Stop { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            
            println!("Sending shutdown signal to node on RPC port {}...", rpc_port);
            
            match client.shutdown().await {
                Ok(_) => {
                    println!(" Shutdown signal sent successfully");
                    println!("  Node will stop gracefully...");
                }
                Err(e) => {
                    eprintln!(" Failed to send shutdown signal: {}", e);
                    eprintln!("\n  Is the node running? Check with:");
                    eprintln!("  ./quanta status --rpc-port {}", rpc_port);
                    std::process::exit(1);
                }
            }
        }

        Commands::NewWallet { file } => {
            // Initialize console logging for non-start commands
            tracing_subscriber::fmt()
                .with_target(false)
                .with_level(true)
                .try_init()
                .ok();
            
            let wallet = QuantumWallet::new();
            
            let password = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("\nEnter password to encrypt wallet:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            let password_confirm = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("Confirm password:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            if password != password_confirm {
                eprintln!("Passwords don't match!");
                return;
            }
            
            wallet.save_quantum_safe(&file, &password).expect("Failed to save wallet");
            println!("Wallet created and encrypted successfully!");
        }

        Commands::NewHdWallet { file, accounts } => {
            use crate::crypto::HDWallet;
            
            let mut wallet = HDWallet::new();
            
            // Generate requested number of accounts
            for i in 0..accounts {
                wallet.generate_account(Some(format!("Account {}", i)));
            }
            
            wallet.display_info();
            
            println!("\nEnter password to encrypt wallet:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            println!("Confirm password:");
            let password_confirm = rpassword::read_password().expect("Failed to read password");
            
            if password != password_confirm {
                eprintln!("Passwords don't match!");
                return;
            }
            
            // Save encrypted wallet
            let encrypted = wallet.export_encrypted(&password).expect("Failed to encrypt wallet");
            std::fs::write(&file, encrypted).expect("Failed to save wallet");
            
            println!("\n HD Wallet created and encrypted successfully!");
            println!(" Saved to: {}", file);
            println!("\n  CRITICAL: Write down your 24-word mnemonic phrase!");
            println!("   This is the ONLY way to recover your wallet.");
        }

        Commands::HdWallet { file } => {
            println!("Enter wallet password:");
            let _password = rpassword::read_password().expect("Failed to read password");
            
            // For now, we'll need to implement proper loading
            println!("HD wallet info display - implementation needed for encrypted load");
            println!("Wallet file: {}", file);
        }

        Commands::Wallet { file, network, db } => {
            let password = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("Enter wallet password:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            let wallet = match QuantumWallet::load_quantum_safe(&file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to load wallet: {}", e);
                    return;
                }
            };
            
            // Determine network type
            let network_type = match network.as_str() {
                "testnet" => core::ChainNetwork::Testnet,
                _ => core::ChainNetwork::Mainnet,
            };
            
            // Load blockchain to get balance
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, network_type).expect("Failed to initialize blockchain")));
            let balance_microunits = blockchain.read().await.get_balance(&wallet.address);
            
            wallet.display_info(microunits_to_qua(balance_microunits));
        }

        Commands::WalletAddress { file } => {
            let password = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("Enter wallet password:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            let wallet = match QuantumWallet::load_quantum_safe(&file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to load wallet: {}", e);
                    return;
                }
            };
            
            println!("\nWallet Address: {}\n", wallet.address);
        }

        Commands::Mine { wallet: wallet_file, db } => {
            let password = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("Enter wallet password:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            let wallet = match QuantumWallet::load_quantum_safe(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!(" Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, core::ChainNetwork::Mainnet).expect("Failed to initialize blockchain")));
            
            println!("  Mining new block...");
            let mine_result = blockchain.write().await.mine_pending_transactions(wallet.address.clone());
            match mine_result {
                Ok(_) => {
                    println!(" Block mined successfully!");
                    let balance_microunits = blockchain.read().await.get_balance(&wallet.address);
                    println!(" New balance: {:.6} QUA", microunits_to_qua(balance_microunits));
                }
                Err(e) => eprintln!(" Mining failed: {}", e),
            }
        }

        Commands::Send { wallet: wallet_file, to, amount, db } => {
            let password = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") {
                p
            } else {
                println!("Enter wallet password:");
                rpassword::read_password().expect("Failed to read password")
            };
            
            let wallet = match QuantumWallet::load_quantum_safe(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!(" Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, core::ChainNetwork::Mainnet).expect("Failed to initialize blockchain")));
            
            // Convert QUA to microunits
            let amount_microunits = qua_to_microunits(amount);
            
            // Get current nonce for sender
            let current_nonce = {
                let bc = blockchain.read().await;
                bc.get_balance(&wallet.address); // Ensure account exists
                let nonce = bc.get_account_state_mut().get_nonce(&wallet.address);
                nonce
            };
            let next_nonce = current_nonce + 1;
            
            use crate::core::transaction::{Transaction, TransactionType};
            let mut tx = Transaction {
                sender: wallet.address.clone(),
                recipient: to.clone(),
                amount: amount_microunits,
                timestamp: Utc::now().timestamp(),
                signature: vec![],
                public_key: wallet.keypair.public_key.clone(),
                fee: 1000, // 0.001 QUA default fee
                nonce: next_nonce,
                tx_type: TransactionType::Transfer,
            };
            
            // Sign transaction
            let signing_data = tx.get_signing_data();
            tx.signature = wallet.keypair.sign(&signing_data);
            
            let add_result = blockchain.write().await.add_transaction(tx);
            match add_result {
                Ok(_) => {
                    println!(" Transaction added to mempool");
                    println!(" Sending {:.6} QUA to {}", amount, to);
                    println!(" Nonce: {}", next_nonce);
                }
                Err(e) => eprintln!(" Transaction failed: {}", e),
            }
        }

        Commands::Stats { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, core::ChainNetwork::Mainnet).expect("Failed to initialize blockchain")));
            let stats = blockchain.read().await.get_stats();
            
            let reward_qua = microunits_to_qua(stats.mining_reward);
            let supply_qua = microunits_to_qua(stats.total_supply);
            
            println!("");
            println!("                QUANTA BLOCKCHAIN STATISTICS                   ");
            println!("");
            println!(" Chain Length: {} blocks                                  ", stats.chain_length);
            println!(" Total Transactions: {}                                    ", stats.total_transactions);
            println!(" Current Difficulty: {}                                     ", stats.current_difficulty);
            println!(" Mining Reward: {:.6} QUA                                 ", reward_qua);
            println!(" Total Supply: {:.6} QUA                                  ", supply_qua);
            println!(" Pending Transactions: {}                                   ", stats.pending_transactions);
            println!("");
            println!(" Quantum Resistance: ACTIVE                                  ");
            println!(" Signature Algorithm: Falcon-512 (NIST PQC)                   ");
            println!(" Hash Algorithm: SHA3-256                                      ");
            println!(" Wallet Encryption: Kyber-1024 + ChaCha20-Poly1305            ");
            println!(" Persistent Storage: Sled Database                            ");
            println!(" Amount Precision: u64 microunits (deterministic)             ");
            println!("");
        }

        Commands::Validate { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, core::ChainNetwork::Mainnet).expect("Failed to initialize blockchain")));
            
            println!("Validating blockchain...");
            
            if blockchain.read().await.is_valid() {
                println!("Blockchain is VALID");
                println!("   All blocks verified");
                println!("   All Falcon signatures verified");
                println!("   Chain integrity maintained");
            } else {
                println!("Blockchain is INVALID");
            }
        }

        Commands::Demo { db } => {
            println!("Running Production Demo...\n");
            run_demo(&db).await;
        }
    }
}

async fn run_demo(db_path: &str) {
    use crate::core::transaction::{Transaction, TransactionType};
    let storage = Arc::new(BlockchainStorage::new(db_path).expect("Failed to open database"));
    
    // Clear old demo data
    storage.clear().expect("Failed to clear database");
    
    let blockchain = Arc::new(RwLock::new(Blockchain::new(storage, crate::core::ChainNetwork::Mainnet).expect("Failed to initialize blockchain")));
    
    // Create demo wallets
    println!(" Creating quantum-safe encrypted demo wallets...");
    let wallet1 = QuantumWallet::new();
    let wallet2 = QuantumWallet::new();
    let wallet3 = QuantumWallet::new();
    
    // WARNING: Insecure password for demo ONLY! Never use in production!
    const DEMO_PASSWORD: &str = "INSECURE_DEMO_PASSWORD_DO_NOT_USE_IN_PRODUCTION";
    println!("  Demo wallets use INSECURE password - FOR TESTING ONLY!");
    
    wallet1.save_quantum_safe("demo_wallet1.qua", DEMO_PASSWORD).unwrap();
    wallet2.save_quantum_safe("demo_wallet2.qua", DEMO_PASSWORD).unwrap();
    wallet3.save_quantum_safe("demo_wallet3.qua", DEMO_PASSWORD).unwrap();
    
    println!("\n  Mining genesis rewards...");
    blockchain.write().await.mine_pending_transactions(wallet1.address.clone()).unwrap();
    blockchain.write().await.mine_pending_transactions(wallet1.address.clone()).unwrap();
    
    println!("\n Creating transactions...");
    
    // Transaction 1: 25 QUA = 25_000_000 microunits
    let amount1_microunits = qua_to_microunits(25.0);
    let nonce1 = {
        let bc = blockchain.read().await;
        let nonce = bc.get_account_state_mut().get_nonce(&wallet1.address);
        nonce + 1
    };
    
    let mut tx1 = Transaction {
        sender: wallet1.address.clone(),
        recipient: wallet2.address.clone(),
        amount: amount1_microunits,
        timestamp: Utc::now().timestamp(),
        signature: vec![],
        public_key: wallet1.keypair.public_key.clone(),
        fee: 1000, // 0.001 QUA
        nonce: nonce1,
        tx_type: TransactionType::Transfer,
    };
    let signing_data1 = tx1.get_signing_data();
    tx1.signature = wallet1.keypair.sign(&signing_data1);
    blockchain.write().await.add_transaction(tx1).unwrap();
    println!("   Tx 1: 25 QUA to wallet2 (nonce {})", nonce1);
    
    println!("\n  Mining first transaction...");
    blockchain.write().await.mine_pending_transactions(wallet2.address.clone()).unwrap();
    
    // Transaction 2: 15 QUA = 15_000_000 microunits
    let amount2_microunits = qua_to_microunits(15.0);
    let nonce2 = {
        let bc = blockchain.read().await;
        let nonce = bc.get_account_state_mut().get_nonce(&wallet1.address);
        nonce + 1
    };
    
    let mut tx2 = Transaction {
        sender: wallet1.address.clone(),
        recipient: wallet3.address.clone(),
        amount: amount2_microunits,
        timestamp: Utc::now().timestamp(),
        signature: vec![],
        public_key: wallet1.keypair.public_key.clone(),
        fee: 1000,
        nonce: nonce2,
        tx_type: TransactionType::Transfer,
    };
    let signing_data2 = tx2.get_signing_data();
    tx2.signature = wallet1.keypair.sign(&signing_data2);
    blockchain.write().await.add_transaction(tx2).unwrap();
    println!("   Tx 2: 15 QUA to wallet3 (nonce {})", nonce2);
    
    println!("\n  Mining second transaction...");
    blockchain.write().await.mine_pending_transactions(wallet3.address.clone()).unwrap();
    
    // Show final balances
    println!("\n Final Balances:");
    let bc = blockchain.read().await;
    let bal1 = microunits_to_qua(bc.get_balance(&wallet1.address));
    let bal2 = microunits_to_qua(bc.get_balance(&wallet2.address));
    let bal3 = microunits_to_qua(bc.get_balance(&wallet3.address));
    println!("  Wallet 1: {:.6} QUA", bal1);
    println!("  Wallet 2: {:.6} QUA", bal2);
    println!("  Wallet 3: {:.6} QUA", bal3);
    
    // Show stats
    let stats = bc.get_stats();
    println!("\n Blockchain Stats:");
    println!("  Blocks: {}", stats.chain_length);
    println!("  Transactions: {}", stats.total_transactions);
    println!("  Total Supply: {:.6} QUA ({} microunits)", microunits_to_qua(stats.total_supply), stats.total_supply);
    println!("  Current Difficulty: {}", stats.current_difficulty);
    
    // Validate
    println!("\n Validating blockchain...");
    if bc.is_valid() {
        println!("   All Falcon signatures verified!");
        println!("   All nonces valid!");
        println!("   Blockchain integrity confirmed!");
        println!("   Data persisted to disk: {}", db_path);
    }
    drop(bc);
    
    // Display comparison
    println!("\n");
    println!("           FALCON vs ECDSA COMPARISON                          ");
    println!("");
    println!("                    Falcon-512    ECDSA (secp256k1)           ");
    println!(" Public Key Size:    897 bytes    33 bytes                    ");
    println!(" Private Key Size:  1281 bytes    32 bytes                    ");
    println!(" Signature Size:     666 bytes    65 bytes                    ");
    println!(" Quantum Resistant:   YES         NO                        ");
    println!(" NIST PQC Standard:   YES         NO                        ");
    println!("");
    
    println!("\n Production demo complete!");
    println!(" Blockchain persisted to: {}", db_path);
    println!(" All amounts stored as u64 microunits (deterministic)");
    println!(" Nonce-based replay protection enabled");
    println!("  Demo wallets password: INSECURE_DEMO_PASSWORD_DO_NOT_USE_IN_PRODUCTION");
    println!("  WARNING: Demo password is PUBLIC - delete wallets after testing!");
    println!("\n To start API server:");
    println!("   cargo run --release -- start --db {} --port 3000", db_path);
}
