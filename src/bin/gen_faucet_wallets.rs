/// gen_faucet_wallets — Generate 10 HD faucet accounts for genesis testnet
///
/// No BIP39 passphrase is used — standard HD derivation (Falcon-512 is already PQC).
/// The wallet FILE is encrypted with a password you set via env var.
///
/// SECURITY: password is NEVER passed as a CLI argument (shell history risk).
///
///   Bash/WSL:
///     export FAUCET_WALLET_PASSWORD="your-file-encryption-password"
///     cargo run --bin gen_faucet_wallets
///
///   To restore from an existing mnemonic:
///     export FAUCET_MNEMONIC="word1 word2 ... word24"
///     export FAUCET_WALLET_PASSWORD="your-file-encryption-password"
///     cargo run --bin gen_faucet_wallets
///
/// Output:
///   - faucet_wallet.json  — encrypted with FAUCET_WALLET_PASSWORD
///   - 10 addresses ready to paste into blockchain.rs (~line 346 and ~line 485)
///   - Current TESTNET_GENESIS_HASH ready to paste into blockchain.rs (~line 208)

use quanta::crypto::HDWallet;
use quanta::core::block::Block;
use bip39::{Mnemonic, Language};
use rand::RngCore;

fn main() {
    println!();
    println!(" ╔══════════════════════════════════════════════════════════╗");
    println!(" ║        QUANTA FAUCET WALLET GENERATOR  v2               ║");
    println!(" ╚══════════════════════════════════════════════════════════╝");
    println!();

    // ── File encryption password (protects wallet.json on disk) ─────────────
    // This is NOT a BIP39 passphrase — it only encrypts the exported file.
    // The HD seed uses standard BIP39 derivation (no 25th word).
    let file_password = std::env::var("FAUCET_WALLET_PASSWORD").unwrap_or_else(|_| {
        eprintln!();
        eprintln!("  ERROR: FAUCET_WALLET_PASSWORD environment variable is not set.");
        eprintln!();
        eprintln!("  This password encrypts the faucet_wallet.json file on disk.");
        eprintln!("  It is NOT a BIP39 passphrase — Falcon-512 is already PQC.");
        eprintln!();
        eprintln!("  Bash/WSL:");
        eprintln!("    export FAUCET_WALLET_PASSWORD=\"your-file-password\"");
        eprintln!("    cargo run --bin gen_faucet_wallets");
        eprintln!();
        std::process::exit(1);
    });

    let existing_mnemonic = std::env::var("FAUCET_MNEMONIC").ok();

    // ── 1. Create or restore wallet (no BIP39 passphrase) ───────────────────
    // BIP39 passphrase = "" means standard HD derivation.
    // Falcon-512 keys are already post-quantum — no extra 25th word needed.
    let mut wallet = match existing_mnemonic {
        Some(m) => {
            let m = m.trim().to_string();
            println!(" Restoring HD wallet from FAUCET_MNEMONIC...");
            println!(" BIP39 passphrase: none (standard derivation)");
            println!();
            HDWallet::from_mnemonic(m, "")
        }
        None => {
            println!(" Generating NEW HD wallet mnemonic...");
            println!(" BIP39 passphrase: none (standard derivation)");
            println!();
            let mut entropy = [0u8; 32]; // 256 bits → 24 BIP39 words
            rand::thread_rng().fill_bytes(&mut entropy);
            let mnemonic = Mnemonic::from_entropy_in(Language::English, &entropy)
                .expect("entropy is always valid for BIP39");
            HDWallet::from_mnemonic(mnemonic.to_string(), "")
        }
    };

    // Generate 10 faucet accounts (indices 0–9)
    println!(" Generating 10 Falcon-512 faucet accounts...");
    for i in 0..10u32 {
        wallet.generate_account(Some(format!("Faucet {}", i)));
    }
    println!(" Done.");
    println!();

    // ── 2. Print mnemonic ────────────────────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 1 — Save your mnemonic (master key)               │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  {}", wallet.mnemonic);
    println!();
    println!("  → Copy to quanta-web/.env.local:");
    println!("    FAUCET_MNEMONIC=\"{}\"", wallet.mnemonic);
    println!();
    println!("  Account 0 = faucet sender (used by the faucet API)");
    println!();

    // ── 3. Print addresses for blockchain.rs ────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 2 — Paste into blockchain.rs                      │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  ── A) testnet_faucets vec (~line 346) ──");
    println!();
    println!("    let testnet_faucets = vec![");
    for account in wallet.get_accounts() {
        println!("        \"{}\",  // Faucet {}", account.address, account.index);
    }
    println!("    ];");
    println!();

    println!("  ── B) SELF-HEAL faucets array (~line 485) ──");
    println!();
    println!("    let faucets = [");
    for account in wallet.get_accounts() {
        println!("        \"{}\",", account.address);
    }
    println!("    ];");
    println!();

    // ── 4. Save encrypted wallet file ───────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 3 — Saving encrypted backup (faucet_wallet.json)  │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();

    let encrypted = wallet
        .export_encrypted(&file_password)
        .expect("Failed to encrypt wallet");

    std::fs::write("faucet_wallet.json", &encrypted)
        .expect("Failed to write faucet_wallet.json");

    println!("  ✓  Saved: faucet_wallet.json");
    println!("     Encrypted with FAUCET_WALLET_PASSWORD (Argon2 + ChaCha20-Poly1305).");
    println!("     Safe to store on server — useless without the password.");
    println!();

    // ── 5. Genesis hash ──────────────────────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 4 — TESTNET_GENESIS_HASH                          │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  The genesis block hash covers only block structural fields.");
    println!("  Faucet addresses go into AccountState (not the block itself),");
    println!("  so this hash is stable as long as block.rs is unchanged.");
    println!();

    let genesis = Block::genesis();
    println!("  ┌──────────────────────────────────────────────────────────────┐");
    println!("  │  {}  │", genesis.hash);
    println!("  └──────────────────────────────────────────────────────────────┘");
    println!();
    println!("  Paste into src/consensus/blockchain.rs (~line 208):");
    println!();
    println!("  const TESTNET_GENESIS_HASH: &str = \"{}\";", genesis.hash);
    println!();

    // ── 6. Checklist ─────────────────────────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  CHECKLIST                                               │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  [ ] Mnemonic saved to password manager");
    println!("  [ ] FAUCET_MNEMONIC set in quanta-web/.env.local");
    println!("  [ ] testnet_faucets vec replaced in blockchain.rs (A above)");
    println!("  [ ] SELF-HEAL faucets array replaced in blockchain.rs (B above)");
    println!("  [ ] TESTNET_GENESIS_HASH updated in blockchain.rs (~line 208)");
    println!("  [ ] faucet_wallet.json backed up securely");
    println!("  [ ] Old testnet database wiped (./quanta_testnet_data or similar)");
    println!("  [ ] cargo build --release && redeploy all nodes");
    println!();
}
