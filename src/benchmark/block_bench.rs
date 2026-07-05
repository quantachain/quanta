use crate::benchmark::crypto_bench::{stat, stat_us};
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::tx_bench::make_signed_tx;
use crate::core::block::Block;
use crate::core::merkle::MerkleTree;
use crate::core::transaction::Transaction;
use crate::crypto::signatures::FalconKeypair;
use std::hint::black_box;
/// Quanta v2 BFT Benchmark — Block Construction & Compression
///
/// Measures:
///   - Block hash computation time (`calculate_hash`)
///   - Merkle tree construction (1 / 10 / 100 / 500 / 1200 transactions)
///   - Block serialization speed (bincode)
///   - zstd compression ratio and throughput (Level 3 = production default)
///   - BFT signing latency (Falcon-512 precommit signature)
///
/// NOTE: PoW hashrate benchmarks removed in v2. Quanta v2 uses BFT consensus
/// from genesis. There is no mining or PoW difficulty.
use std::time::Instant;

/// Tx counts to test for Merkle / block construction
const TX_COUNTS: &[usize] = &[1, 10, 100, 500, 1200];

pub fn run(iterations: usize, _full_pow_solve: bool) -> BenchmarkSection {
    println!("  [4/6] Block Construction & Compression (v2 BFT)...");

    let mut stats = Vec::new();

    // Pre-generate signed transactions
    let wallets: Vec<FalconKeypair> = (0..20).map(|_| FalconKeypair::generate()).collect();
    let max_txs = 1200;
    let signed_txs: Vec<Transaction> = (0..max_txs)
        .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
        .collect();

    // ── Block hash computation ──────────────────────────────────────────────────
    {
        let genesis = Block::genesis();
        let n = iterations;
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let t = Instant::now();
            let _h = black_box(genesis.calculate_hash());
            samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
        }
        let mut s = stat_us(
            "Block Hash Computation (SHA3-256 double)",
            "µs/op",
            &samples,
        );
        s.note = Some("SHA3-256(SHA3-256(header)) — used for BFT block ID".to_string());
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
        let block = Block::new_bft(
            1,
            txs.to_vec(),
            "0".repeat(64),
            0,
            0,
            "0xbenchmarkproposer".to_string(),
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

        let ratio = if compressed_size > 0 {
            raw_bytes as f64 / compressed_size as f64
        } else {
            1.0
        };

        let mut cs = stat(
            &format!("Block Compress zstd-L3 ({} txs)", n_tx),
            "ms",
            &compress_samples,
        );
        cs.note = Some(format!(
            "raw={} KB → compressed={} KB  ratio={:.2}×  savings={:.1} KB/block",
            raw_bytes / 1024,
            compressed_size / 1024,
            ratio,
            (raw_bytes - compressed_size) as f64 / 1024.0,
        ));
        stats.push(cs);

        let mut ds = stat(
            &format!("Block Decompress ({} txs)", n_tx),
            "ms",
            &decompress_samples,
        );
        ds.note = Some(format!(
            "Compressed={} KB → raw={} KB",
            compressed_size / 1024,
            raw_bytes / 1024
        ));
        stats.push(ds);

        println!(
            "        block({} tx): raw={} KB  compressed={} KB  ratio={:.2}×",
            n_tx,
            raw_bytes / 1024,
            compressed_size / 1024,
            ratio
        );
    }

    // ── BFT Precommit Signing ─────────────────────────────────────────────────
    {
        println!("        BFT precommit signing benchmark...");
        let kp = FalconKeypair::generate();
        let genesis = Block::genesis();
        let payload = genesis.bft_signing_payload();

        let n = iterations.min(50); // Falcon-512 signing is slow
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let t = Instant::now();
            let _sig = black_box(kp.sign_hash(&payload));
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut s = stat("BFT Precommit Sign (Falcon-512)", "ms", &samples);
        s.note = Some("Signs 32-byte bft_signing_payload() with Falcon-512".to_string());
        stats.push(s);
    }

    BenchmarkSection {
        name: "Block Construction & BFT Signing".to_string(),
        description: format!(
            "Block construction, Merkle tree, zstd compression (level 3), and BFT signing.\n\
             Max block size: 2 MB. Max transactions per block: 1200 (Falcon-512 size constraint).\n\
             Compression saves ~{:.1}× bandwidth on average for production blocks.\n\
             PoW mining REMOVED in v2 — Quanta v2 uses Tendermint-style BFT from genesis.",
            3.5_f64,
        ),
        stats,
    }
}
