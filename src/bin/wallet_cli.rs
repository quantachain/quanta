#[allow(deprecated)]
use quanta::crypto::{QuantumWallet, HDWallet, TreasuryMultisig, TreasuryMultisigV2, MultiSigTransaction};
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
    /// Create a new encrypted Falcon-512 raw wallet (advanced/server use).
    /// For regular users, prefer `new-hd` which gives you a 24-word recovery phrase.
    New {
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },

    /// Create a new HD wallet with a 24-word mnemonic (RECOMMENDED)
    NewHd {
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        #[arg(short, long, default_value = "1")]
        accounts: u32,
    },

    /// Restore HD wallet from mnemonic phrase (mnemonic is prompted, not a CLI arg)
    RestoreHd {
        #[arg(short, long, default_value = "hd_wallet.json")]
        file: String,
        /// How many accounts to restore
        #[arg(short, long, default_value = "1")]
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
    // Treasury multisig (3-of-N)
    // ──────────────────────────────────────────────────────────────────────
    /// Initialize a 3-of-N treasury multisig (generates N Falcon-512 keys, requires 3 to spend)
    TreasuryInit {
        /// Output path for treasury setup JSON (keep alongside your wallet files)
        #[arg(long, default_value = "treasury_setup.json")]
        out: String,
        /// Prefix for the N key wallet files
        #[arg(long, default_value = "treasury")]
        key_prefix: String,
        /// Total number of keyholders N (must be ≥ 3). Any 3 of N can sign a spend.
        /// Common choices: 5 (2 keys can be lost safely), 7 (4 keys can be lost safely)
        #[arg(long, default_value = "5")]
        signers: usize,
        /// Password for all N key files (use TREASURY_KEY_PASSWORD env var to avoid prompt)
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
            println!("\n  NOTE: 'new' creates a raw key wallet (no recovery phrase).");
            println!("  For regular users, consider 'new-hd' instead — it gives you");
            println!("  a 24-word mnemonic that can restore all your accounts.\n");
            let wallet = QuantumWallet::new();
            let pwd = read_new_password("wallet");
            wallet.save_quantum_safe(&file, &pwd).expect("Failed to save wallet");
            println!("\n Wallet created!");
            println!("  Address : {}", wallet.address);
            println!("  File    : {}", file);
            println!("\n  SECURITY: Back up this file and remember the password!");
            println!("  WARNING : There is NO recovery phrase — if you lose this file, funds are lost!");
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
            println!("  CRITICAL: Write down your 24-word mnemonic shown above!");
            println!("  It is the ONLY way to recover your wallet if you lose the file.");
        }

        Commands::RestoreHd { file, accounts } => {
            // SECURITY: Mnemonic is prompted interactively — never passed as a CLI arg
            // (CLI args appear in shell history and `ps` output, exposing the seed phrase)
            println!("Enter your 24-word mnemonic phrase:");
            let mnemonic = rpassword::read_password().expect("Failed to read mnemonic");
            let mnemonic = mnemonic.trim().to_string();
            if mnemonic.split_whitespace().count() < 12 {
                eprintln!(" Error: Mnemonic must be at least 12 words.");
                std::process::exit(1);
            }
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
                lock_time: 0,
                tx_type:   TransactionType::Transfer,
                sig_scheme: SignatureScheme::Falcon512,
                // Testnet. A --network flag can be added later to select mainnet (1).
                network_id: 0,
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
        Commands::TreasuryInit { out, key_prefix, signers, password } => {
            // Validate
            if signers < TreasuryMultisigV2::REQUIRED {
                eprintln!(
                    " --signers must be >= {} (got {}). A 3-of-N treasury needs at least 3 keyholders.",
                    TreasuryMultisigV2::REQUIRED, signers
                );
                std::process::exit(1);
            }

            println!("\n Generating 3-of-{} treasury multisig (Falcon-512)...\n", signers);

            let (setup, keypairs) = TreasuryMultisigV2::generate(signers);

            println!("  Policy          : {}", setup.policy_string());
            println!("  Treasury Address: {}", setup.address);
            println!("\n  NEXT STEP: Add this to quanta.toml:");
            println!("    treasury_address = \"{}\"", setup.address);

            // Save treasury setup JSON
            std::fs::write(&out, setup.to_json()).expect("Failed to save treasury setup");
            println!("\n  Setup saved     : {}", out);

            // Determine password
            let pwd = password
                .or_else(|| std::env::var("TREASURY_KEY_PASSWORD").ok())
                .unwrap_or_else(|| {
                    println!("\n  Enter password for all {} treasury key files:", signers);
                    rpassword::read_password().expect("Failed to read password")
                });

            // Save N key wallet files
            println!();
            for (i, kp) in keypairs.iter().enumerate() {
                let keyfile = format!("{}_key{}.qua", key_prefix, i);
                let wallet = QuantumWallet {
                    keypair: kp.clone(),
                    address: kp.get_address(),
                };
                wallet.save_quantum_safe(&keyfile, &pwd).expect("Failed to save key file");
                println!("  Key {} saved : {} (address: {})", i, keyfile, kp.get_address());
            }

            println!("\n SECURITY CHECKLIST:");
            println!("  [ ] Distribute the {} key files to {} SEPARATE secure locations", signers, signers);
            println!("  [ ] Add treasury_address to quanta.toml and restart the node");
            println!("  [ ] Test with a small treasury-propose before accumulating large balance");
            println!("  [ ] Never store more than 2 keys on the same machine in production");
            println!("  [ ] Any {} of {} keyholders can authorize a spend", TreasuryMultisigV2::REQUIRED, signers);
        }

        Commands::TreasuryInfo { setup, node } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");

            // Try V2 format first (3-of-N), fall back to legacy V1 (2-of-3)
            let (address, required, total) =
                if let Ok(ts) = TreasuryMultisigV2::from_json(&json) {
                    (ts.address, ts.required, ts.public_keys.len())
                } else {
                    #[allow(deprecated)]
                    let ts = TreasuryMultisig::from_json(&json)
                        .expect("Invalid treasury setup JSON (tried both V2 and legacy V1 formats)");
                    #[allow(deprecated)]
                    (ts.address, ts.required, ts.public_keys.len())
                };

            let balance = fetch_balance(&node, &address).await;
            println!("\n Treasury Info");
            println!("  Address : {}", address);
            println!("  Policy  : {}-of-{} Falcon-512 multisig", required, total);
            println!("  Balance : {:.6} QUA ({} microunits)", balance as f64 / 1_000_000.0, balance);
        }

        Commands::TreasuryPropose { setup, to, amount, fee, nonce, out } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");

            // Try V2 format first (3-of-N), fall back to legacy V1 (2-of-3)
            let proposal = if let Ok(ts) = TreasuryMultisigV2::from_json(&json) {
                ts.propose_spend(
                    to.clone(),
                    qua_to_microunits(amount),
                    qua_to_microunits(fee),
                    nonce,
                    Utc::now().timestamp(),
                )
            } else {
                #[allow(deprecated)]
                let ts = TreasuryMultisig::from_json(&json)
                    .expect("Invalid treasury setup JSON (tried both V2 and legacy V1 formats)");
                ts.propose_spend(
                    to.clone(),
                    qua_to_microunits(amount),
                    qua_to_microunits(fee),
                    nonce,
                    Utc::now().timestamp(),
                )
            };

            let from_addr  = proposal.base_tx.sender.clone();
            let req_sigs   = proposal.required_signatures;
            let total_keys = proposal.public_keys.len();

            std::fs::write(&out, proposal.to_json()).expect("Failed to save proposal");
            println!("\n Treasury spend proposal created!");
            println!("  From    : {}", from_addr);
            println!("  To      : {}", to);
            println!("  Amount  : {:.6} QUA", amount);
            println!("  Nonce   : {}", nonce);
            println!("  Out     : {}", out);
            println!("\n  Next: sign with any {} of {} treasury keys:", req_sigs, total_keys);
            for i in 0..req_sigs {
                println!(
                    "   quanta-wallet treasury-sign --proposal {} --key treasury_key{}.qua --index {}",
                    out, i, i
                );
            }
        }

        Commands::TreasurySign { proposal, key, index } => {
            let prop_json = std::fs::read_to_string(&proposal).expect("Could not read proposal");
            let mut prop = MultiSigTransaction::from_json(&prop_json)
                .expect("Invalid proposal JSON");

            let pwd = read_password(&format!("Enter password for {}", key));
            let wallet = QuantumWallet::load_quantum_safe(&key, &pwd)
                .expect("Failed to load treasury key wallet");

            #[allow(deprecated)]
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
    let url = format!("{}/api/balance/{}", node, address);
    let resp = reqwest::get(&url).await;
    match resp {
        Ok(r) => {
            if let Ok(j) = r.json::<serde_json::Value>().await {
                j["balance_microunits"].as_u64()
                    .or_else(|| j["balance"].as_u64())
                    .unwrap_or(0)
            } else { 0 }
        }
        Err(_) => {
            eprintln!("  Warning: could not fetch balance from {}", node);
            0
        }
    }
}

async fn fetch_nonce(node: &str, address: &str) -> u64 {
    // GET /api/balance/:address returns { balance_microunits, nonce, address }
    let url = format!("{}/api/balance/{}", node, address);
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
    let url = format!("{}/api/transactions/submit", node);
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
