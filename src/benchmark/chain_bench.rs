/// Quanta PQC Benchmark — Chain Validation & State
///
/// Measures:
///   - State root computation at 1K / 10K / 50K accounts
///   - `unlock_mature_coinbase` throughput (locked balance scan)
///   - Block `is_valid()` pipeline timing
///   - Parallel verify speedup vs serial (realistic block of 1200 txs)
///   - LRU signature cache hit-rate simulation
///   - Transaction hash throughput (mempool dedup path)

use std::time::Instant;
use crate::core::block::Block;
use crate::core::transaction::{Transaction, AccountState, AccountBalance, LockedBalance};
use crate::core::ChainNetwork;
use crate::crypto::signatures::FalconKeypair;
use crate::consensus::performance::verify_transactions_parallel;
use crate::consensus::blockchain::MIN_DIFFICULTY;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::crypto_bench::stat;
use crate::benchmark::tx_bench::make_signed_tx;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::Mutex;

pub fn run(iterations: usize) -> BenchmarkSection {
    println!("  [5/6] Chain Validation & State...");

    let mut stats = Vec::new();

    // ── State root computation at various account counts ──────────────────────
    for &n_accounts in &[1_000usize, 10_000, 50_000] {
        let state = build_account_state(n_accounts);
        let n_iters = (iterations / 10).max(5);
        let mut samples = Vec::with_capacity(n_iters);
        for _ in 0..n_iters {
            let t = Instant::now();
            let _root = state.calculate_state_root();
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut s = stat(&format!("State Root ({} accounts)", n_accounts), "ms", &samples);
        s.note = Some(format!(
            "SHA3-256 over sorted {} addresses + balances + nonces — deterministic across all nodes",
            n_accounts
        ));
        stats.push(s);
        println!("        state_root({} accounts): {:.2} ms", n_accounts, samples.iter().sum::<f64>() / samples.len() as f64);
    }

    // ── Coinbase unlock throughput ─────────────────────────────────────────────
    {
        let mut state = build_account_state_with_locked(10_000);
        let n_iters = (iterations / 5).max(10);
        let mut samples = Vec::with_capacity(n_iters);
        for i in 0..n_iters {
            let t = Instant::now();
            state.unlock_mature_coinbase(100_000 + i as u64); // height well past unlock
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        let mut s = stat("Coinbase Unlock (10K locked entries)", "ms", &samples);
        s.note = Some("Called once per block; scans and unlocks matured coinbase rewards".to_string());
        stats.push(s);
    }

    // ── Block is_valid() pipeline ─────────────────────────────────────────────
    {
        let genesis = Block::genesis(ChainNetwork::Testnet);
        // Build a simple valid child block (no txs, easy target for timing)
        let mut child = Block::new(1, vec![], genesis.hash.clone(), 1);
        child.mine(); // mine with difficulty=1 (instant)

        let n_iters = iterations.min(200);
        let mut samples = Vec::with_capacity(n_iters);
        for _ in 0..n_iters {
            let t = Instant::now();
            let _valid = child.is_valid(Some(&genesis));
            samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
        }
        let mut s = stat("Block Validation Pipeline (is_valid)", "µs/op", &samples);
        s.note = Some("Hash integrity + PoW + Merkle root + chain linkage (excludes tx sig verify)".to_string());
        stats.push(s);
    }

    // ── Parallel vs serial verify on realistic block ──────────────────────────
    {
        println!("        Building 1200-tx block for parallel verify benchmark (slow — signing)...");
        let wallets: Vec<FalconKeypair> = (0..30).map(|_| FalconKeypair::generate()).collect();
        let n_tx = 200; // 200 is representative without being too slow for keygen
        let txs: Vec<Transaction> = (0..n_tx)
            .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
            .collect();

        // Serial
        let t_serial = Instant::now();
        for tx in &txs { let _ = tx.verify(); }
        let serial_ms = t_serial.elapsed().as_secs_f64() * 1000.0;

        // Parallel
        let t_par = Instant::now();
        let _ = verify_transactions_parallel(&txs);
        let par_ms = t_par.elapsed().as_secs_f64() * 1000.0;

        let cores = num_cpus::get_physical().max(1);
        let speedup = if par_ms > 0.0 { serial_ms / par_ms } else { 1.0 };
        let efficiency = speedup / cores as f64 * 100.0;

        stats.push(BenchmarkStat {
            name: format!("Block Verify Serial ({} txs)", n_tx),
            unit: "ms".to_string(),
            iterations: n_tx,
            mean_ms: serial_ms,
            stddev_ms: 0.0,
            min: serial_ms,
            max: serial_ms,
            p50: serial_ms,
            p95: serial_ms,
            p99: serial_ms,
            throughput: Some(n_tx as f64 / (serial_ms / 1000.0)),
            note: None,
        });
        stats.push(BenchmarkStat {
            name: format!("Block Verify Parallel/{} cores ({} txs)", cores, n_tx),
            unit: "ms".to_string(),
            iterations: n_tx,
            mean_ms: par_ms,
            stddev_ms: 0.0,
            min: par_ms,
            max: par_ms,
            p50: par_ms,
            p95: par_ms,
            p99: par_ms,
            throughput: Some(n_tx as f64 / (par_ms / 1000.0)),
            note: Some(format!(
                "Speedup: {:.2}×  Core efficiency: {:.1}%  (theoretical max: {}×)",
                speedup, efficiency, cores
            )),
        });
        println!("        verify({} txs): serial={:.1}ms  parallel={:.1}ms  speedup={:.2}×",
            n_tx, serial_ms, par_ms, speedup);
    }

    // ── LRU cache hit-rate simulation ─────────────────────────────────────────
    {
        let wallets: Vec<FalconKeypair> = (0..5).map(|_| FalconKeypair::generate()).collect();
        let txs: Vec<Transaction> = (0..50)
            .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
            .collect();

        let cache: Mutex<LruCache<String, bool>> =
            Mutex::new(LruCache::new(NonZeroUsize::new(10_000).unwrap()));

        // Simulate 80% repeat verification (realistic mempool re-broadcast)
        let n_sim = 500;
        let mut cache_hits = 0usize;
        let mut cache_misses = 0usize;
        let t_cached = Instant::now();
        for i in 0..n_sim {
            let tx = &txs[i % txs.len()];
            let hash = tx.hash();
            let mut c = cache.lock().unwrap();
            if let Some(&result) = c.get(&hash) {
                cache_hits += 1;
                let _ = result;
            } else {
                cache_misses += 1;
                let result = tx.verify();
                c.put(hash, result);
            }
        }
        let cached_ms = t_cached.elapsed().as_secs_f64() * 1000.0;
        let hit_rate = cache_hits as f64 / n_sim as f64 * 100.0;

        stats.push(BenchmarkStat {
            name: "LRU Signature Cache Simulation".to_string(),
            unit: "ms total".to_string(),
            iterations: n_sim,
            mean_ms: cached_ms / n_sim as f64,
            stddev_ms: 0.0,
            min: cached_ms / n_sim as f64,
            max: cached_ms / n_sim as f64,
            p50: cached_ms / n_sim as f64,
            p95: cached_ms / n_sim as f64,
            p99: cached_ms / n_sim as f64,
            throughput: Some(n_sim as f64 / (cached_ms / 1000.0)),
            note: Some(format!(
                "{} ops: {} hits ({:.1}%) + {} misses ({:.1}%) — cache saves {:.0}% of Falcon verify cost",
                n_sim, cache_hits, hit_rate, cache_misses, 100.0 - hit_rate, hit_rate
            )),
        });
    }

    // ── Transaction hash throughput (mempool dedup) ───────────────────────────
    {
        let wallets: Vec<FalconKeypair> = (0..5).map(|_| FalconKeypair::generate()).collect();
        let txs: Vec<Transaction> = (0..100)
            .map(|i| make_signed_tx(&wallets[i % wallets.len()], (i / wallets.len() + 1) as u64))
            .collect();

        let n_iters = iterations;
        let t = Instant::now();
        for i in 0..n_iters {
            let _ = txs[i % txs.len()].hash();
        }
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        stats.push(BenchmarkStat {
            name: "Transaction Hash (SHA3-256, mempool dedup)".to_string(),
            unit: "µs/op".to_string(),
            iterations: n_iters,
            mean_ms: (elapsed_ms / n_iters as f64) * 1000.0,
            stddev_ms: 0.0,
            min: (elapsed_ms / n_iters as f64) * 1000.0,
            max: (elapsed_ms / n_iters as f64) * 1000.0,
            p50: (elapsed_ms / n_iters as f64) * 1000.0,
            p95: (elapsed_ms / n_iters as f64) * 1000.0,
            p99: (elapsed_ms / n_iters as f64) * 1000.0,
            throughput: Some(n_iters as f64 / (elapsed_ms / 1000.0)),
            note: Some("Covers all tx fields except signature — used for Merkle leaves & mempool IDs".to_string()),
        });
    }

    BenchmarkSection {
        name: "Chain Validation & State".to_string(),
        description: format!(
            "State root, coinbase unlock, block validation, parallel signature verification, \
             LRU cache simulation, and tx hash throughput.\n\
             Rayon thread pool: {} physical cores.",
            num_cpus::get_physical()
        ),
        stats,
    }
}

// ─── State builders ───────────────────────────────────────────────────────────

fn build_account_state(n: usize) -> AccountState {
    let mut state = AccountState::new();
    for i in 0..n {
        let addr = format!("0x{:040x}", i);
        let tx = crate::core::transaction::Transaction {
            sender: "GENESIS".to_string(),
            recipient: addr,
            amount: 1_000_000_000,
            timestamp: 0,
            signature: vec![],
            public_key: vec![],
            fee: 0,
            nonce: 0,
            lock_time: 0,
            tx_type: crate::core::transaction::TransactionType::Transfer,
            sig_scheme: crate::core::transaction::SignatureScheme::Falcon512,
            network_id: 0,
        };
        state.credit_account(&tx, 0, 0);
    }
    state
}

fn build_account_state_with_locked(n: usize) -> AccountState {
    // Use the public API only — create base state then we'll test unlock behavior
    let mut state = AccountState::new();
    for i in 0..n {
        let addr = format!("0x{:040x}", i);
        // Add as locked coinbase via the add_locked_balance API
        let unlock_height = (i as u64 % 200) + 50; // varies, half already matured
        state.add_locked_balance(&addr, 100_000_000, unlock_height);
    }
    state
}
