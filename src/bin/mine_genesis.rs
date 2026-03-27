/// Mine a new testnet genesis block.
///
/// Workflow:
///   1. Benchmarks SHA3-256 double-hash speed on this machine
///   2. Calculates the difficulty needed for exactly 30-second block time
///   3. Mines a valid genesis nonce
///   4. Prints the three constants to paste into src/core/block.rs
///
/// Run with:
///   cargo run --release --bin mine_genesis

use sha3::{Digest, Sha3_256};
use std::time::Instant;

// ── Genesis parameters ──────────────────────────────────────────────────────

/// Timestamp: 2026-04-01 00:00:00 UTC (Alpha V2 Testnet relaunch)
/// Change this to any future Unix timestamp if you want a different launch date.
const GENESIS_TIMESTAMP: i64 = 1774483200; // 2026-03-26 00:00:00 UTC

/// Target block time in seconds
const TARGET_BLOCK_SECS: u64 = 30;

/// How many seconds to benchmark the hashrate before mining.
/// Longer = more accurate difficulty, but takes longer to start.
const BENCHMARK_SECS: u64 = 5;

// ── Hash function (must match src/crypto/signatures.rs::double_sha3) ────────

fn double_sha3(data: &[u8]) -> String {
    let first  = Sha3_256::digest(data);
    let second = Sha3_256::digest(&first);
    hex::encode(second)
}

fn block_hash(timestamp: i64, nonce: u64, difficulty: u32) -> String {
    // Must match Block::calculate_hash() in src/core/block.rs exactly.
    // Genesis has no transactions, previous_hash is 64 zeros,
    // merkle_root and state_root are 64 zeros.
    let data = format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        0u64,                          // index
        timestamp,
        "",                            // no transactions → tx_hashes_joined = ""
        "0".repeat(64),               // previous_hash
        nonce,
        difficulty,
        "0".repeat(64),               // merkle_root
        "0".repeat(64),               // state_root
    );
    double_sha3(data.as_bytes())
}

fn meets_target(hash: &str, difficulty: u32) -> bool {
    if hash.len() < 16 { return false; }
    let prefix = u64::from_str_radix(&hash[..16], 16).unwrap_or(u64::MAX);
    let target  = if difficulty == 0 { u64::MAX } else { u64::MAX / difficulty as u64 };
    prefix <= target
}

// ── Main ─────────────────────────────────────────────────────────────────────

fn main() {
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  QUANTA Testnet Genesis Miner");
    println!("  Target block time: {} seconds", TARGET_BLOCK_SECS);
    println!("══════════════════════════════════════════════════");
    println!();

    // ── Step 1: benchmark hashrate ───────────────────────────────────────
    println!("Step 1 — Benchmarking SHA3 double-hash speed for {} seconds...", BENCHMARK_SECS);
    let bench_start = Instant::now();
    let bench_limit = std::time::Duration::from_secs(BENCHMARK_SECS);
    let mut bench_nonce: u64 = 0;

    while bench_start.elapsed() < bench_limit {
        // Use a trivially easy difficulty so every nonce "passes" - pure speed benchmark
        let _ = block_hash(GENESIS_TIMESTAMP, bench_nonce, 1);
        bench_nonce += 1;
    }

    let elapsed = bench_start.elapsed().as_secs_f64();
    let hashrate = bench_nonce as f64 / elapsed;
    println!("  Hashrate: {:.0} H/s ({:.2}s elapsed, {} hashes)", hashrate, elapsed, bench_nonce);
    println!();

    // ── Step 2: calculate difficulty for 30-second block time ────────────
    // difficulty = hashrate × target_block_time
    // (difficulty IS the expected number of hashes to find a valid nonce)
    let difficulty = (hashrate * TARGET_BLOCK_SECS as f64) as u64;
    // Clamp to u32 range (MAX_DIFFICULTY in blockchain.rs is 2^31-1)
    let difficulty = difficulty.min(2_147_483_647) as u32;

    println!("Step 2 — Calculated genesis difficulty:");
    println!("  difficulty   = {} hashes needed on average", difficulty);
    println!("  expected time ≈ {:.1}s per block on this machine", difficulty as f64 / hashrate);
    println!();

    // ── Step 3: mine the genesis block ───────────────────────────────────
    println!("Step 3 — Mining genesis block (difficulty = {})...", difficulty);
    println!("  This should take ~30 seconds by design.\n");

    let mine_start = Instant::now();
    let mut nonce: u64 = 0;
    let mut hash  = String::new();

    loop {
        hash = block_hash(GENESIS_TIMESTAMP, nonce, difficulty);
        if meets_target(&hash, difficulty) {
            break;
        }
        nonce += 1;
        if nonce % 500_000 == 0 {
            let secs = mine_start.elapsed().as_secs_f64();
            let rate  = nonce as f64 / secs;
            eprintln!("  ... {:.1}s  {:.0}M hashes  {:.0} H/s",
                secs, nonce as f64 / 1_000_000.0, rate);
        }
    }

    let mine_secs = mine_start.elapsed().as_secs_f64();
    println!();
    println!("══════════════════════════════════════════════════");
    println!("  GENESIS BLOCK FOUND!  (took {:.1}s, {} hashes)", mine_secs, nonce);
    println!("══════════════════════════════════════════════════");
    println!();
    println!("Paste these values into  src/core/block.rs  (Testnet arm):");
    println!();
    println!("  crate::core::ChainNetwork::Testnet => ({}, {}, {}),",
        GENESIS_TIMESTAMP, difficulty, nonce);
    println!();
    println!("And update TESTNET_GENESIS_HASH:");
    println!();
    println!("  const TESTNET_GENESIS_HASH: &str = \"{}\";", hash);
    println!();
    println!("Notes:");
    println!("  • Difficulty {} means ~30s blocks at {:.0} H/s (this machine)", difficulty, hashrate);
    println!("  • Difficulty will auto-adjust every 2016 blocks to compensate");
    println!("    for faster/slower machines on the network.");
    println!("  • If your miners are much faster, the next adjustment upward");
    println!("    will happen at block 2016 (takes ~{:.0} real minutes to reach).",
        difficulty as f64 / hashrate * 2016.0 / 60.0);
    println!();
}
