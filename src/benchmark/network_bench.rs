/// Quanta PQC Benchmark — Live Node Network Stress Test
///
/// Fires N concurrent HTTP transactions at a running local node via the REST API.
/// Requires `--live-node` flag (node must be running on localhost:3000).
///
/// Measures:
///   - End-to-end transaction submission latency (p50/p95/p99)
///   - Concurrent throughput (reqwest async, tokio tasks)
///   - Mempool acceptance rate under flood
///   - Error rate breakdown (invalid sig / nonce / balance / rate-limit)

use std::time::Instant;
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};
use crate::benchmark::crypto_bench::stat;
use crate::crypto::signatures::FalconKeypair;
use crate::core::transaction::{Transaction, TransactionType, SignatureScheme};
use crate::core::TESTNET_NETWORK_ID;
use chrono::Utc;

/// Number of concurrent tasks used for flood test
const CONCURRENT_TASKS: usize = 10;

pub async fn run(node_url: &str, tx_count: usize) -> BenchmarkSection {
    println!("  [6/6] Live Node Network Stress Test → {}", node_url);
    println!("        Generating {} signed transactions...", tx_count);

    // Generate a fresh wallet with a known address for the test
    // NOTE: This wallet must have balance. For testnet, use a faucet wallet.
    // The benchmark will detect 0-balance rejections and report them separately.
    let kp = FalconKeypair::generate();
    let address = kp.get_address();
    println!("        Benchmark wallet: {}", address);
    println!("        Note: For full throughput test, fund this address via faucet.");

    // ── Latency test: sequential submissions ──────────────────────────────────
    println!("        Sequential latency test ({} txs)...", tx_count.min(50));
    let sequential_count = tx_count.min(50);
    let mut latency_samples: Vec<f64> = Vec::new();
    let mut success_count = 0usize;
    let mut error_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    for i in 0..sequential_count {
        let tx = build_test_tx(&kp, i as u64 + 1);
        let payload = serde_json::json!({
            "sender": tx.sender,
            "recipient": tx.recipient,
            "amount": tx.amount,
            "timestamp": tx.timestamp,
            "signature": hex::encode(&tx.signature),
            "public_key": hex::encode(&tx.public_key),
            "fee": tx.fee,
            "nonce": tx.nonce,
            "network_id": tx.network_id,
        });

        let url = format!("{}/api/transactions/submit", node_url);
        let t = Instant::now();
        let result = client.post(&url).json(&payload).send().await;
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        latency_samples.push(elapsed_ms);

        match result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    success_count += 1;
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    let key = extract_error_key(&body);
                    *error_counts.entry(key).or_insert(0) += 1;
                }
            }
            Err(e) => {
                let key = if e.is_connect() { "connection_refused".to_string() }
                          else if e.is_timeout() { "timeout".to_string() }
                          else { "network_error".to_string() };
                *error_counts.entry(key).or_insert(0) += 1;
            }
        }
    }

    let mut stats = Vec::new();

    if !latency_samples.is_empty() {
        let mut s = stat("Tx Submission Latency (sequential)", "ms/op", &latency_samples);
        s.note = Some(format!(
            "success={}/{} | errors: {:?}",
            success_count, sequential_count, error_counts
        ));
        stats.push(s);
    }

    // ── Concurrent flood test ─────────────────────────────────────────────────
    println!("        Concurrent flood test ({} tasks × {} txs)...", CONCURRENT_TASKS, tx_count / CONCURRENT_TASKS);
    let txs_per_task = (tx_count / CONCURRENT_TASKS).max(1);
    let flood_start = Instant::now();
    let mut handles = Vec::new();

    for task_id in 0..CONCURRENT_TASKS {
        let kp_clone = FalconKeypair::generate(); // each task uses own wallet
        let url = format!("{}/api/transactions/submit", node_url);
        let client_clone = client.clone();
        let base_nonce = (task_id * txs_per_task) as u64;

        let handle = tokio::spawn(async move {
            let mut task_success = 0usize;
            let mut task_errors = 0usize;
            for i in 0..txs_per_task {
                let tx = build_test_tx(&kp_clone, base_nonce + i as u64 + 1);
                let payload = serde_json::json!({
                    "sender": tx.sender,
                    "recipient": tx.recipient,
                    "amount": tx.amount,
                    "timestamp": tx.timestamp,
                    "signature": hex::encode(&tx.signature),
                    "public_key": hex::encode(&tx.public_key),
                    "fee": tx.fee,
                    "nonce": tx.nonce,
                    "network_id": tx.network_id,
                });
                match client_clone.post(&url).json(&payload).send().await {
                    Ok(r) if r.status().is_success() => task_success += 1,
                    _ => task_errors += 1,
                }
            }
            (task_success, task_errors)
        });
        handles.push(handle);
    }

    let mut total_success = 0usize;
    let mut total_errors = 0usize;
    for h in handles {
        if let Ok((s, e)) = h.await {
            total_success += s;
            total_errors += e;
        }
    }
    let flood_ms = flood_start.elapsed().as_secs_f64() * 1000.0;
    let total_txs = CONCURRENT_TASKS * txs_per_task;
    let concurrent_tps = total_txs as f64 / (flood_ms / 1000.0);

    stats.push(BenchmarkStat {
        name: format!("Concurrent Flood ({} tasks)", CONCURRENT_TASKS),
        unit: "tx/sec (end-to-end)".to_string(),
        iterations: total_txs,
        mean_ms: flood_ms / total_txs as f64,
        stddev_ms: 0.0,
        min: flood_ms / total_txs as f64,
        max: flood_ms / total_txs as f64,
        p50: flood_ms / total_txs as f64,
        p95: flood_ms / total_txs as f64,
        p99: flood_ms / total_txs as f64,
        throughput: Some(concurrent_tps),
        note: Some(format!(
            "success={} errors={} total={} tasks={} | {:.0} tx/sec end-to-end (includes API + verify + mempool)",
            total_success, total_errors, total_txs, CONCURRENT_TASKS, concurrent_tps
        )),
    });

    println!("        Flood result: {:.0} tx/sec  success={}  errors={}",
        concurrent_tps, total_success, total_errors);

    // ── Node health check ─────────────────────────────────────────────────────
    let health_url = format!("{}/api/stats", node_url);
    if let Ok(resp) = client.get(&health_url).send().await {
        if let Ok(json) = resp.json::<serde_json::Value>().await {
            let height = json.get("chain_length").and_then(|v| v.as_u64()).unwrap_or(0);
            let difficulty = json.get("current_difficulty").and_then(|v| v.as_u64()).unwrap_or(0);
            let mempool = json.get("pending_transactions").and_then(|v| v.as_u64()).unwrap_or(0);
            stats.push(BenchmarkStat {
                name: "Node Live State Snapshot".to_string(),
                unit: "info".to_string(),
                iterations: 1,
                mean_ms: 0.0,
                stddev_ms: 0.0,
                min: 0.0,
                max: 0.0,
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
                throughput: None,
                note: Some(format!(
                    "Chain height: {} blocks | Difficulty: {} | Mempool: {} pending | URL: {}",
                    height, difficulty, mempool, node_url
                )),
            });
            println!("        Live node: height={} difficulty={} mempool={}", height, difficulty, mempool);
        }
    }

    BenchmarkSection {
        name: "Live Node Network Stress Test".to_string(),
        description: format!(
            "End-to-end HTTP transaction stress test against a running Quanta node.\n\
             Node URL: {}\n\
             Sequential latency: measures API round-trip + Falcon-512 verify + mempool insert.\n\
             Concurrent flood: {} parallel tasks, measures sustained throughput.\n\
             Note: Results depend on node hardware, OS scheduler, and wallet funding.",
            node_url, CONCURRENT_TASKS
        ),
        stats,
    }
}

/// Offline placeholder when --live-node is not specified
pub fn run_skipped() -> BenchmarkSection {
    BenchmarkSection {
        name: "Live Node Network Stress Test".to_string(),
        description: "SKIPPED — run with --live-node http://localhost:3000 to enable.\n\
                      Requires a running Quanta node and a funded wallet on the testnet."
            .to_string(),
        stats: vec![BenchmarkStat {
            name: "Live Node Test".to_string(),
            unit: "n/a".to_string(),
            iterations: 0,
            mean_ms: 0.0,
            stddev_ms: 0.0,
            min: 0.0,
            max: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            throughput: None,
            note: Some("Use --live-node <url> to run this section".to_string()),
        }],
    }
}

fn build_test_tx(kp: &FalconKeypair, nonce: u64) -> Transaction {
    let mut tx = Transaction {
        sender: kp.get_address(),
        recipient: "0xbenchmark000000000000000000000000000000".to_string(),
        amount: 1_000,   // 0.001 QUA
        timestamp: Utc::now().timestamp(),
        signature: vec![],
        public_key: kp.public_key.clone(),
        fee: 1_000,
        nonce,
        lock_time: 0,
        tx_type: TransactionType::Transfer,
        sig_scheme: SignatureScheme::Falcon512,
        network_id: TESTNET_NETWORK_ID,
    };
    let signing_bytes = tx.get_signing_bytes();
    tx.signature = kp.sign_transaction_canonical(&signing_bytes);
    tx
}

fn extract_error_key(body: &str) -> String {
    if body.contains("insufficient") { "insufficient_balance".to_string() }
    else if body.contains("nonce") { "invalid_nonce".to_string() }
    else if body.contains("signature") { "invalid_signature".to_string() }
    else if body.contains("rate") { "rate_limited".to_string() }
    else if body.contains("mempool") { "mempool_full".to_string() }
    else { format!("other({})", &body[..body.len().min(40)]) }
}
