// Quanta Wallet CLI
// HD wallet is the default. Use `new-raw` only for server/programmatic key management.
// All commands support QUANTA_WALLET_PASSWORD env var for non-interactive / AI-agent use.

#[allow(deprecated)]
use quanta::crypto::{QuantumWallet, HDWallet, TreasuryMultisigV2, MultiSigTransaction};
use quanta::core::transaction::{Transaction, TransactionType, SignatureScheme};
use quanta::core::contracts::{NativeContracts, EscrowInitArgs, EscrowClaimArgs, TEMPLATE_ESCROW};
use clap::{Parser, Subcommand};
use chrono::Utc;

const MICROUNITS_PER_QUA: u64 = 1_000_000;

fn qua_to_u(qua: f64) -> u64 { (qua * MICROUNITS_PER_QUA as f64) as u64 }
fn u_to_qua(u: u64) -> f64 { u as f64 / MICROUNITS_PER_QUA as f64 }

// ─────────────────────────────────────────────────────────────────────────────
// CLI definition
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "quanta-wallet")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "Quanta Wallet CLI — HD key management, transactions, contracts, and staking")]
#[command(long_about = "
Quanta Wallet CLI — Quantum-Resistant (Falcon-512) Wallet & AI Agent SDK

QUICK START:
  quanta-wallet new              # Create HD wallet (24-word mnemonic)
  quanta-wallet address          # Show your address
  quanta-wallet send --to <ADDR> --amount 10.5

AI AGENT QUICK START:
  export QUANTA_WALLET_PASSWORD=mypassword   # skip password prompts
  quanta-wallet deploy-escrow --beneficiary <WORKER_ADDR> --secret-hash <HASH> --amount 1.0
  quanta-wallet claim-escrow  --contract <ADDR> --preimage <HEX>
")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(rename_all = "kebab-case")]
enum Commands {
    // ── Wallet Management ──────────────────────────────────────────────────

    /// [DEFAULT] Create a new HD wallet with a 24-word recovery mnemonic.
    /// Use this for all regular users and AI agents.
    New {
        /// Output wallet file path
        #[arg(short, long, default_value = "wallet.json")]
        file: String,
        /// Number of accounts to pre-generate
        #[arg(short, long, default_value = "1")]
        accounts: u32,
    },

    /// Restore an HD wallet from its 24-word mnemonic phrase (prompted securely).
    Restore {
        /// Output wallet file path
        #[arg(short, long, default_value = "wallet.json")]
        file: String,
        /// Number of accounts to restore
        #[arg(short, long, default_value = "1")]
        accounts: u32,
    },

    /// [ADVANCED] Create a raw Falcon-512 key wallet (no recovery phrase).
    /// Use for server/HSM deployments where the file IS the backup.
    NewRaw {
        /// Output wallet file path
        #[arg(short, long, default_value = "wallet.qua")]
        file: String,
    },

    /// Show wallet address(es).
    Address {
        #[arg(short, long, default_value = "wallet.json")]
        file: String,
    },

    /// Reveal the 24-word recovery mnemonic from an HD wallet file.
    ///
    /// Use this to import your existing wallet into the browser extension:
    ///   1. Run this command and copy the phrase.
    ///   2. Open the extension → Import Wallet → Mnemonic tab.
    ///   3. Paste the phrase and set a password.
    ///
    /// Only works with HD wallets (wallet.json). Raw .qua wallets have no
    /// recovery phrase — the file itself is the key.
    ShowMnemonic {
        #[arg(short, long, default_value = "wallet.json")]
        file: String,
    },

    /// Export your Address and Falcon-512 Public Key to a genesis JSON file.
    ExportValidator {
        #[arg(short, long, default_value = "wallet.qua")]
        wallet: String,
        #[arg(short, long, default_value = "validator.json")]
        out: String,
    },

    /// Show wallet balance and info (requires a running node).
    Info {
        #[arg(short, long, default_value = "wallet.json")]
        file: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ── Basic Transactions ─────────────────────────────────────────────────

    /// Send QUA to an address.
    Send {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        /// Recipient address
        #[arg(long)]
        to: String,
        /// Amount in QUA (e.g. 10.5)
        #[arg(long)]
        amount: f64,
        /// Fee in QUA
        #[arg(long, default_value = "0.001")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Send QUA with an attached data payload (AI agent data provenance).
    /// The payload is cryptographically bound to the transaction signature.
    SendWithData {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        amount: f64,
        #[arg(long, default_value = "0.001")]
        fee: f64,
        /// Data payload (UTF-8 string, e.g. JSON). Included in the tx signature.
        #[arg(long)]
        data: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ── Staking / BFT Validator ────────────────────────────────────────────

    /// Register as a BFT validator by staking QUA.
    /// Your wallet's Falcon-512 public key is used for BFT signing.
    Stake {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        /// Amount of QUA to stake (minimum recommended: 1000)
        #[arg(long)]
        amount: f64,
        #[arg(long, default_value = "0.01")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Deregister as a BFT validator and begin the unbonding period.
    /// Staked QUA is locked for 2 epochs before it is returned to your balance.
    Unstake {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        #[arg(long, default_value = "0.01")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ── Native Smart Contracts ─────────────────────────────────────────────

    /// Deploy a trustless Escrow contract.
    ///
    /// WHAT IT DOES: Locks your QUA on-chain. The beneficiary (worker AI) can
    /// claim the funds only by providing the preimage (raw bytes) whose SHA3-256
    /// hash matches the --secret-hash you provide here. Perfect for trustless
    /// AI-to-AI hiring: the employer locks funds, the worker proves task completion.
    ///
    /// EXAMPLE:
    ///   # Compute the hash of your task output file:
    ///   sha3sum output.dat → 3a7f...
    ///   quanta-wallet deploy-escrow --beneficiary <WORKER> --secret-hash 3a7f... --amount 5.0
    DeployEscrow {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        /// The worker agent's address that will receive funds upon claim
        #[arg(long)]
        beneficiary: String,
        /// SHA3-256 hash of the task output (hex string, 64 chars)
        #[arg(long)]
        secret_hash: String,
        /// QUA to lock in the escrow
        #[arg(long)]
        amount: f64,
        #[arg(long, default_value = "0.01")]
        fee: f64,
        /// Block height after which the deployer can reclaim funds (0 = no refund deadline)
        #[arg(long, default_value = "0")]
        refund_height: u64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Claim funds from an Escrow contract by providing the preimage.
    ///
    /// WHAT IT DOES: Proves task completion by submitting the raw preimage (hex)
    /// whose SHA3-256 hash was committed in the escrow deployment. If correct,
    /// funds are atomically transferred to the beneficiary address.
    ClaimEscrow {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        /// The escrow contract address (starts with 0xc_)
        #[arg(long)]
        contract: String,
        /// The raw preimage in hex (the actual task output hash, not its SHA3)
        #[arg(long)]
        preimage: String,
        #[arg(long, default_value = "0.001")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Generic contract call — invoke any method on any deployed native contract.
    ContractCall {
        #[arg(short, long, default_value = "wallet.json")]
        wallet: String,
        /// Contract address (starts with 0xc_)
        #[arg(long)]
        contract: String,
        /// Method name to invoke (e.g. "claim")
        #[arg(long)]
        method: String,
        /// JSON-encoded arguments (e.g. '{"preimage":"deadbeef"}')
        #[arg(long, default_value = "{}")]
        args: String,
        #[arg(long, default_value = "0.001")]
        fee: f64,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    // ── Treasury Multisig ──────────────────────────────────────────────────

    /// Initialize a 3-of-N treasury multisig (generates N Falcon-512 keys).
    TreasuryInit {
        #[arg(long, default_value = "treasury_setup.json")]
        out: String,
        #[arg(long, default_value = "treasury")]
        key_prefix: String,
        /// Total signers N (≥ 3). Requires any 3 to authorize a spend.
        #[arg(long, default_value = "5")]
        signers: usize,
        #[arg(long)]
        password: Option<String>,
    },

    /// Show treasury address and balance.
    TreasuryInfo {
        #[arg(long, default_value = "treasury_setup.json")]
        setup: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },

    /// Propose a treasury spend (creates unsigned proposal JSON).
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

    /// Sign a treasury proposal with one of your treasury keys.
    TreasurySign {
        #[arg(long, default_value = "proposal.json")]
        proposal: String,
        /// Treasury key wallet file (e.g. treasury_key0.qua)
        #[arg(long)]
        key: String,
        /// Key index in the setup (0, 1, 2, …)
        #[arg(long)]
        index: usize,
    },

    /// Broadcast a fully signed treasury proposal to the network.
    TreasuryBroadcast {
        #[arg(long, default_value = "proposal.json")]
        proposal: String,
        #[arg(short, long, default_value = "http://localhost:3000")]
        node: String,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {

        // ── Wallet creation ──────────────────────────────────────────────

        Commands::New { file, accounts } => {
            let mut wallet = HDWallet::new();
            for i in 0..accounts {
                wallet.generate_account(Some(format!("Account {}", i)));
            }
            wallet.display_info();
            let pwd = read_new_password("wallet");
            let encrypted = wallet.export_encrypted(&pwd).expect("Encryption failed");
            std::fs::write(&file, encrypted).expect("Failed to save wallet");
            println!("\n Wallet saved: {}", file);
            println!("  CRITICAL: Write down your 24-word mnemonic shown above!");
            println!("  It is the ONLY way to recover your funds if you lose this file.");
        }

        Commands::Restore { file, accounts } => {
            println!("Enter your 24-word mnemonic phrase (input hidden):");
            let mnemonic = rpassword::read_password().expect("Failed to read mnemonic");
            let mnemonic = mnemonic.trim().to_string();
            if mnemonic.split_whitespace().count() < 12 {
                die("Mnemonic must be at least 12 words.");
            }
            // BIP39 passphrase ("25th word") — optional, defaults to "" which matches
            // all wallets created without a passphrase (backward compatible).
            // To restore a wallet created with a passphrase:
            //   export QUANTA_WALLET_PASSPHRASE="your-passphrase"
            //   quanta-wallet restore
            let bip39_passphrase = std::env::var("QUANTA_WALLET_PASSPHRASE")
                .unwrap_or_default();
            if !bip39_passphrase.is_empty() {
                println!("  BIP39 passphrase: [SET via QUANTA_WALLET_PASSPHRASE]");
            }
            let mut wallet = HDWallet::from_mnemonic(mnemonic, &bip39_passphrase);
            for i in 0..accounts {
                wallet.generate_account(Some(format!("Account {}", i)));
            }
            wallet.display_info();
            let pwd = read_new_password("wallet");
            let encrypted = wallet.export_encrypted(&pwd).expect("Encryption failed");
            std::fs::write(&file, encrypted).expect("Failed to save wallet");
            println!("\n Wallet restored and saved: {}", file);
        }

        Commands::NewRaw { file } => {
            println!("\n  NOTE: Raw wallets have NO recovery phrase.");
            println!("  If you lose this file, your funds are unrecoverable.");
            println!("  Consider `quanta-wallet new` (HD wallet) for regular use.\n");
            let wallet = QuantumWallet::new();
            let pwd = read_new_password("raw wallet");
            wallet.save_quantum_safe(&file, &pwd).expect("Failed to save wallet");
            println!("\n Raw wallet created!");
            println!("  Address : {}", wallet.address);
            println!("  File    : {}", file);
        }

        Commands::Address { file } => {
            match try_load_wallet(&file) {
                WalletKind::Hd(w)  => {
                    for acc in &w.accounts {
                        println!("{}", acc.address);
                    }
                }
                WalletKind::Raw(w) => println!("{}", w.address),
                WalletKind::None(e) => die(&e),
            }
        }

        Commands::ShowMnemonic { file } => {
            match try_load_wallet(&file) {
                WalletKind::Hd(w) => {
                    eprintln!("\n  ⚠  KEEP THIS SECRET — anyone with this phrase controls your funds.\n");
                    println!("{}", w.mnemonic);
                    eprintln!("\n  Paste this into the wallet extension → Import Wallet → Mnemonic tab.");
                }
                WalletKind::Raw(_) => die(
                    "Raw .qua wallets have no recovery phrase — the file IS the key.\n\
                     Use the extension's 'Import from Private Key' panel instead."
                ),
                WalletKind::None(e) => die(&e),
            }
        }

        Commands::ExportValidator { wallet, out } => {
            let kp = load_keypair_for_signing(&wallet);
            let public_key_hex = hex::encode(&kp.keypair.public_key);
            
            // Generate a simple JSON object string
            let json = format!(
                "{{\n  \"address\": \"{}\",\n  \"public_key\": \"{}\"\n}}",
                kp.address, public_key_hex
            );
            
            std::fs::write(&out, json).unwrap_or_else(|e| die(&format!("Failed to write {}: {}", out, e)));
            println!("\n Validator keys exported successfully!");
            println!("  File      : {}", out);
            println!("  Address   : {}", kp.address);
            println!("\n Send {} to the network coordinator to be included in the Genesis Block.", out);
        }

        Commands::Info { file, node } => {
            match try_load_wallet(&file) {
                WalletKind::Hd(w) => {
                    println!("\n HD Wallet ({} account(s))", w.accounts.len());
                    for acc in &w.accounts {
                        let bal = fetch_balance(&node, &acc.address).await;
                        println!("  [{}] {} — {:.6} QUA", acc.label.as_deref().unwrap_or("?"), acc.address, u_to_qua(bal));
                    }
                }
                WalletKind::Raw(w) => {
                    let bal = fetch_balance(&node, &w.address).await;
                    w.display_info(u_to_qua(bal));
                }
                WalletKind::None(e) => die(&e),
            }
        }

        // ── Basic transactions ────────────────────────────────────────────

        Commands::Send { wallet, to, amount, fee, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;
            let tx = build_transfer(&kp, &to, amount, fee, nonce, vec![]);
            broadcast_and_print(&node, &tx, "Transfer", &kp.address, &to, amount, fee).await;
        }

        Commands::SendWithData { wallet, to, amount, fee, data, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;
            let data_len = data.len();
            let tx = build_transfer(&kp, &to, amount, fee, nonce, data.into_bytes());
            broadcast_and_print(&node, &tx, "Transfer+Data", &kp.address, &to, amount, fee).await;
            println!("  Payload  : {} bytes attached", data_len);
        }

        // ── Staking ───────────────────────────────────────────────────────

        Commands::Stake { wallet, amount, fee, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;
            let mut tx = Transaction {
                sender:     kp.address.clone(),
                recipient:  kp.address.clone(), // staking is self-directed
                amount:     qua_to_u(amount),
                timestamp:  Utc::now().timestamp(),
                signature:  vec![],
                public_key: kp.keypair.public_key.clone(),
                fee:        qua_to_u(fee),
                nonce,
                lock_time:  0,
                tx_type:    TransactionType::Stake { validator_pubkey: kp.keypair.public_key.clone() },
                sig_scheme: SignatureScheme::Falcon512,
                network_id: 0,
                payload:    vec![],
            };
            let sig_bytes = tx.get_signing_bytes();
            tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);
            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Stake transaction submitted!");
                    println!("  TX Hash  : {}", hash);
                    println!("  Staked   : {:.6} QUA", amount);
                    println!("  Validator: {}", kp.address);
                    println!("\n  You will join the BFT committee at the next epoch boundary.");
                }
                Err(e) => die(&format!("Stake failed: {}", e)),
            }
        }

        Commands::Unstake { wallet, fee, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;
            let mut tx = Transaction {
                sender:     kp.address.clone(),
                recipient:  kp.address.clone(),
                amount:     0,
                timestamp:  Utc::now().timestamp(),
                signature:  vec![],
                public_key: kp.keypair.public_key.clone(),
                fee:        qua_to_u(fee),
                nonce,
                lock_time:  0,
                tx_type:    TransactionType::Unstake,
                sig_scheme: SignatureScheme::Falcon512,
                network_id: 0,
                payload:    vec![],
            };
            let sig_bytes = tx.get_signing_bytes();
            tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);
            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Unstake transaction submitted!");
                    println!("  TX Hash  : {}", hash);
                    println!("  Address  : {}", kp.address);
                    println!("\n  Staked QUA will be locked for 2 epochs before returning to your balance.");
                }
                Err(e) => die(&format!("Unstake failed: {}", e)),
            }
        }

        // ── Native Contracts ──────────────────────────────────────────────

        Commands::DeployEscrow { wallet, beneficiary, secret_hash, amount, fee, refund_height, node } => {
            if secret_hash.len() != 64 {
                die("--secret-hash must be a 64-character hex string (SHA3-256 output).");
            }
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;

            let init_args = serde_json::to_vec(&EscrowInitArgs {
                beneficiary: beneficiary.clone(),
                secret_hash: secret_hash.clone(),
                refund_height,
            }).expect("Failed to encode init args");

            // Compute the deterministic contract address from a preview of the tx hash
            // (the node will compute this authoritatively — we show it as a preview)
            let mut tx = Transaction {
                sender:     kp.address.clone(),
                recipient:  String::new(), // filled below
                amount:     qua_to_u(amount),
                timestamp:  Utc::now().timestamp(),
                signature:  vec![],
                public_key: kp.keypair.public_key.clone(),
                fee:        qua_to_u(fee),
                nonce,
                lock_time:  0,
                tx_type:    TransactionType::ContractDeploy { template_id: TEMPLATE_ESCROW, init_args },
                sig_scheme: SignatureScheme::Falcon512,
                network_id: 0,
                payload:    vec![],
            };
            let tx_hash = tx.hash();
            let contract_address = NativeContracts::generate_address(&tx_hash);
            tx.recipient = contract_address.clone();

            let sig_bytes = tx.get_signing_bytes();
            tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);

            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Escrow deployed!");
                    println!("  TX Hash        : {}", hash);
                    println!("  Contract Addr  : {}", contract_address);
                    println!("  Beneficiary    : {}", beneficiary);
                    println!("  Secret Hash    : {}", secret_hash);
                    println!("  Locked Amount  : {:.6} QUA", amount);
                    println!("\n  Share the contract address with the worker so they can call claim.");
                    println!("  Worker command:");
                    println!("    quanta-wallet claim-escrow --contract {} --preimage <HEX>", contract_address);
                }
                Err(e) => die(&format!("Deploy failed: {}", e)),
            }
        }

        Commands::ClaimEscrow { wallet, contract, preimage, fee, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;

            let call_args = serde_json::to_vec(&EscrowClaimArgs {
                preimage: preimage.clone(),
            }).expect("Failed to encode claim args");

            let mut tx = Transaction {
                sender:     kp.address.clone(),
                recipient:  contract.clone(),
                amount:     0,
                timestamp:  Utc::now().timestamp(),
                signature:  vec![],
                public_key: kp.keypair.public_key.clone(),
                fee:        qua_to_u(fee),
                nonce,
                lock_time:  0,
                tx_type:    TransactionType::ContractCall {
                    contract_address: contract.clone(),
                    method:           "claim".to_string(),
                    call_args,
                },
                sig_scheme: SignatureScheme::Falcon512,
                network_id: 0,
                payload:    vec![],
            };
            let sig_bytes = tx.get_signing_bytes();
            tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);

            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Escrow claim submitted!");
                    println!("  TX Hash   : {}", hash);
                    println!("  Contract  : {}", contract);
                    let preimage_preview = &preimage[..preimage.len().min(16)];
                    println!("  Preimage  : {}...", preimage_preview);
                    println!("\n  If the preimage matches, funds will be released to the beneficiary.");
                }
                Err(e) => die(&format!("Claim failed: {}", e)),
            }
        }

        Commands::ContractCall { wallet, contract, method, args, fee, node } => {
            let kp = load_keypair_for_signing(&wallet);
            let nonce = fetch_nonce(&node, &kp.address).await + 1;

            let call_args: Vec<u8> = args.into_bytes();

            let mut tx = Transaction {
                sender:     kp.address.clone(),
                recipient:  contract.clone(),
                amount:     0,
                timestamp:  Utc::now().timestamp(),
                signature:  vec![],
                public_key: kp.keypair.public_key.clone(),
                fee:        qua_to_u(fee),
                nonce,
                lock_time:  0,
                tx_type:    TransactionType::ContractCall {
                    contract_address: contract.clone(),
                    method:           method.clone(),
                    call_args,
                },
                sig_scheme: SignatureScheme::Falcon512,
                network_id: 0,
                payload:    vec![],
            };
            let sig_bytes = tx.get_signing_bytes();
            tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);

            match broadcast_tx(&node, &tx).await {
                Ok(hash) => {
                    println!("\n Contract call submitted!");
                    println!("  TX Hash  : {}", hash);
                    println!("  Contract : {}", contract);
                    println!("  Method   : {}", method);
                }
                Err(e) => die(&format!("Contract call failed: {}", e)),
            }
        }

        // ── Treasury ──────────────────────────────────────────────────────

        Commands::TreasuryInit { out, key_prefix, signers, password } => {
            if signers < TreasuryMultisigV2::REQUIRED {
                die(&format!(
                    "--signers must be >= {} (got {}). A 3-of-N treasury needs at least 3 keyholders.",
                    TreasuryMultisigV2::REQUIRED, signers
                ));
            }
            println!("\n Generating 3-of-{} treasury multisig (Falcon-512)...\n", signers);
            let (setup, keypairs) = TreasuryMultisigV2::generate(signers);
            println!("  Policy          : {}", setup.policy_string());
            println!("  Treasury Address: {}", setup.address);
            std::fs::write(&out, setup.to_json()).expect("Failed to save treasury setup");
            println!("\n  Setup saved: {}", out);

            let pwd = password
                .or_else(|| std::env::var("QUANTA_WALLET_PASSWORD").ok())
                .unwrap_or_else(|| {
                    println!("\n  Enter password for all {} treasury key files:", signers);
                    rpassword::read_password().expect("Failed to read password")
                });

            println!();
            for (i, kp) in keypairs.iter().enumerate() {
                let keyfile = format!("{}_key{}.qua", key_prefix, i);
                let w = QuantumWallet { keypair: kp.clone(), address: kp.get_address() };
                w.save_quantum_safe(&keyfile, &pwd).expect("Failed to save key file");
                println!("  Key {} saved : {} ({})", i, keyfile, kp.get_address());
            }
            println!("\n SECURITY CHECKLIST:");
            println!("  [ ] Distribute the {} key files to {} SEPARATE secure locations", signers, signers);
            println!("  [ ] Any 3 of {} keyholders can authorize a spend", signers);
        }

        Commands::TreasuryInfo { setup, node } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");
            let ts = TreasuryMultisigV2::from_json(&json).expect("Invalid treasury setup JSON");
            let bal = fetch_balance(&node, &ts.address).await;
            println!("\n Treasury Info");
            println!("  Address : {}", ts.address);
            println!("  Policy  : 3-of-{} Falcon-512 multisig", ts.public_keys.len());
            println!("  Balance : {:.6} QUA ({} microunits)", u_to_qua(bal), bal);
        }

        Commands::TreasuryPropose { setup, to, amount, fee, nonce, out } => {
            let json = std::fs::read_to_string(&setup).expect("Could not read treasury setup");
            let ts = TreasuryMultisigV2::from_json(&json).expect("Invalid treasury setup JSON");
            let proposal = ts.propose_spend(to.clone(), qua_to_u(amount), qua_to_u(fee), nonce, Utc::now().timestamp());
            let req_sigs = proposal.required_signatures;
            let total_keys = proposal.public_keys.len();
            std::fs::write(&out, proposal.to_json()).expect("Failed to save proposal");
            println!("\n Treasury proposal created: {}", out);
            println!("  To      : {}", to);
            println!("  Amount  : {:.6} QUA", amount);
            println!("\n  Sign with any {} of {} treasury keys:", req_sigs, total_keys);
            for i in 0..req_sigs {
                println!("   quanta-wallet treasury-sign --proposal {} --key {}_key{}.qua --index {}", out, "treasury", i, i);
            }
        }

        Commands::TreasurySign { proposal, key, index } => {
            let prop_json = std::fs::read_to_string(&proposal).expect("Could not read proposal");
            let mut prop = MultiSigTransaction::from_json(&prop_json).expect("Invalid proposal JSON");
            let pwd = read_password(&format!("Password for {}", key));
            let wallet = QuantumWallet::load_quantum_safe(&key, &pwd).expect("Failed to load key");
            #[allow(deprecated)]
            quanta::crypto::TreasuryMultisig::sign_proposal(&mut prop, index, &wallet.keypair)
                .expect("Failed to sign proposal");
            let (collected, required) = prop.signature_progress();
            std::fs::write(&proposal, prop.to_json()).expect("Failed to save signed proposal");
            println!("\n Signed with key {}. Signatures: {}/{}", index, collected, required);
            if prop.is_complete() {
                println!("  READY TO BROADCAST:");
                println!("   quanta-wallet treasury-broadcast --proposal {}", proposal);
            } else {
                println!("  Need {} more signature(s).", required - collected);
            }
        }

        Commands::TreasuryBroadcast { proposal, node } => {
            let json = std::fs::read_to_string(&proposal).expect("Could not read proposal");
            let prop = MultiSigTransaction::from_json(&json).expect("Invalid proposal JSON");
            let (col, req) = prop.signature_progress();
            if !prop.is_complete() { die(&format!("Proposal not complete: {}/{} signatures", col, req)); }
            if !prop.verify() { die("Signature verification failed — proposal is invalid!"); }
            match broadcast_tx(&node, &prop.base_tx).await {
                Ok(hash) => {
                    println!("\n Treasury transaction broadcast!");
                    println!("  TX Hash : {}", hash);
                    println!("  To      : {}", prop.base_tx.recipient);
                    println!("  Amount  : {:.6} QUA", u_to_qua(prop.base_tx.amount));
                }
                Err(e) => die(&format!("Broadcast failed: {}", e)),
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wallet loading helpers
// ─────────────────────────────────────────────────────────────────────────────

enum WalletKind {
    Hd(HDWallet),
    Raw(QuantumWallet),
    None(String),
}

struct SigningWallet {
    address: String,
    keypair: quanta::crypto::FalconKeypair,
}

/// Try to load a wallet file. Detects HD (.json) vs raw (.qua) automatically.
fn try_load_wallet(file: &str) -> WalletKind {
    let pwd = read_password("Enter wallet password");
    // Try HD wallet first (export_encrypted produces binary, import_encrypted takes &[u8])
    if let Ok(bytes) = std::fs::read(file) {
        if let Ok(w) = HDWallet::import_encrypted(&bytes, &pwd) {
            return WalletKind::Hd(w);
        }
    }
    // Fall back to raw wallet
    match QuantumWallet::load_quantum_safe(file, &pwd) {
        Ok(w)  => WalletKind::Raw(w),
        Err(e) => WalletKind::None(format!("Failed to load wallet '{}': {}", file, e)),
    }
}

/// Load the first signing keypair from either HD or raw wallet.
fn load_keypair_for_signing(file: &str) -> SigningWallet {
    let pwd = read_password("Enter wallet password");
    // Try HD wallet first
    if let Ok(bytes) = std::fs::read(file) {
        if let Ok(w) = HDWallet::import_encrypted(&bytes, &pwd) {
            if let Some(acc) = w.accounts.first() {
                if let Ok(kp) = w.get_keypair(0) {
                    return SigningWallet { address: acc.address.clone(), keypair: kp };
                }
            }
        }
    }
    // Fall back to raw wallet
    let w = QuantumWallet::load_quantum_safe(file, &pwd)
        .unwrap_or_else(|e| { die(&format!("Failed to load wallet: {}", e)) });
    SigningWallet { address: w.address.clone(), keypair: w.keypair }
}

// ─────────────────────────────────────────────────────────────────────────────
// Transaction builder helpers
// ─────────────────────────────────────────────────────────────────────────────

fn build_transfer(kp: &SigningWallet, to: &str, amount: f64, fee: f64, nonce: u64, payload: Vec<u8>) -> Transaction {
    let mut tx = Transaction {
        sender:     kp.address.clone(),
        recipient:  to.to_string(),
        amount:     qua_to_u(amount),
        timestamp:  Utc::now().timestamp(),
        signature:  vec![],
        public_key: kp.keypair.public_key.clone(),
        fee:        qua_to_u(fee),
        nonce,
        lock_time:  0,
        tx_type:    TransactionType::Transfer,
        sig_scheme: SignatureScheme::Falcon512,
        network_id: 0,
        payload,
    };
    let sig_bytes = tx.get_signing_bytes();
    tx.signature = kp.keypair.sign_transaction_canonical(&sig_bytes);
    tx
}

async fn broadcast_and_print(node: &str, tx: &Transaction, kind: &str, from: &str, to: &str, amount: f64, fee: f64) {
    match broadcast_tx(node, tx).await {
        Ok(hash) => {
            println!("\n {} submitted!", kind);
            println!("  TX Hash  : {}", hash);
            println!("  From     : {}", from);
            println!("  To       : {}", to);
            println!("  Amount   : {:.6} QUA", amount);
            println!("  Fee      : {:.6} QUA", fee);
        }
        Err(e) => die(&format!("{} failed: {}", kind, e)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// I/O helpers
// ─────────────────────────────────────────────────────────────────────────────

fn die(msg: &str) -> ! {
    eprintln!(" Error: {}", msg);
    std::process::exit(1);
}

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
                j["balance_microunits"].as_u64().or_else(|| j["balance"].as_u64()).unwrap_or(0)
            } else { 0 }
        }
        Err(_) => { eprintln!("  Warning: could not fetch balance from {}", node); 0 }
    }
}

async fn fetch_nonce(node: &str, address: &str) -> u64 {
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
    let resp = client.post(&url).json(tx).send().await.map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if status.is_success() {
        Ok(body["tx_hash"].as_str().unwrap_or("(unknown)").to_string())
    } else {
        Err(body["error"].as_str().unwrap_or("Unknown error").to_string())
    }
}
