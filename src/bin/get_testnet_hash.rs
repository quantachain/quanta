/// get_testnet_hash — Print the current testnet genesis block hash.
///
/// Usage:
///   cargo run --bin get_testnet_hash
///
/// The genesis block hash is deterministic — it is computed from the
/// fixed fields in Block::genesis() (timestamp, previous_hash, merkle_root,
/// state_root, proposer, epoch, bft_round). It does NOT depend on faucet
/// addresses or validator keys (those go into AccountState, not the block).
///
/// Run this whenever block.rs changes to get the new hash to paste into
/// blockchain.rs as TESTNET_GENESIS_HASH.
use quanta::core::block::Block;

fn main() {
    let genesis = Block::genesis();

    println!();
    println!(" ╔══════════════════════════════════════════════════════════╗");
    println!(" ║        QUANTA TESTNET GENESIS HASH                      ║");
    println!(" ╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("  Genesis block index     : {}", genesis.index);
    println!("  Genesis block timestamp : {}", genesis.timestamp);
    println!("  Genesis block proposer  : {}", genesis.proposer);
    println!("  Genesis block epoch     : {}", genesis.epoch);
    println!("  Genesis block bft_round : {}", genesis.bft_round);
    println!("  Genesis merkle_root     : {}", genesis.merkle_root);
    println!("  Genesis state_root      : {}", genesis.state_root);
    println!();
    println!(" ┌──────────────────────────────────────────────────────────┐");
    println!(" │  TESTNET_GENESIS_HASH:                                   │");
    println!(" │  {}  │", genesis.hash);
    println!(" └──────────────────────────────────────────────────────────┘");
    println!();
    println!("  Paste this into src/consensus/blockchain.rs (~line 208):");
    println!();
    println!("  const TESTNET_GENESIS_HASH: &str = \"{}\";", genesis.hash);
    println!();

    // Sanity check: recompute to verify determinism
    let genesis2 = Block::genesis();
    if genesis.hash == genesis2.hash {
        println!("  ✓  Hash is deterministic (double-checked).");
    } else {
        eprintln!("  ✗  CRITICAL: Hash is NOT deterministic! Block::genesis() is broken.");
        std::process::exit(1);
    }
    println!();
}
