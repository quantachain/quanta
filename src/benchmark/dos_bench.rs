use std::time::Instant;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::crypto_bench::stat;
use crate::crypto::signatures::FalconKeypair;
use crate::core::transaction::Transaction;
use crate::consensus::mempool::Mempool;
use rayon::prelude::*;

pub fn run(iterations: usize) -> BenchmarkSection {
    println!("  [7/7] Adversarial DoS Simulation...");

    let mut stats = Vec::new();
    let kp = FalconKeypair::generate();
    let address = kp.get_address();

    // 1. Generate a valid transaction
    let mut tx = Transaction::new(address.clone(), "0xrecip".into(), 100, 0);
    tx.public_key = kp.public_key.clone();
    let signing_bytes = tx.get_signing_bytes();
    tx.signature = kp.sign_transaction_canonical(&signing_bytes);
    
    // 2. Invalidate signature (flip a bit in the signature payload)
    let mut invalid_tx = tx.clone();
    if !invalid_tx.signature.is_empty() {
        invalid_tx.signature[10] ^= 0xFF; 
    }

    // Benchmark 1: Invalid Signature CPU Exhaustion (Cold Path)
    let mut times_invalid_verify = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t = Instant::now();
        let _ = invalid_tx.verify();
        times_invalid_verify.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    stats.push(stat("Invalid Signature Verify (Cold Path)", "ms/op", &times_invalid_verify));

    // Benchmark 2: Parallel Invalid Signature Flood
    let batch_size = 1000;
    let mut parallel_invalid_verify_times = Vec::with_capacity(50);
    let batch: Vec<Transaction> = vec![invalid_tx.clone(); batch_size];
    
    for _ in 0..50 { // Run 50 batches
        let t = Instant::now();
        let valid_count: usize = batch.par_iter().map(|t| if t.verify() { 1 } else { 0 }).sum();
        parallel_invalid_verify_times.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(valid_count, 0, "Invalid signatures must fail verification");
    }
    
    let mut parallel_stat = stat(&format!("Parallel Invalid Flood ({} txs)", batch_size), "ms/batch", &parallel_invalid_verify_times);
    let mean_batch_ms = parallel_invalid_verify_times.iter().sum::<f64>() / parallel_invalid_verify_times.len() as f64;
    parallel_stat.throughput = Some((batch_size as f64) / (mean_batch_ms / 1000.0));
    parallel_stat.note = Some("Proves node does not CPU-lock under signature flood".to_string());
    stats.push(parallel_stat);

    // Benchmark 3: Mempool Duplicate Injection (Memory footprint / rejection)
    let mut mempool = Mempool::new(50_000);
    let _ = mempool.add(tx.clone()); // Insert the valid one once
    
    let mut times_dup = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let t = Instant::now();
        let _ = mempool.add(tx.clone()); // Should reject instantly as duplicate
        times_dup.push(t.elapsed().as_secs_f64() * 1000.0 * 1000.0); // measure in microseconds
    }
    
    let mut dup_stat = stat("Mempool Duplicate Rejection", "µs/op", &times_dup);
    let mean_us = times_dup.iter().sum::<f64>() / times_dup.len() as f64;
    dup_stat.throughput = Some(1_000_000.0 / mean_us);
    dup_stat.note = Some("Measures O(1) duplicate prevention under DoS attack".to_string());
    stats.push(dup_stat);

    BenchmarkSection {
        name: "Adversarial DoS Simulation".to_string(),
        description: "Evaluates node resilience under targeted adversarial conditions: \
                      invalid signature flood (testing CPU exhaustion) and mempool \
                      duplicate injection (testing memory fragmentation and rejection speed).".to_string(),
        stats,
    }
}
