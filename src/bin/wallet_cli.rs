use quanta::crypto::{QuantumWallet, HDWallet, FalconKeypair, TreasuryMultisig, MultiSigTransaction};
use quanta::core::transaction::{Transaction, TransactionType, SignatureScheme};
use clap::{Parser, Subcommand};
use chrono::Utc;

const MICROUNITS_PER_QUA: u64 = 1_000_000;

fn qua_to_microunits(qua: f64) -> u64 { (qua * MICROUNITS_PER_QUA as f64) as u64 }

#[derive(Parser)]
#[command(name = "quanta-wallet")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "QUANTA Wallet CLI — Quantum-Resistant Key Management & Transactions", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "kebab-case")]
enum Commands {
    // ──────────────────────────────────────────────────────────────────────
    // Wallet management
    // ──────────────────────────────────────────────────────────────────────
    /// Create a new encrypted Falcon-512 wallet
    New {
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },

    /// Create a new HD wallet with 24-word mnemonic
    NewHd {
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        #[arg(short, long, default_value = "3")]
        accounts: u32,
    },

    /// Restore HD wallet from mnemonic phrase
    RestoreHd {
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        /// Mnemonic phrase (24 words, quoted)
        mnemonic: String,
        #[arg(short, long, default_value = "3")]
        accounts: u32,
    },

    /// Show wallet address
    Address {
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },

    /// Show wallet info and balance (requires running node)
    Info {
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ──────────────────────────────────────────────────────────────────────
    // Transactions
    // ──────────────────────────────────────────────────────────────────────
    /// Send QUA to another address (submits to running node)
    Send {
        #[arg(short, long, default_value = "wallet.qua")]
        wallet: String,
        #[arg(short, long)]
        to: String,
        #[arg(short, long)]
        amount: f64,
        #[arg(long, default_value = "0.001")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ──────────────────────────────────────────────────────────────────────
    // Treasury multisig (2-of-3, founder holds all 3 keys)
    // ──────────────────────────────────────────────────────────────────────
    /// Initialize 2-of-3 treasury multisig (generates 3 Falcon-512 keys)
    TreasuryInit {
        /// Output path for treasury setup JSON (keep alongside your wallet files)
        #[arg(long, default_value = "treasury_setup.json")]
        out: String,
        /// Prefix for the 3 key wallet files
        #[arg(long, default_value = "treasury")]
        key_prefix: String,
        /// Password for the 3 key files (use TREASURY_KEY_PASSWORD env var to avoid prompt)
        #[arg(long)]
        password: Option<String>,
    },

    /// Show treasury address and balance
    TreasuryInfo {
        #[arg(long, default_value = "treasury_setup.json")]
        setup: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Propose a treasury spend (creates unsigned proposal JSON)
    TreasuryPropose {
        #[arg(long, default_value = "treasury_setup.json")]
        setup: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: f64,
        #[arg(long, default_value = "0.01")]
        fee: f64,
        #[arg(long)]
        nonce: u64,
        #[arg(long, default_value = "proposal.json")]
        out: String,
    },

    /// Sign a treasury proposal with one of your 3 treasury keys
    TreasurySign {
        #[arg(long, default_value = "proposal.json")]
        proposal: String,
        /// Treasury key wallet file (treasury_key0.qua, treasury_key1.qua, or treasury_key2.qua)
        #[arg(long)]
        key: String,
        /// Key index in the setup (0, 1, or 2)
        #[arg(long)]
        index: usize,
    },

    /// Broadcast a completed (2-of-3 signed) treasury proposal to the network
    TreasuryBroadcast {
        #[arg(long, default_value = "proposal.json")]
        proposal: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        // ──────────────────── Wallet creation ────────────────────
        Commands::New { file } => {
            let wallet = QuantumWallet::new();
            let pwd = read_new_password("wallet");
            wallet.save_quantum_safe(&file, &pwd).expect("Failed to save wallet");
            println!("\n Wallet created!");
            println!("  Address : {}", wallet.address);
            println!("  File    : {}", file);
            println!("\n  SECURITY: Back up this file and remember the password!");
        }

        Commands::NewHd { file, accounts } => {
            let mut wallet = HDWallet::new();
            for i in 0..accounts {
                wallet.generate_account(Some(format!("Account {}", i)));
            }
            wallet.display_info();
            let pwd = read_new_password("HD wallet");
            let encrypted = wallet.export_encrypted(&pwd).expect("Encrypt failed");
            std::fs::write(&file, encrypted).expect("Save failed");
            println!("\n HD Wallet saved to: {}", file);
            println!("  CRITICAL: Write down your 24-word mnemonic above!");
        }

        Commands::RestoreHd { file, mnemonic, accounts } => {
            let mut wallet = HDWallet::from_mnemonic(mnemonic, "");
            for i in 0..accounts {
                wallet.generate_account(Some(format!("Account {}", i)));
            }
            wallet.display_info();
            let pwd = read_new_password("HD wallet");
            let encrypted = wallet.export_encrypted(&pwd).expect("Encrypt failed");
            std::fs::write(&file, encrypted).expect("Save failed");
            println!("\n HD Wallet restored and saved to: {}", file);
        }

        Commands::Address { file } => {
            let pwd = read_password("Enter wallet password");
            match QuantumWallet::load_quantum_safe(&file, &pwd) {
                Ok(w)  => println!("{}", w.address),
                Err(e) => { eprintln!(" Failed: {}", e); std::process::exit(1); }
            }
        }

        Commands::Info { file, node } => {
            let pwd = read_password("Enter wallet password");
            let wallet = QuantumWallet::load_quantum_safe(&file, &pwd)
                .expect("Failed to load wallet");
            let balance = fetch_balance(&node, &wallet.address).await;
            wallet.display_info(balance as f64 / MICROUNITS_PER_QUA as f64);
        }

        // ──────────────────── Send ────────────────────
        Commands::Send { wallet, to, amount, fee, node } => {
            let pwd = read_password("Enter wallet password");
            let w = QuantumWallet::load_quantum_safe(&wallet, &pwd)
                .expect("Failed to load wallet");

            // Fetch current nonce from node
            let nonce = fetch_nonce(&node, &w.address).await + 1;

            let mut tx = Transaction {
                sender:    w.address.clone(),
                recipient: to.clone(),
                amount:    qua_to_microunits(amount),
                timestamp: Utc::now().timestamp(),
                signature: vec![],
                public_key: w.keypair.public_key.clone(),
                fee:       qua_to_microunits(fee),
                nonce,
                tx_type:   TransactionType::Transfer,
                sig_scheme: SignatureScheme::Falcon512,
            };

            let signing_bytes = tx.get_signing_bytes();
            tx.signature = w.keypair.sign_transaction_canonical(&signing_bytes);

            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Transaction submitted!");
                    println!("  TX Hash  : {}", hash);
                    println!("  From     : {}", w.address);
                    println!("  To       : {}", to);
                    println!("  Amount   : {:.6} QUA", amount);
                    println!("  Fee      : {:.6} QUA", fee);
                }
                Err(e) => {
                    eprintln!(" Failed to broadcast transaction: {}", e);
                    std::process::exit(1);
                }
            }
        }

        // ──────────────────── Treasury ────────────────────
        Commands::TreasuryInit { out, key_prefix, password } => {
            println!("\n Generating 2-of-3 treasury multisig (Falcon-512)...\n");

            let (setup, [k0, k1, k2]) = TreasuryMultisig::generate();

            println!("  Treasury Address : {}", setup.address);
            println!("\n  NEXT STEP: Set this in quanta.toml:");
            println!("    treasury_address = \"{}\"", setup.address);

            // Save treasury setup JSON
            std::fs::write(&out, setup.to_json()).expect("Failed to save treasury setup");
            println!("\n  Setup saved   : {}", out);

            // Determine password
            let pwd = password
                .or_else(|| std::env::var("TREASURY_KEY_PASSWORD").ok())
                .unwrap_or_else(|| {
                    println!("\n  Enter password for all 3 treasury key files:");
                    rpassword::read_password().expect("Failed to read password")
                });

            // Save 3 key wallet files
            for (i, kp) in [&k0, &k1, &k2].iter().enumerate() {
                let keyfile = format!("{}_key{}.qua", key_prefix, i);
                let wallet = QuantumWallet {
                    keypair: (*kp).clone(),
                    address: kp.get_address(),
                };
                wallet.save_quantum_safe(&keyfile, &pwd).expect("Failed to save key file");
                println!("  Key {} saved   : {} (address: {})", i, keyfile, kp.get_address());
            }

            println!("\n SECURITY CHECKLIST:");
            println!("  [ ] Copy the 3 key files to 3 SEPARATE USB drives / backup locations");
            println!("  [ ] Add treasury_address to quanta.toml and restart node");
            println!("  [ ] Test with a small treasury-propose before accumulating large balance");
            println!("  [ ] Never store all 3 keys on the same machine in production");
        }

        Commands::TreasuryInfo { setup, node } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");
            let ts = TreasuryMultisig::from_json(&json).expect("Invalid treasury setup JSON");
            let balance = fetch_balance(&node, &ts.address).await;
            println!("\n Treasury Info");
            println!("  Address : {}", ts.address);
            println!("  Policy  : {}-of-{} Falcon-512 multisig", ts.required, ts.public_keys.len());
            println!("  Balance : {:.6} QUA ({} microunits)", balance as f64 / 1_000_000.0, balance);
        }

        Commands::TreasuryPropose { setup, to, amount, fee, nonce, out } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");
            let ts = TreasuryMultisig::from_json(&json).expect("Invalid treasury setup JSON");

            let proposal = ts.propose_spend(
                to.clone(),
                qua_to_microunits(amount),
                qua_to_microunits(fee),
                nonce,
                Utc::now().timestamp(),
            );

            std::fs::write(&out, proposal.to_json()).expect("Failed to save proposal");
            println!("\n Treasury spend proposal created!");
            println!("  From    : {}", ts.address);
            println!("  To      : {}", to);
            println!("  Amount  : {:.6} QUA", amount);
            println!("  Nonce   : {}", nonce);
            println!("  Out     : {}", out);
            println!("\n  Next: sign with 2 of 3 treasury keys:");
            println!("   quanta-wallet treasury-sign --proposal {} --key treasury_key0.qua --index 0", out);
            println!("   quanta-wallet treasury-sign --proposal {} --key treasury_key1.qua --index 1", out);
        }

        Commands::TreasurySign { proposal, key, index } => {
            let prop_json = std::fs::read_to_string(&proposal).expect("Could not read proposal");
            let mut prop = MultiSigTransaction::from_json(&prop_json)
                .expect("Invalid proposal JSON");

            let pwd = read_password(&format!("Enter password for {}", key));
            let wallet = QuantumWallet::load_quantum_safe(&key, &pwd)
                .expect("Failed to load treasury key wallet");

            TreasuryMultisig::sign_proposal(&mut prop, index, &wallet.keypair)
                .expect("Failed to sign proposal");

            let (collected, required) = prop.signature_progress();
            std::fs::write(&proposal, prop.to_json()).expect("Failed to save signed proposal");

            println!("\n Signed with key {} successfully!", index);
            println!("  Signatures: {}/{}", collected, required);
            if prop.is_complete() {
                println!("\n  READY TO BROADCAST!");
                println!("   quanta-wallet treasury-broadcast --proposal {}", proposal);
            } else {
                println!("  Need {} more signature(s)", required - collected);
            }
        }

        Commands::TreasuryBroadcast { proposal, node } => {
            let json = std::fs::read_to_string(&proposal).expect("Could not read proposal");
            let prop = MultiSigTransaction::from_json(&json).expect("Invalid proposal JSON");

            if !prop.is_complete() {
                let (col, req) = prop.signature_progress();
                eprintln!(" Proposal not complete: {}/{} signatures", col, req);
                std::process::exit(1);
            }

            if !prop.verify() {
                eprintln!(" Signature verification failed! Proposal is invalid.");
                std::process::exit(1);
            }

            // For multisig, we submit the base_tx (consensus doesn't know multisig yet)
            // The base_tx sender is the multisig address — works for balance tracking
            match broadcast_tx(&node, &prop.base_tx).await {
                Ok(hash) => {
                    println!("\n Treasury transaction broadcast!");
                    println!("  TX Hash : {}", hash);
                    println!("  To      : {}", prop.base_tx.recipient);
                    println!("  Amount  : {:.6} QUA", prop.base_tx.amount as f64 / 1_000_000.0);
                }
                Err(e) => {
                    eprintln!(" Broadcast failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────

fn read_password(prompt: &str) -> String {
    if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") { return p; }
    println!("{}:", prompt);
    rpassword::read_password().expect("Failed to read password")
}

fn read_new_password(label: &str) -> String {
    if let Ok(p) = std::env::var("QUANTA_WALLET_PASSWORD") { return p; }
    loop {
        println!("Enter password for {}:", label);
        let p1 = rpassword::read_password().expect("Read failed");
        println!("Confirm password:");
        let p2 = rpassword::read_password().expect("Read failed");
        if p1 == p2 { return p1; }
        println!(" Passwords don't match, try again.");
    }
}

async fn fetch_balance(node: &str, address: &str) -> u64 {
    let url = format!("{}/balance/{}", node, address);
    let resp = reqwest::get(&url).await;
    match resp {
        Ok(r) => {
            if let Ok(j) = r.json::<serde_json::Value>().await {
                j["balance"].as_u64().unwrap_or(0)
            } else { 0 }
        }
        Err(_) => {
            eprintln!("  Warning: could not fetch balance from {}", node);
            0
        }
    }
}

async fn fetch_nonce(node: &str, address: &str) -> u64 {
    let url = format!("{}/nonce/{}", node, address);
    let resp = reqwest::get(&url).await;
    match resp {
        Ok(r) => {
            if let Ok(j) = r.json::<serde_json::Value>().await {
                j["nonce"].as_u64().unwrap_or(0)
            } else { 0 }
        }
        Err(_) => 0,
    }
}

async fn broadcast_tx(node: &str, tx: &Transaction) -> Result<String, String> {
    let url = format!("{}/transactions", node);
    let client = reqwest::Client::new();
    let resp = client.post(&url)
        .json(tx)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();

    if status.is_success() {
        Ok(body["tx_hash"].as_str().unwrap_or("(unknown)").to_string())
    } else {
        Err(body["error"].as_str().unwrap_or("Unknown error").to_string())
    }
}
