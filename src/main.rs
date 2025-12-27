mod crypto;
mod transaction;
mod block;
mod blockchain;
mod quantum_wallet;
mod storage;
mod api;
mod p2p;
mod mempool;
mod config;
mod merkle;
mod prometheus_metrics;
mod hd_wallet;
mod multisig;
mod contract;
mod contract_executor;

#[cfg(test)]
mod tests;

use blockchain::Blockchain;
use quantum_wallet::QuantumWallet;
use storage::BlockchainStorage;
use p2p::{Network, NetworkConfig};
use mempool::MetricsCollector;
use config::QuantaConfig;
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "quanta")]
#[command(about = "QUANTA - Quantum-Resistant Blockchain with Falcon Signatures", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the blockchain node with REST API and P2P networking
    Start {
        /// Configuration file path
        #[arg(short = 'c', long)]
        config: Option<String>,
        
        /// API server port (overrides config)
        #[arg(short, long)]
        port: Option<u16>,
        
        /// P2P network port (overrides config)
        #[arg(short = 'n', long)]
        network_port: Option<u16>,
        
        /// Database path (overrides config)
        #[arg(short, long)]
        db: Option<String>,
        
        /// Bootstrap peer addresses (comma-separated host:port)
        #[arg(short = 'b', long)]
        bootstrap: Option<String>,
        
        /// Disable P2P networking (single node mode)
        #[arg(long)]
        no_network: bool,
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
    
    /// Deploy a smart contract (WASM)
    DeployContract {
        /// Path to WASM file
        #[arg(short, long)]
        wasm: String,
        
        /// Deployer wallet file
        #[arg(short = 'w', long, default_value = "wallet.qua")]
        wallet: String,
        
        /// Transaction fee
        #[arg(short, long, default_value_t = 0.1)]
        fee: f64,
        
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Call a smart contract function
    CallContract {
        /// Contract address
        #[arg(short, long)]
        contract: String,
        
        /// Function name
        #[arg(short, long)]
        function: String,
        
        /// Function arguments (JSON format)
        #[arg(short, long, default_value = "{}")]
        args: String,
        
        /// Amount to send (QUA)
        #[arg(short = 'm', long, default_value_t = 0.0)]
        amount: f64,
        
        /// Caller wallet file
        #[arg(short = 'w', long, default_value = "wallet.qua")]
        wallet: String,
        
        /// Transaction fee
        #[arg(short, long, default_value_t = 0.01)]
        fee: f64,
        
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// List deployed contracts
    ListContracts {
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║        QUANTA - Quantum-Resistant Blockchain                  ║");
    println!("║         Falcon Signatures | Post-Quantum Cryptography         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let cli = Cli::parse();

    match cli.command {
        Commands::Start { config, port, network_port, db, bootstrap, no_network } => {
            // Load configuration
            let cfg = QuantaConfig::load_with_overrides(
                config,
                port,
                network_port,
                db,
                bootstrap.clone(),
                no_network
            ).expect("Failed to load configuration");
            
            tracing::info!("Starting QUANTA node with configuration:");
            tracing::info!("  API Port: {}", cfg.node.api_port);
            tracing::info!("  Network Port: {}", cfg.node.network_port);
            tracing::info!("  Database: {}", cfg.node.db_path);
            
            let storage = Arc::new(BlockchainStorage::new(&cfg.node.db_path).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            
            let metrics = Arc::new(MetricsCollector::new());
            
            // Start Prometheus metrics server if enabled
            if cfg.metrics.enabled {
                let metrics_port = cfg.metrics.port;
                tokio::spawn(async move {
                    prometheus_metrics::start_metrics_server(metrics_port).await;
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
            }
        }

        Commands::NewWallet { file } => {
            let wallet = QuantumWallet::new();
            
            println!("\nEnter password to encrypt wallet:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            println!("Confirm password:");
            let password_confirm = rpassword::read_password().expect("Failed to read password");
            
            if password != password_confirm {
                eprintln!("Passwords don't match!");
                return;
            }
            
            wallet.save_quantum_safe(&file, &password).expect("Failed to save wallet");
            println!("Wallet created and encrypted successfully!");
        }

        Commands::NewHdWallet { file, accounts } => {
            use hd_wallet::HDWallet;
            
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
            
            println!("\n✅ HD Wallet created and encrypted successfully!");
            println!("📁 Saved to: {}", file);
            println!("\n⚠️  CRITICAL: Write down your 24-word mnemonic phrase!");
            println!("   This is the ONLY way to recover your wallet.");
        }

        Commands::HdWallet { file } => {
            println!("Enter wallet password:");
            let _password = rpassword::read_password().expect("Failed to read password");
            
            // For now, we'll need to implement proper loading
            println!("HD wallet info display - implementation needed for encrypted load");
            println!("Wallet file: {}", file);
        }

        Commands::Wallet { file } => {
            println!("Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match QuantumWallet::load_quantum_safe(&file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("Failed to load wallet: {}", e);
                    return;
                }
            };
            
            // Load blockchain to get balance
            let storage = Arc::new(BlockchainStorage::new("./quanta_data").expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            let balance = blockchain.read().await.get_balance(&wallet.address);
            
            wallet.display_info(balance);
        }

        Commands::Mine { wallet: wallet_file, db } => {
            println!("Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match QuantumWallet::load_quantum_safe(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("❌ Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            
            println!("Mining new block...");
            let mine_result = blockchain.write().await.mine_pending_transactions(wallet.address.clone());
            match mine_result {
                Ok(_) => {
                    println!("Block mined successfully!");
                    let balance = blockchain.read().await.get_balance(&wallet.address);
                    println!("New balance: {:.6} QUA", balance);
                }
                Err(e) => eprintln!("Mining failed: {}", e),
            }
        }

        Commands::Send { wallet: wallet_file, to, amount, db } => {
            println!("Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match QuantumWallet::load_quantum_safe(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("❌ Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            
            let mut tx = transaction::Transaction::new(
                wallet.address.clone(),
                to,
                amount,
                Utc::now().timestamp(),
            );
            
            // Sign transaction
            let signing_data = tx.get_signing_data();
            tx.signature = wallet.keypair.sign(&signing_data);
            tx.public_key = wallet.keypair.public_key.clone();
            
            let add_result = blockchain.write().await.add_transaction(tx);
            match add_result {
                Ok(_) => println!("Transaction added to mempool"),
                Err(e) => eprintln!("Transaction failed: {}", e),
            }
        }

        Commands::Stats { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            let stats = blockchain.read().await.get_stats();
            
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║                QUANTA BLOCKCHAIN STATISTICS                   ║");
            println!("╠═══════════════════════════════════════════════════════════════╣");
            println!("║ Chain Length: {} blocks                                  ║", stats.chain_length);
            println!("║ Total Transactions: {}                                    ║", stats.total_transactions);
            println!("║ Current Difficulty: {}                                     ║", stats.current_difficulty);
            println!("║ Mining Reward: {:.2} QUA                                 ║", stats.mining_reward);
            println!("║ Total Supply: {:.2} QUA                                 ║", stats.total_supply);
            println!("║ Pending Transactions: {}                                   ║", stats.pending_transactions);
            println!("╠═══════════════════════════════════════════════════════════════╣");
            println!("║ Quantum Resistance: ACTIVE                                  ║");
            println!("║ Signature Algorithm: Falcon-512 (NIST PQC)                   ║");
            println!("║ Hash Algorithm: SHA3-256                                      ║");
            println!("║ Wallet Encryption: Kyber-1024 + ChaCha20-Poly1305            ║");
            println!("║ Persistent Storage: Sled Database                            ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
        }

        Commands::Validate { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            
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
        
        Commands::DeployContract { wasm, wallet, fee, db } => {
            handle_deploy_contract(wasm, wallet, fee, db).await;
        }
        
        Commands::CallContract { contract, function, args, amount, wallet, fee, db } => {
            handle_call_contract(contract, function, args, amount, wallet, fee, db).await;
        }
        
        Commands::ListContracts { db } => {
            handle_list_contracts(db).await;
        }
    }
}

async fn run_demo(db_path: &str) {
    use crate::transaction;
    let storage = Arc::new(BlockchainStorage::new(db_path).expect("Failed to open database"));
    
    // Clear old demo data
    storage.clear().expect("Failed to clear database");
    
    let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
    
    // Create demo wallets
    println!("📝 Creating quantum-safe encrypted demo wallets...");
    let wallet1 = QuantumWallet::new();
    let wallet2 = QuantumWallet::new();
    let wallet3 = QuantumWallet::new();
    
    // WARNING: Insecure password for demo ONLY! Never use in production!
    const DEMO_PASSWORD: &str = "INSECURE_DEMO_PASSWORD_DO_NOT_USE_IN_PRODUCTION";
    println!("⚠️  Demo wallets use INSECURE password - FOR TESTING ONLY!");
    
    wallet1.save_quantum_safe("demo_wallet1.qua", DEMO_PASSWORD).unwrap();
    wallet2.save_quantum_safe("demo_wallet2.qua", DEMO_PASSWORD).unwrap();
    wallet3.save_quantum_safe("demo_wallet3.qua", DEMO_PASSWORD).unwrap();
    
    println!("\n⛏️  Mining genesis rewards...");
    blockchain.write().await.mine_pending_transactions(wallet1.address.clone()).unwrap();
    blockchain.write().await.mine_pending_transactions(wallet1.address.clone()).unwrap();
    
    println!("\n💸 Creating transactions...");
    
    // Transaction 1
    let mut tx1 = transaction::Transaction::new(
        wallet1.address.clone(),
        wallet2.address.clone(),
        25.0,
        Utc::now().timestamp(),
    );
    let signing_data1 = tx1.get_signing_data();
    tx1.signature = wallet1.keypair.sign(&signing_data1);
    tx1.public_key = wallet1.keypair.public_key.clone();
    blockchain.write().await.add_transaction(tx1).unwrap();
    
    println!("\n⛏️  Mining first transaction...");
    blockchain.write().await.mine_pending_transactions(wallet2.address.clone()).unwrap();
    
    // Transaction 2
    let mut tx2 = transaction::Transaction::new(
        wallet1.address.clone(),
        wallet3.address.clone(),
        15.0,
        Utc::now().timestamp(),
    );
    let signing_data2 = tx2.get_signing_data();
    tx2.signature = wallet1.keypair.sign(&signing_data2);
    tx2.public_key = wallet1.keypair.public_key.clone();
    blockchain.write().await.add_transaction(tx2).unwrap();
    
    println!("\n⛏️  Mining second transaction...");
    blockchain.write().await.mine_pending_transactions(wallet3.address.clone()).unwrap();
    
    // Show final balances
    println!("\n💰 Final Balances:");
    let bc = blockchain.read().await;
    println!("Wallet 1: {:.6} QUA", bc.get_balance(&wallet1.address));
    println!("Wallet 2: {:.6} QUA", bc.get_balance(&wallet2.address));
    println!("Wallet 3: {:.6} QUA", bc.get_balance(&wallet3.address));
    
    // Show stats
    let stats = bc.get_stats();
    println!("\n📊 Blockchain Stats:");
    println!("Blocks: {}", stats.chain_length);
    println!("Transactions: {}", stats.total_transactions);
    println!("Total Supply: {:.2} QUA", stats.total_supply);
    
    // Validate
    println!("\n🔍 Validating blockchain...");
    if bc.is_valid() {
        println!("✅ All Falcon signatures verified!");
        println!("✅ Blockchain integrity confirmed!");
        println!("✅ Data persisted to disk: {}", db_path);
    }
    drop(bc);
    
    // Display comparison
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║           FALCON vs ECDSA COMPARISON                          ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║                    Falcon-512  │  ECDSA (secp256k1)           ║");
    println!("║ Public Key Size:    897 bytes  │  33 bytes                    ║");
    println!("║ Private Key Size:  1281 bytes  │  32 bytes                    ║");
    println!("║ Signature Size:     666 bytes  │  65 bytes                    ║");
    println!("║ Quantum Resistant:  ✓ YES      │  ✗ NO                        ║");
    println!("║ NIST PQC Standard:  ✓ YES      │  ✗ NO                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    
    println!("\n🎉 Production demo complete!");
    println!("💾 Blockchain persisted to: {}", db_path);
    println!("⚠️  Demo wallets password: INSECURE_DEMO_PASSWORD_DO_NOT_USE_IN_PRODUCTION");
    println!("⚠️  WARNING: Demo password is PUBLIC - delete wallets after testing!");
    println!("\n📡 To start API server:");
    println!("   cargo run --release -- start --db {} --port 3000", db_path);
}

async fn handle_deploy_contract(wasm: String, wallet: String, fee: f64, db: String) {
    use std::fs;
    use std::sync::Arc;
    use crate::storage::BlockchainStorage;
    use crate::blockchain::Blockchain;
    use crate::transaction::Transaction;
    use chrono::Utc;
    
    println!("🚀 Deploying smart contract...");
    
    // Read WASM file
    let code = match fs::read(&wasm) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("❌ Failed to read WASM file: {}", e);
            return;
        }
    };
    
    println!("📦 WASM size: {} bytes", code.len());
    
    // Load wallet
    println!("🔓 Enter wallet password:");
    let password = rpassword::read_password().expect("Failed to read password");
    
    let wallet = match quantum_wallet::QuantumWallet::load_quantum_safe(&wallet, &password) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("❌ Failed to load wallet: {}", e);
            return;
        }
    };
    
    // Load blockchain
    let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
    let blockchain = Blockchain::new(storage).expect("Failed to load blockchain");
    
    // Create deploy transaction
    let mut tx = Transaction::new_deploy_contract(
        wallet.address.clone(),
        code,
        Utc::now().timestamp(),
        fee,
    );
    
    // Sign transaction
    let signing_data = tx.get_signing_data();
    tx.signature = wallet.keypair.sign(&signing_data);
    tx.public_key = wallet.keypair.public_key.clone();
    
    // Add to blockchain
    match blockchain.add_transaction(tx) {
        Ok(_) => {
            println!("✅ Contract deployment transaction added to mempool");
            println!("⛏️  Transaction will be included in the next mined block");
            println!("💡 Contract address will be generated upon mining");
        }
        Err(e) => {
            eprintln!("❌ Failed to add transaction: {:?}", e);
        }
    }
}

async fn handle_call_contract(
    contract: String,
    function: String,
    args: String,
    amount: f64,
    wallet: String,
    fee: f64,
    db: String,
) {
    use std::sync::Arc;
    use crate::storage::BlockchainStorage;
    use crate::blockchain::Blockchain;
    use crate::transaction::Transaction;
    use chrono::Utc;
    
    println!("📞 Calling smart contract...");
    println!("   Contract: {}", contract);
    println!("   Function: {}", function);
    println!("   Args: {}", args);
    
    // Parse args (for now, just convert to bytes)
    let args_bytes = args.as_bytes().to_vec();
    
    // Load wallet
    println!("🔓 Enter wallet password:");
    let password = rpassword::read_password().expect("Failed to read password");
    
    let wallet = match quantum_wallet::QuantumWallet::load_quantum_safe(&wallet, &password) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("❌ Failed to load wallet: {}", e);
            return;
        }
    };
    
    // Load blockchain
    let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
    let blockchain = Blockchain::new(storage).expect("Failed to load blockchain");
    
    // Create call transaction
    let mut tx = Transaction::new_call_contract(
        wallet.address.clone(),
        contract.clone(),
        function.clone(),
        args_bytes,
        amount,
        Utc::now().timestamp(),
        fee,
    );
    
    // Sign transaction
    let signing_data = tx.get_signing_data();
    tx.signature = wallet.keypair.sign(&signing_data);
    tx.public_key = wallet.keypair.public_key.clone();
    
    // Add to blockchain
    match blockchain.add_transaction(tx) {
        Ok(_) => {
            println!("✅ Contract call transaction added to mempool");
            println!("⛏️  Will be executed when block is mined");
        }
        Err(e) => {
            eprintln!("❌ Failed to add transaction: {:?}", e);
        }
    }
}

async fn handle_list_contracts(db: String) {
    println!("📜 Listing deployed contracts...\n");
    
    // TODO: Implement contract listing from storage
    // For now, show placeholder
    println!("Contract listing not yet implemented");
    println!("Contracts will be stored in: {}/contracts", db);
}
