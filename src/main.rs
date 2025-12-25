mod crypto;
mod transaction;
mod block;
mod blockchain;
mod wallet;
mod secure_wallet;
mod storage;
mod api;

use blockchain::Blockchain;
use secure_wallet::SecureWallet;
use storage::BlockchainStorage;
use chrono::Utc;
use clap::{Parser, Subcommand};
use std::sync::Arc;
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
    /// Start the blockchain node with REST API
    Start {
        /// API server port
        #[arg(short, long, default_value = "3000")]
        port: u16,
        
        /// Database path
        #[arg(short, long, default_value = "./quanta_data")]
        db: String,
    },
    
    /// Create a new encrypted wallet
    NewWallet {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
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
        Commands::Start { port, db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Arc::new(Blockchain::new(storage).expect("Failed to initialize blockchain"));
            
            api::start_server(blockchain, port).await;
        }

        Commands::NewWallet { file } => {
            let wallet = SecureWallet::new();
            
            println!("\n🔐 Enter password to encrypt wallet:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            println!("🔐 Confirm password:");
            let password_confirm = rpassword::read_password().expect("Failed to read password");
            
            if password != password_confirm {
                eprintln!("❌ Passwords don't match!");
                return;
            }
            
            wallet.save_encrypted(&file, &password).expect("Failed to save wallet");
            println!("✅ Wallet created and encrypted successfully!");
        }

        Commands::Wallet { file } => {
            println!("🔐 Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match SecureWallet::load_encrypted(&file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("❌ Failed to load wallet: {}", e);
                    return;
                }
            };
            
            // Load blockchain to get balance
            let storage = Arc::new(BlockchainStorage::new("./quanta_data").expect("Failed to open database"));
            let blockchain = Blockchain::new(storage).expect("Failed to initialize blockchain");
            let balance = blockchain.get_balance(&wallet.address);
            
            wallet.display_info(balance);
        }

        Commands::Mine { wallet: wallet_file, db } => {
            println!("🔐 Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match SecureWallet::load_encrypted(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("❌ Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Blockchain::new(storage).expect("Failed to initialize blockchain");
            
            println!("⛏️  Mining new block...");
            match blockchain.mine_pending_transactions(wallet.address.clone()) {
                Ok(_) => {
                    println!("✅ Block mined successfully!");
                    let balance = blockchain.get_balance(&wallet.address);
                    println!("💰 New balance: {:.6} QUA", balance);
                }
                Err(e) => eprintln!("❌ Mining failed: {}", e),
            }
        }

        Commands::Send { wallet: wallet_file, to, amount, db } => {
            println!("🔐 Enter wallet password:");
            let password = rpassword::read_password().expect("Failed to read password");
            
            let wallet = match SecureWallet::load_encrypted(&wallet_file, &password) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("❌ Failed to load wallet: {}", e);
                    return;
                }
            };
            
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Blockchain::new(storage).expect("Failed to initialize blockchain");
            
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
            
            match blockchain.add_transaction(tx) {
                Ok(_) => println!("✅ Transaction added to mempool"),
                Err(e) => eprintln!("❌ Transaction failed: {}", e),
            }
        }

        Commands::Stats { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Blockchain::new(storage).expect("Failed to initialize blockchain");
            let stats = blockchain.get_stats();
            
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
            println!("║ 🛡️  Quantum Resistance: ACTIVE                                ║");
            println!("║ Signature Algorithm: Falcon-512 (NIST PQC)                   ║");
            println!("║ Hash Algorithm: SHA3-256                                      ║");
            println!("║ 🔐 Wallet Encryption: AES-256-GCM + Argon2                    ║");
            println!("║ 💾 Persistent Storage: Sled Database                          ║");
            println!("╚═══════════════════════════════════════════════════════════════╝");
        }

        Commands::Validate { db } => {
            let storage = Arc::new(BlockchainStorage::new(&db).expect("Failed to open database"));
            let blockchain = Blockchain::new(storage).expect("Failed to initialize blockchain");
            
            println!("🔍 Validating blockchain...");
            
            if blockchain.is_valid() {
                println!("✅ Blockchain is VALID");
                println!("   All blocks verified");
                println!("   All Falcon signatures verified");
                println!("   Chain integrity maintained");
            } else {
                println!("❌ Blockchain is INVALID");
            }
        }

        Commands::Demo { db } => {
            println!("🎬 Running Production Demo...\n");
            run_demo(&db).await;
        }
    }
}

async fn run_demo(db_path: &str) {
    let storage = Arc::new(BlockchainStorage::new(db_path).expect("Failed to open database"));
    
    // Clear old demo data
    storage.clear().expect("Failed to clear database");
    
    let blockchain = Arc::new(Blockchain::new(storage).expect("Failed to initialize blockchain"));
    
    // Create demo wallets
    println!("📝 Creating encrypted demo wallets...");
    let wallet1 = SecureWallet::new();
    let wallet2 = SecureWallet::new();
    let wallet3 = SecureWallet::new();
    
    let password = "demo123";
    wallet1.save_encrypted("demo_wallet1.qua", password).unwrap();
    wallet2.save_encrypted("demo_wallet2.qua", password).unwrap();
    wallet3.save_encrypted("demo_wallet3.qua", password).unwrap();
    
    println!("\n⛏️  Mining genesis rewards...");
    blockchain.mine_pending_transactions(wallet1.address.clone()).unwrap();
    blockchain.mine_pending_transactions(wallet1.address.clone()).unwrap();
    
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
    blockchain.add_transaction(tx1).unwrap();
    
    println!("\n⛏️  Mining first transaction...");
    blockchain.mine_pending_transactions(wallet2.address.clone()).unwrap();
    
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
    blockchain.add_transaction(tx2).unwrap();
    
    println!("\n⛏️  Mining second transaction...");
    blockchain.mine_pending_transactions(wallet3.address.clone()).unwrap();
    
    // Show final balances
    println!("\n💰 Final Balances:");
    println!("Wallet 1: {:.6} QUA", blockchain.get_balance(&wallet1.address));
    println!("Wallet 2: {:.6} QUA", blockchain.get_balance(&wallet2.address));
    println!("Wallet 3: {:.6} QUA", blockchain.get_balance(&wallet3.address));
    
    // Show stats
    let stats = blockchain.get_stats();
    println!("\n📊 Blockchain Stats:");
    println!("Blocks: {}", stats.chain_length);
    println!("Transactions: {}", stats.total_transactions);
    println!("Total Supply: {:.2} QUA", stats.total_supply);
    
    // Validate
    println!("\n🔍 Validating blockchain...");
    if blockchain.is_valid() {
        println!("✅ All Falcon signatures verified!");
        println!("✅ Blockchain integrity confirmed!");
        println!("✅ Data persisted to disk: {}", db_path);
    }
    
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
    println!("🔐 Wallets encrypted with password: demo123");
    println!("\n📡 To start API server:");
    println!("   cargo run --release -- start --db {} --port 3000", db_path);
}
