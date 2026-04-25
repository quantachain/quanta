/// Quanta PQC Benchmark — Transaction Throughput
///
/// Measures:
///   - Unsigned transaction build rate
///   - Serial sign throughput (transactions/second)
///   - Serial verify throughput
///   - Parallel verify throughput (rayon, all physical cores)
///   - Parallel speedup factor
///   - Transaction wire size (serialized bincode, min/max/mean)

use std::time::Instant;
use crate::crypto::signatures::FalconKeypair;
use crate::core::transaction::{Transaction, TransactionType, SignatureScheme};
use crate::core::TESTNET_NETWORK_ID;
use crate::consensus::performance::verify_transactions_parallel;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::crypto_bench::stat;
use rayon::prelude::*;
use chrono::Utc;

/// Batch sizes to test — mirrors realistic network conditions.
const BATCH_SIZES: &[usize] = &[50, 100, 500, 1_000, 2_000];

pub fn run(iterations: usize) -> BenchmarkSection {
    println!("  [2/6] Transaction Throughput...");

    // Generate keypairs once — key generation is not what we're measuring here
    let num_wallets = 20;
    let wallets: Vec<FalconKeypair> = (0..num_wallets).map(|_| FalconKeypair::generate()).collect();

    let mut stats = Vec::new();

    // ── Build throughput ──────────────────────────────────────────────────────
    {
        let n = iterations;
        let t = Instant::now();
        for i in 0..n {
            let kp = &wallets[i % wallets.len()];
            let _tx = make_unsigned_tx(kp, i as u64 + 1);
        }
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        let tps = n as f64 / (elapsed_ms / 1000.0);
        stats.push(BenchmarkStat {
            name: "Transaction Build (unsigned)".to_string(),
            unit: "tx/sec".to_string(),
            iterations: n,
            mean_ms: elapsed_ms / n as f64,
            stddev_ms: 0.0,
            min: elapsed_ms / n as f64,
            max: elapsed_ms / n as f64,
            p50: elapsed_ms / n as f64,
            p95: elapsed_ms / n as f64,
            p99: elapsed_ms / n as f64,
            throughput: Some(tps),
            note: Some("Unsigned tx construction only — no crypto".to_string()),
        });
    }

    // ── Per-batch serial sign + verify + parallel verify ─────────────────────
    for &batch in BATCH_SIZES {
        // Build a batch of fully-signed transactions
        let signed_txs: Vec<Transaction> = (0..batch)
            .map(|i| make_signed_tx(&wallets[i % wallets.len()], i as u64 + 1))
            .collect();

        // Wire size
        let sizes: Vec<f64> = signed_txs.iter()
            .filter_map(|tx| bincode::serialize(tx).ok())
            .map(|b| b.len() as f64)
            .collect();
        if !sizes.is_empty() {
            let size_stat = stat(
                &format!("Tx Wire Size (batch {})", batch),
                "bytes",
                &sizes,
            );
            stats.push(size_stat);
        }

        // Serial sign — build fresh txs and sign them
        let sign_ms = {
            let kp = &wallets[0];
            let data = b"serial-sign-benchmark-payload";
            let t = Instant::now();
            for _ in 0..batch {
                let _sig = kp.sign_transaction_canonical(data);
            }
            t.elapsed().as_secs_f64() * 1000.0
        };
        let sign_tps = batch as f64 / (sign_ms / 1000.0);

        // Serial verify
        let verify_t = Instant::now();
        for tx in &signed_txs {
            let _ = tx.verify();
        }
        let serial_verify_ms = verify_t.elapsed().as_secs_f64() * 1000.0;
        let serial_verify_tps = batch as f64 / (serial_verify_ms / 1000.0);

        // Parallel verify (rayon)
        let par_t = Instant::now();
        let _all_valid = verify_transactions_parallel(&signed_txs);
        let par_verify_ms = par_t.elapsed().as_secs_f64() * 1000.0;
        let par_verify_tps = batch as f64 / (par_verify_ms / 1000.0);

        let speedup = if par_verify_ms > 0.0 { serial_verify_ms / par_verify_ms } else { 1.0 };
        let cores = num_cpus::get_physical().max(1);

        stats.push(BenchmarkStat {
            name: format!("Sign TPS (serial, batch={})", batch),
            unit: "tx/sec".to_string(),
            iterations: batch,
            mean_ms: sign_ms / batch as f64,
            stddev_ms: 0.0,
            min: sign_ms / batch as f64,
            max: sign_ms / batch as f64,
            p50: sign_ms / batch as f64,
            p95: sign_ms / batch as f64,
            p99: sign_ms / batch as f64,
            throughput: Some(sign_tps),
            note: None,
        });

        stats.push(BenchmarkStat {
            name: format!("Verify TPS (serial, batch={})", batch),
            unit: "tx/sec".to_string(),
            iterations: batch,
            mean_ms: serial_verify_ms / batch as f64,
            stddev_ms: 0.0,
            min: serial_verify_ms / batch as f64,
            max: serial_verify_ms / batch as f64,
            p50: serial_verify_ms / batch as f64,
            p95: serial_verify_ms / batch as f64,
            p99: serial_verify_ms / batch as f64,
            throughput: Some(serial_verify_tps),
            note: None,
        });

        stats.push(BenchmarkStat {
            name: format!("Verify TPS (parallel/{} cores, batch={})", cores, batch),
            unit: "tx/sec".to_string(),
            iterations: batch,
            mean_ms: par_verify_ms / batch as f64,
            stddev_ms: 0.0,
            min: par_verify_ms / batch as f64,
            max: par_verify_ms / batch as f64,
            p50: par_verify_ms / batch as f64,
            p95: par_verify_ms / batch as f64,
            p99: par_verify_ms / batch as f64,
            throughput: Some(par_verify_tps),
            note: Some(format!(
                "Speedup vs serial: {:.2}×  (theoretical max: {}×)",
                speedup, cores
            )),
        });

        println!("        batch={:5}  sign={:.0} tps  verify_serial={:.0} tps  verify_parallel={:.0} tps  speedup={:.2}×",
            batch, sign_tps, serial_verify_tps, par_verify_tps, speedup);
    }

    BenchmarkSection {
        name: "Transaction Throughput".to_string(),
        description: format!(
            "End-to-end Falcon-512 transaction sign/verify performance.\n\
             Parallel verification uses Rayon with {} physical cores.\n\
             Wire sizes use bincode binary encoding (as transmitted over P2P).\n\
             Batch sizes tested: {:?}",
            num_cpus::get_physical(),
            BATCH_SIZES,
        ),
        stats,
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_unsigned_tx(kp: &FalconKeypair, nonce: u64) -> Transaction {
    let mut tx = Transaction::new(
        kp.get_address(),
        "0xbenchmark000000000000000000000000000000".to_string(),
        1_000_000,  // 1 QUA
        Utc::now().timestamp(),
    );
    tx.nonce = nonce;
    tx.public_key = kp.public_key.clone();
    tx
}

pub(crate) fn make_signed_tx(kp: &FalconKeypair, nonce: u64) -> Transaction {
    let mut tx = make_unsigned_tx(kp, nonce);
    let signing_bytes = tx.get_signing_bytes();
    tx.signature = kp.sign_transaction_canonical(&signing_bytes);
    tx
}
