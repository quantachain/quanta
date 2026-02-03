use quanta::crypto::{QuantumWallet, HDWallet};
use clap::{Parser, Subcommand};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "quanta-wallet")]
#[command(about = "QUANTA Wallet - Secure Key Management", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "snake_case")]
enum Commands {
    /// Create a new encrypted wallet
    New {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },
    
    /// Create a new HD wallet with 24-word mnemonic
    NewHd {
        /// Wallet file name
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        
        /// Number of accounts to generate
        #[arg(short, long, default_value = "3")]
        accounts: u32,
    },
    
    /// Show wallet information
    Info {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },
    
    /// Show wallet address only
    Address {
        /// Wallet file name
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },
}

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::New { file } => {
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

        Commands::NewHd { file, accounts } => {
            let mut wallet = HDWallet::new();
            
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
            
            let encrypted = wallet.export_encrypted(&password).expect("Failed to encrypt wallet");
            std::fs::write(&file, encrypted).expect("Failed to save wallet");
            
            println!("\n HD Wallet created and encrypted successfully!");
            println!(" Saved to: {}", file);
        }

        Commands::Info { file } => {
             // Basic implementation - loading would require password prompt
             println!("Wallet info for: {}", file);
             println!("To view full details, use the main node client or implement password prompt here.");
        }
        
        Commands::Address { file } => {
            println!("Enter password to decrypt wallet:");
            let password = rpassword::read_password().expect("Failed to read password");
             
            match QuantumWallet::load_quantum_safe(&file, &password) {
                Ok(w) => println!("{}", w.address),
                Err(e) => eprintln!("Failed to load wallet: {}", e),
            }
        }
    }
}
