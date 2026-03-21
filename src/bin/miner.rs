/// quanta-miner — Standalone mining daemon (Monero-style separate binary)
///
/// Usage:
///   quanta-miner start --wallet miner.qua --node http://localhost:3000
///   quanta-miner status --node http://localhost:7782
///   quanta-miner stop   --node http://localhost:7782
use clap::{Parser, Subcommand};
use quanta::crypto::QuantumWallet;
use quanta::rpc::RpcClient;

#[derive(Parser)]
#[command(name = "quanta-miner")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "QUANTA Miner — Standalone mining daemon", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "kebab-case")]
enum Commands {
    /// Start mining to your wallet address (sends start_mining RPC to node)
    Start {
        /// Wallet file containing your mining reward address
        #[arg(short, long, default_value = "wallet.qua")]
        wallet: String,

        /// Override mining address (skips wallet load if provided)
        #[arg(long)]
        address: Option<String>,

        /// RPC port of the running quanta node
        #[arg(short, long, default_value = "7782")]
        rpc_port: u16,
    },

    /// Stop mining
    Stop {
        #[arg(short, long, default_value = "7782")]
        rpc_port: u16,
    },

    /// Show current mining status
    Status {
        #[arg(short, long, default_value = "7782")]
        rpc_port: u16,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start { wallet, address, rpc_port } => {
            // Resolve mining address
            let mining_address = if let Some(addr) = address {
                addr
            } else {
                let pwd = if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") { p } else {
                    println!("Enter wallet password:");
                    rpassword::read_password().expect("Failed to read password")
                };
                let w = QuantumWallet::load_quantum_safe(&wallet, &pwd)
                    .expect("Failed to load wallet");
                w.address
            };

            println!(" Starting mining to: {}", mining_address);
            println!("  Connecting to node RPC on port {}...", rpc_port);

            let client = RpcClient::new(rpc_port);
            match client.start_mining(&mining_address).await {
                Ok(_) => {
                    println!("\n Mining started!");
                    println!("  Rewards sent to : {}", mining_address);
                    println!("  Block time       : 30 seconds");
                    println!("  Block reward     : 100 QUA (Year 1)");
                    println!("  Your share       : 47.5 QUA per block (50% locked 6 months)");
                    println!("\n Use 'quanta-miner status' to monitor");
                    println!("     'quanta-miner stop'   to stop");
                }
                Err(e) => {
                    eprintln!(" Failed to start mining: {}", e);
                    eprintln!("  Is the quanta node running? Start it with:");
                    eprintln!("    quanta start");
                    std::process::exit(1);
                }
            }
        }

        Commands::Stop { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            match client.stop_mining().await {
                Ok(_) => println!(" Mining stopped"),
                Err(e) => { eprintln!(" Failed: {}", e); std::process::exit(1); }
            }
        }

        Commands::Status { rpc_port } => {
            let client = RpcClient::new(rpc_port);
            match client.get_mining_status().await {
                Ok(s) => {
                    println!("\n QUANTA Miner Status");
                    println!("  Mining       : {}", if s.is_mining { "ACTIVE " } else { "STOPPED" });
                    if let Some(ref addr) = s.mining_address {
                        println!("  Address      : {}", addr);
                    }
                    println!("  Blocks mined : {}", s.blocks_mined);
                    println!("  Difficulty   : {}", s.difficulty);
                    println!("  Reward       : {:.6} QUA", s.mining_reward as f64 / 1_000_000.0);
                    if let Some(t) = s.last_block_time {
                        use chrono::{DateTime, Utc};
                        let dt = DateTime::<Utc>::from_timestamp(t, 0).unwrap_or_else(Utc::now);
                        println!("  Last block   : {}", dt.format("%Y-%m-%d %H:%M:%S UTC"));
                    }
                    println!();
                }
                Err(e) => { eprintln!(" Failed: {}", e); std::process::exit(1); }
            }
        }
    }
}
