mod core;
mod consensus;
mod crypto;
mod storage;
mod network;
mod api;
mod config;

#[cfg(test)]
mod tests;

use consensus::Blockchain;
use crypto::QuantumWallet;
use storage::BlockchainStorage;
use network::{Network, NetworkConfig};
use consensus::MetricsCollector;
use config::QuantaConfig;
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
            let balance_microunits = blockchain.read().await.get_balance(&wallet.address);
            
            wallet.display_info(microunits_to_qua(balance_microunits));
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
            
            println!("⛏️  Mining new block...");
            let mine_result = blockchain.write().await.mine_pending_transactions(wallet.address.clone());
            match mine_result {
                Ok(_) => {
                    println!("✅ Block mined successfully!");
                    let balance_microunits = blockchain.read().await.get_balance(&wallet.address);
                    println!("💰 New balance: {:.6} QUA", microunits_to_qua(balance_microunits));
                }
                Err(e) => eprintln!("❌ Mining failed: {}", e),
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
                    println!("✅ Transaction added to mempool");
                    println!("📤 Sending {:.6} QUA to {}", amount, to);
                    println!("🔢 Nonce: {}", next_nonce);
                }
                Err(e) => eprintln!("❌ Transaction failed: {}", e),
            }
        }

        Commands::Stats { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(RwLock::new(Blockchain::new(storage).expect("Failed to initialize blockchain")));
            let stats = blockchain.read().await.get_stats();
            
            let reward_qua = microunits_to_qua(stats.mining_reward);
            let supply_qua = microunits_to_qua(stats.total_supply);
            
            println!("╔═══════════════════════════════════════════════════════════════╗");
            println!("║                QUANTA BLOCKCHAIN STATISTICS                   ║");
            println!("╠═══════════════════════════════════════════════════════════════╣");
            println!("║ Chain Length: {} blocks                                  ║", stats.chain_length);
            println!("║ Total Transactions: {}                                    ║", stats.total_transactions);
            println!("║ Current Difficulty: {}                                     ║", stats.current_difficulty);
            println!("║ Mining Reward: {:.6} QUA                                 ║", reward_qua);
            println!("║ Total Supply: {:.6} QUA                                  ║", supply_qua);
            println!("║ Pending Transactions: {}                                   ║", stats.pending_transactions);
            println!("╠═══════════════════════════════════════════════════════════════╣");
            println!("║ Quantum Resistance: ACTIVE                                  ║");
            println!("║ Signature Algorithm: Falcon-512 (NIST PQC)                   ║");
            println!("║ Hash Algorithm: SHA3-256                                      ║");
            println!("║ Wallet Encryption: Kyber-1024 + ChaCha20-Poly1305            ║");
            println!("║ Persistent Storage: Sled Database                            ║");
            println!("║ Amount Precision: u64 microunits (deterministic)             ║");
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
    }
}

async fn run_demo(db_path: &str) {
    use crate::core::transaction::{Transaction, TransactionType};
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
    println!("  ✅ Tx 1: 25 QUA to wallet2 (nonce {})", nonce1);
    
    println!("\n⛏️  Mining first transaction...");
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
    println!("  ✅ Tx 2: 15 QUA to wallet3 (nonce {})", nonce2);
    
    println!("\n⛏️  Mining second transaction...");
    blockchain.write().await.mine_pending_transactions(wallet3.address.clone()).unwrap();
    
    // Show final balances
    println!("\n💰 Final Balances:");
    let bc = blockchain.read().await;
    let bal1 = microunits_to_qua(bc.get_balance(&wallet1.address));
    let bal2 = microunits_to_qua(bc.get_balance(&wallet2.address));
    let bal3 = microunits_to_qua(bc.get_balance(&wallet3.address));
    println!("  Wallet 1: {:.6} QUA", bal1);
    println!("  Wallet 2: {:.6} QUA", bal2);
    println!("  Wallet 3: {:.6} QUA", bal3);
    
    // Show stats
    let stats = bc.get_stats();
    println!("\n📊 Blockchain Stats:");
    println!("  Blocks: {}", stats.chain_length);
    println!("  Transactions: {}", stats.total_transactions);
    println!("  Total Supply: {:.6} QUA ({} microunits)", microunits_to_qua(stats.total_supply), stats.total_supply);
    println!("  Current Difficulty: {}", stats.current_difficulty);
    
    // Validate
    println!("\n🔍 Validating blockchain...");
    if bc.is_valid() {
        println!("  ✅ All Falcon signatures verified!");
        println!("  ✅ All nonces valid!");
        println!("  ✅ Blockchain integrity confirmed!");
        println!("  ✅ Data persisted to disk: {}", db_path);
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
    println!("💰 All amounts stored as u64 microunits (deterministic)");
    println!("🔢 Nonce-based replay protection enabled");
    println!("⚠️  Demo wallets password: INSECURE_DEMO_PASSWORD_DO_NOT_USE_IN_PRODUCTION");
    println!("⚠️  WARNING: Demo password is PUBLIC - delete wallets after testing!");
    println!("\n📡 To start API server:");
    println!("   cargo run --release -- start --db {} --port 3000", db_path);
}
