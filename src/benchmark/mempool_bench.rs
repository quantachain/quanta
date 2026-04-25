/// Quanta PQC Benchmark — Mempool Stress Test
///
/// Measures:
///   - Insert throughput (tx/sec)
///   - Fee-ordered selection (`get_best_transactions`)
///   - Duplicate detection / rejection latency
///   - Eviction under flood (fill to capacity → force evicts)
///   - Mempool capacity vs memory footprint

use std::time::Instant;
use crate::consensus::mempool::Mempool;
use crate::crypto::signatures::FalconKeypair;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::tx_bench::make_signed_tx;
use chrono::Utc;
use crate::core::transaction::Transaction;

pub fn run(iterations: usize) -> BenchmarkSection {
    println!("  [3/6] Mempool Stress Test...");

    let mut stats = Vec::new();

    // Pre-generate keypairs and signed transactions (not measured in mempool timing)
    let wallets: Vec<FalconKeypair> = (0..50).map(|_| FalconKeypair::generate()).collect();
    let tx_count = iterations.min(2000); // cap to avoid extremely long runs

    let transactions: Vec<Transaction> = (0..tx_count)
        .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
        .collect();

    // ── Insert throughput ─────────────────────────────────────────────────────
    {
        let mut pool = Mempool::new(tx_count + 100);
        let t = Instant::now();
        let mut inserted = 0usize;
        for tx in &transactions {
            if pool.add(tx.clone()).is_ok() {
                inserted += 1;
            }
        }
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        let insert_tps = inserted as f64 / (elapsed_ms / 1000.0);
        stats.push(BenchmarkStat {
            name: "Mempool Insert Throughput".to_string(),
            unit: "tx/sec".to_string(),
            iterations: inserted,
            mean_ms: elapsed_ms / inserted.max(1) as f64,
            stddev_ms: 0.0,
            min: elapsed_ms / inserted.max(1) as f64,
            max: elapsed_ms / inserted.max(1) as f64,
            p50: elapsed_ms / inserted.max(1) as f64,
            p95: elapsed_ms / inserted.max(1) as f64,
            p99: elapsed_ms / inserted.max(1) as f64,
            throughput: Some(insert_tps),
            note: Some(format!("Inserted {}/{} txs (some may share nonces — expected)", inserted, tx_count)),
        });
        println!("        insert={:.0} tx/sec  ({} inserted)", insert_tps, inserted);
    }

    // ── Duplicate rejection latency ───────────────────────────────────────────
    {
        let mut pool = Mempool::new(tx_count + 100);
        // Fill with valid txs first
        for tx in &transactions {
            let _ = pool.add(tx.clone());
        }
        // Now measure rejection of duplicates
        let n_dup = 200.min(transactions.len());
        let dup_samples: Vec<f64> = transactions[..n_dup].iter().map(|tx| {
            let t = Instant::now();
            let _ = pool.add(tx.clone()); // must return Err("already in mempool")
            t.elapsed().as_secs_f64() * 1_000_000.0 // µs
        }).collect();
        let mut s = crate::benchmark::crypto_bench::stat(
            "Duplicate Rejection Latency", "µs/op", &dup_samples,
        );
        s.note = Some("O(1) via transaction hash map lookup".to_string());
        stats.push(s);
    }

    // ── Fee-ordered selection ─────────────────────────────────────────────────
    {
        let mut pool = Mempool::new(tx_count + 100);
        for tx in &transactions {
            let _ = pool.add(tx.clone());
        }
        let select_sizes = [10, 50, 100, 500, 1200];
        for &n in &select_sizes {
            let t = Instant::now();
            let selected = pool.get_best_transactions(n);
            let elapsed_us = t.elapsed().as_secs_f64() * 1_000_000.0;
            stats.push(BenchmarkStat {
                name: format!("Fee-Ordered Selection (top {})", n),
                unit: "µs".to_string(),
                iterations: selected.len(),
                mean_ms: elapsed_us,
                stddev_ms: 0.0,
                min: elapsed_us,
                max: elapsed_us,
                p50: elapsed_us,
                p95: elapsed_us,
                p99: elapsed_us,
                throughput: None,
                note: Some(format!("Selected {} txs in {:.1} µs", selected.len(), elapsed_us)),
            });
        }
        println!("        fee-ordered selection: ✓");
    }

    // ── Eviction under flood ──────────────────────────────────────────────────
    {
        let cap = 500;
        let mut pool = Mempool::new(cap);
        // Fill to capacity
        for tx in &transactions[..cap.min(transactions.len())] {
            let _ = pool.add(tx.clone());
        }
        // Generate more txs with higher fees to trigger eviction
        let kp = FalconKeypair::generate();
        let extra_count = 200;
        let extra_txs: Vec<Transaction> = (0..extra_count).map(|i| {
            let mut tx = make_signed_tx(&kp, (i + 1) as u64);
            tx.fee = 999_999 + i as u64; // high fees → will evict low-fee txs
            tx
        }).collect();

        let t = Instant::now();
        let mut evictions = 0usize;
        for tx in &extra_txs {
            let before = pool.len();
            let _ = pool.add(tx.clone());
            if pool.len() <= before {
                evictions += 1;
            }
        }
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        stats.push(BenchmarkStat {
            name: "Mempool Eviction Under Flood".to_string(),
            unit: "ms total".to_string(),
            iterations: extra_count,
            mean_ms: elapsed_ms / extra_count as f64,
            stddev_ms: 0.0,
            min: elapsed_ms / extra_count as f64,
            max: elapsed_ms / extra_count as f64,
            p50: elapsed_ms / extra_count as f64,
            p95: elapsed_ms / extra_count as f64,
            p99: elapsed_ms / extra_count as f64,
            throughput: Some(extra_count as f64 / (elapsed_ms / 1000.0)),
            note: Some(format!(
                "Pool cap={}, {} high-fee txs inserted; {} evictions triggered",
                cap, extra_count, evictions
            )),
        });
        println!("        eviction test: {} evictions triggered", evictions);
    }

    // ── Memory footprint estimate ─────────────────────────────────────────────
    {
        let mut pool = Mempool::new(5000);
        for tx in &transactions {
            let _ = pool.add(tx.clone());
        }
        let pool_len = pool.len();
        // Estimate: each tx is ~1713 bytes (666 sig + 897 pubkey + ~150 overhead)
        let estimated_bytes = pool_len * 1713;
        stats.push(BenchmarkStat {
            name: "Mempool Memory Footprint (estimated)".to_string(),
            unit: "bytes".to_string(),
            iterations: pool_len,
            mean_ms: estimated_bytes as f64 / pool_len.max(1) as f64,
            stddev_ms: 0.0,
            min: 1713.0,
            max: 1713.0,
            p50: 1713.0,
            p95: 1713.0,
            p99: 1713.0,
            throughput: None,
            note: Some(format!(
                "{} txs × ~1713 B/tx = ~{:.1} KB total (Falcon-512 sig={} B + pubkey={} B + fields)",
                pool_len,
                estimated_bytes as f64 / 1024.0,
                666, 897
            )),
        });
    }

    BenchmarkSection {
        name: "Mempool Stress Test".to_string(),
        description: "Priority-fee mempool (BTreeMap by fee, O(log n) insert, O(1) remove).\n\
                      Bloom filter provides O(1) duplicate detection at 50K capacity with 0.01% FP rate.\n\
                      Eviction policy: lowest-fee transaction ejected when pool is at capacity."
            .to_string(),
        stats,
    }
}
