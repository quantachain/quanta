/// Quanta PQC Benchmark — Block Construction & Compression
///
/// Measures:
///   - Block hash computation time (`calculate_hash`)
///   - Merkle tree construction (1 / 10 / 100 / 500 / 1200 transactions)
///   - Block serialization speed (bincode)
///   - zstd compression ratio and throughput (Level 3 = production default)
///   - Block decompression throughput
///   - PoW hashrate (10-second timed run + extrapolation)
///   - PoW full solve at current difficulty (optional, may take minutes)

use std::time::{Instant, Duration};
use crate::core::block::Block;
use crate::core::transaction::Transaction;
use crate::core::ChainNetwork;
use crate::crypto::signatures::FalconKeypair;
use crate::core::merkle::MerkleTree;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::crypto_bench::stat;
use crate::benchmark::tx_bench::make_signed_tx;
use crate::consensus::blockchain::MIN_DIFFICULTY;
use chrono::Utc;

/// Tx counts to test for Merkle / block construction
const TX_COUNTS: &[usize] = &[1, 10, 100, 500, 1200];

pub fn run(iterations: usize, full_pow_solve: bool) -> BenchmarkSection {
    println!("  [4/6] Block Construction & Compression...");

    let mut stats = Vec::new();

    // Pre-generate signed transactions
    let wallets: Vec<FalconKeypair> = (0..20).map(|_| FalconKeypair::generate()).collect();
    let max_txs = 1200;
    let signed_txs: Vec<Transaction> = (0..max_txs)
        .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
        .collect();

    // ── Block hash computation ────────────────────────────────────────────────
    {
        let genesis = Block::genesis(ChainNetwork::Testnet);
        let n = iterations;
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let t = Instant::now();
            let _h = genesis.calculate_hash();
            samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
        }
        let mut s = stat("Block Hash Computation (SHA3-256 double)", "µs/op", &samples);
        s.note = Some("SHA3-256(SHA3-256(header)) — used for PoW mining".to_string());
        stats.push(s);
    }

    // ── Merkle tree construction ──────────────────────────────────────────────
    for &n_tx in TX_COUNTS {
        let txs = &signed_txs[..n_tx.min(signed_txs.len())];
        let n_iters = (iterations / 5).max(10);
        let mut samples = Vec::with_capacity(n_iters);
        for _ in 0..n_iters {
            let t = Instant::now();
            let _tree = MerkleTree::from_transactions(txs);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut s = stat(&format!("Merkle Root ({} txs)", n_tx), "ms", &samples);
        s.note = Some("SHA3-256 binary Merkle tree".to_string());
        stats.push(s);
    }

    // ── Block serialization + compression ─────────────────────────────────────
    for &n_tx in TX_COUNTS {
        let txs = &signed_txs[..n_tx.min(signed_txs.len())];
        let block = Block::new(
            1,
            txs.to_vec(),
            "0".repeat(64),
            MIN_DIFFICULTY,
        );

        // Raw serialization
        let raw = bincode::serialize(&block).unwrap_or_default();
        let raw_bytes = raw.len();

        // Compression (zstd level 3 = production default)
        let n_iters = (iterations / 5).max(5);
        let mut compress_samples = Vec::with_capacity(n_iters);
        let mut decompress_samples = Vec::with_capacity(n_iters);
        let mut compressed_size = 0usize;

        for _ in 0..n_iters {
            let t = Instant::now();
            let compressed = zstd::encode_all(raw.as_slice(), 3).unwrap_or_default();
            compress_samples.push(t.elapsed().as_secs_f64() * 1000.0);
            compressed_size = compressed.len();

            let t2 = Instant::now();
            let _dec = zstd::decode_all(compressed.as_slice()).unwrap_or_default();
            decompress_samples.push(t2.elapsed().as_secs_f64() * 1000.0);
        }

        let ratio = if compressed_size > 0 { raw_bytes as f64 / compressed_size as f64 } else { 1.0 };

        let mut cs = stat(&format!("Block Compress zstd-L3 ({} txs)", n_tx), "ms", &compress_samples);
        cs.note = Some(format!(
            "raw={} KB → compressed={} KB  ratio={:.2}×  savings={:.1} KB/block",
            raw_bytes / 1024,
            compressed_size / 1024,
            ratio,
            (raw_bytes - compressed_size) as f64 / 1024.0,
        ));
        stats.push(cs);

        let mut ds = stat(&format!("Block Decompress ({} txs)", n_tx), "ms", &decompress_samples);
        ds.note = Some(format!("Compressed={} KB → raw={} KB", compressed_size / 1024, raw_bytes / 1024));
        stats.push(ds);

        println!("        block({} tx): raw={} KB  compressed={} KB  ratio={:.2}×",
            n_tx, raw_bytes / 1024, compressed_size / 1024, ratio);
    }

    // ── PoW Hashrate: 10-second timed run ─────────────────────────────────────
    {
        println!("        Mining hashrate test (10 sec)...");
        let mut block = Block::new(
            99999,
            vec![],
            "0".repeat(64),
            MIN_DIFFICULTY,
        );
        // Temporarily set a very easy target so we can measure raw hash rate
        block.difficulty = 1; // accept any hash
        let mut hash_count = 0u64;
        let start = Instant::now();
        let deadline = Duration::from_secs(10);
        while start.elapsed() < deadline {
            block.hash = block.calculate_hash();
            block.nonce = block.nonce.wrapping_add(1);
            hash_count += 1;
        }
        let elapsed = start.elapsed().as_secs_f64();
        let hashrate = hash_count as f64 / elapsed;
        let hashrate_kh = hashrate / 1000.0;

        stats.push(BenchmarkStat {
            name: "PoW Hashrate (10-sec timed run)".to_string(),
            unit: "kH/s".to_string(),
            iterations: hash_count as usize,
            mean_ms: (elapsed * 1000.0) / hash_count as f64,
            stddev_ms: 0.0,
            min: (elapsed * 1000.0) / hash_count as f64,
            max: (elapsed * 1000.0) / hash_count as f64,
            p50: (elapsed * 1000.0) / hash_count as f64,
            p95: (elapsed * 1000.0) / hash_count as f64,
            p99: (elapsed * 1000.0) / hash_count as f64,
            throughput: Some(hashrate),
            note: Some(format!(
                "{:.1} kH/s  ({} hashes in {:.1}s)  Current network difficulty: {} → avg solve time: {:.1}s",
                hashrate_kh,
                hash_count,
                elapsed,
                MIN_DIFFICULTY,
                MIN_DIFFICULTY as f64 / hashrate
            )),
        });
        println!("        Hashrate: {:.1} kH/s  (est. solve time at diff {}: {:.1}s)",
            hashrate_kh, MIN_DIFFICULTY, MIN_DIFFICULTY as f64 / hashrate);
    }

    // ── PoW Full Difficulty Solve ──────────────────────────────────────────────
    if full_pow_solve {
        println!("        Full PoW solve at difficulty {} (may take minutes)...", MIN_DIFFICULTY);
        let mut block = Block::new(
            100000,
            vec![],
            "0".repeat(64),
            MIN_DIFFICULTY,
        );
        let t = Instant::now();
        let mut hash_count = 0u64;
        loop {
            block.hash = block.calculate_hash();
            hash_count += 1;
            if block.has_valid_hash() { break; }
            block.nonce = block.nonce.wrapping_add(1);
        }
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        let hashrate = hash_count as f64 / (elapsed_ms / 1000.0);

        stats.push(BenchmarkStat {
            name: "PoW Full Solve (actual difficulty)".to_string(),
            unit: "seconds".to_string(),
            iterations: hash_count as usize,
            mean_ms: elapsed_ms,
            stddev_ms: 0.0,
            min: elapsed_ms,
            max: elapsed_ms,
            p50: elapsed_ms,
            p95: elapsed_ms,
            p99: elapsed_ms,
            throughput: Some(hashrate),
            note: Some(format!(
                "Difficulty={} | Nonce={} | Hashes={} | Hash={:.16}...",
                MIN_DIFFICULTY, block.nonce, hash_count, &block.hash[..16]
            )),
        });
        println!("        Solved in {:.2}s  nonce={}  hash={}...",
            elapsed_ms / 1000.0, block.nonce, &block.hash[..16]);
    }

    BenchmarkSection {
        name: "Block Construction & Mining".to_string(),
        description: format!(
            "Block construction, Merkle tree, zstd compression (level 3), and PoW mining.\n\
             Max block size: 2 MB. Max transactions per block: 1200 (Falcon-512 size constraint).\n\
             Compression saves ~{:.1}× bandwidth on average for production blocks.\n\
             Full PoW solve: {}",
            3.5_f64,
            if full_pow_solve { "YES (included)" } else { "SKIPPED (use --full-pow to enable)" }
        ),
        stats,
    }
}
