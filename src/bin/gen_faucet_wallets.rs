/// gen_faucet_wallets — Generate 10 HD accounts for genesis testnet faucet wallets
///
/// Usage:
///   cargo run --bin gen_faucet_wallets
///   cargo run --bin gen_faucet_wallets -- "your 24 word mnemonic phrase here"
///
/// Output:
///   - faucet_wallet.json  (encrypted backup — safe to store on server)
///   - Mnemonic printed to terminal (write it down → .env.local)
///   - 10 addresses to paste into blockchain.rs genesis
use quanta::crypto::HDWallet;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    println!();
    println!(" ╔══════════════════════════════════════════════════════════╗");
    println!(" ║        QUANTA FAUCET WALLET GENERATOR                   ║");
    println!(" ╚══════════════════════════════════════════════════════════╝");
    println!();

    // ── 1. Create or restore wallet ────────────────────────────────────────
    let mut wallet = if args.len() > 1 {
        let mnemonic = args[1..].join(" ");
        println!(" Restoring HD wallet from provided mnemonic...\n");
        HDWallet::from_mnemonic(mnemonic, "")
    } else {
        println!(" Generating NEW HD wallet mnemonic...\n");
        HDWallet::new()
    };

    // Generate 10 accounts (indices 0–9)
    for i in 0..10u32 {
        wallet.generate_account(Some(format!("Faucet {}", i)));
    }

    // ── 2. Print mnemonic ──────────────────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 1 — Write down your mnemonic (your master key)    │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  {}", wallet.mnemonic);
    println!();
    println!("  → Copy this to quanta-web/.env.local:");
    println!("    FAUCET_MNEMONIC=\"{}\"", wallet.mnemonic);
    println!();
    println!("  Account 0 = faucet sender (the API uses index 0 by default)");
    println!();

    // ── 3. Print addresses for blockchain.rs ──────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 2 — Paste into blockchain.rs (testnet_faucets)    │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("    let testnet_faucets = vec![");
    for account in wallet.get_accounts() {
        println!("        \"{}\",  // Faucet {}", account.address, account.index);
    }
    println!("    ];");
    println!();

    // ── 4. Save encrypted wallet file (Monero-style backup) ───────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  STEP 3 — Saving encrypted backup (faucet_wallet.json)  │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  This file is useless without the password.");
    println!("  Safe to keep on the server as a recovery backup.\n");

    let password = loop {
        println!("  Enter password for faucet_wallet.json:");
        let p1 = rpassword::read_password().expect("Failed to read password");
        println!("  Confirm password:");
        let p2 = rpassword::read_password().expect("Failed to read password");
        if p1 == p2 {
            if p1.len() < 8 {
                println!("  ⚠  Password too short (min 8 chars), try again.\n");
                continue;
            }
            break p1;
        }
        println!("  ✗  Passwords don't match, try again.\n");
    };

    let encrypted = wallet
        .export_encrypted(&password)
        .expect("Failed to encrypt wallet");

    let out_path = "faucet_wallet.json";
    std::fs::write(out_path, &encrypted).expect("Failed to save faucet_wallet.json");

    println!();
    println!("  ✓  Saved: {}", out_path);
    println!();

    // ── 5. After-run checklist ─────────────────────────────────────────────
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  CHECKLIST                                               │");
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  [ ] Mnemonic written down / stored in password manager");
    println!("  [ ] FAUCET_MNEMONIC set in quanta-web/.env.local");
    println!("  [ ] 10 addresses pasted into blockchain.rs testnet_faucets");
    println!("  [ ] faucet_wallet.json backed up to a safe location");
    println!("  [ ] Re-run: cargo run --bin get_testnet_hash");
    println!("  [ ] Update TESTNET_GENESIS_HASH in blockchain.rs");
    println!("  [ ] Delete old testnet database (./quanta_testnet_data)");
    println!();
    println!("  To view wallet later:");
    println!("    quanta-wallet new-hd  (restore with same mnemonic)");
    println!("  Or load backup:");
    println!("    cargo run --bin gen_faucet_wallets -- \"<your mnemonic>\"");
    println!();
}
