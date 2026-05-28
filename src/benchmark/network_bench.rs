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
use crate::core::transaction::{Transaction, TransactionType, SignatureScheme};
use crate::core::TESTNET_NETWORK_ID;
use chrono::Utc;
use falcon_rust::falcon512::{self as fr, SecretKey as FrSK};
use sha3::{Sha3_256, Digest};

/// Domain tag — must match SIGNING_DOMAIN in signatures.rs and quanta-wasm.
const SIGNING_DOMAIN: &[u8] = b"QUANTA_TX_V1:";

/// Sign with falcon-rust (NOT pqcrypto) — this is the format verify_signature_strict() expects.
/// Output: sig.to_bytes() || hash  (last 32 bytes = the hash that was signed)
fn sign_with_falcon_rust(sk_bytes: &[u8], signing_data: &[u8]) -> Vec<u8> {
    // Step 1: domain-separated hash
    let mut h = Sha3_256::new();
    h.update(SIGNING_DOMAIN);
    h.update(signing_data);
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&h.finalize());

    // Step 2: sign the hash with falcon-rust
    let sk = FrSK::from_bytes(sk_bytes).expect("benchmark: invalid falcon-rust SK");
    let sig = fr::sign(&hash, &sk);
    let sig_bytes = sig.to_bytes();

    // Step 3: blob = sig_bytes || hash  (verify_signature_strict splits last 32 B)
    let mut blob = Vec::with_capacity(sig_bytes.len() + 32);
    blob.extend_from_slice(&sig_bytes);
    blob.extend_from_slice(&hash);
    blob
}

/// Derive address from raw public key bytes (SHA3-256, first 20 bytes, 0x-prefixed).
fn address_from_pubkey(pk: &[u8]) -> String {
    let hash = Sha3_256::digest(pk);
    format!("0x{}", hex::encode(&hash[..20]))
}

/// Number of concurrent tasks used for flood test
const CONCURRENT_TASKS: usize = 10;

/// Load (pk_bytes, sk_bytes) from a faucet wallet export JSON file.
/// Returns None if the file can't be read or the index is out of bounds.
fn load_wallet_from_file(path: &str, index: usize) -> Option<(Vec<u8>, Vec<u8>)> {
    let contents = std::fs::read_to_string(path).ok()?;
    let wallets: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let entry = wallets.get(index)?;
    let pk = hex::decode(entry["public_key_hex"].as_str()?).ok()?;
    let sk = hex::decode(entry["secret_key_hex"].as_str()?).ok()?;
    // Verify falcon-rust can parse the SK before returning
    FrSK::from_bytes(&sk).ok()?;
    Some((pk, sk))
}

pub async fn run(node_url: &str, tx_count: usize, wallet_file: Option<&str>, wallet_index: usize) -> BenchmarkSection {
    println!("  [6/6] Live Node Network Stress Test → {}", node_url);

    // Load wallet: (pk_bytes, sk_bytes) from file, or generate a throwaway key.
    let (pk_bytes, sk_bytes) = if let Some(path) = wallet_file {
        match load_wallet_from_file(path, wallet_index) {
            Some(pair) => {
                println!("        Loaded wallet index {} from {} (falcon-rust compatible)", wallet_index, path);
                pair
            }
            None => {
                println!("        ⚠️  Could not load wallet from {} (index {}), generating throwaway key", path, wallet_index);
                let kp = crate::crypto::signatures::FalconKeypair::generate();
                let pk = kp.public_key.clone();
                let sk = kp.secret_key_bytes().to_vec();
                (pk, sk)
            }
        }
    } else {
        println!("        No --wallet-file given. Generating throwaway wallet.");
        println!("        Tip: use --wallet-file faucet_accounts_export.json for real txs.");
        let kp = crate::crypto::signatures::FalconKeypair::generate();
        let pk = kp.public_key.clone();
        let sk = kp.secret_key_bytes().to_vec();
        (pk, sk)
    };
    let address = address_from_pubkey(&pk_bytes);
    println!("        Benchmark wallet: {}", address);

    // Fetch the current on-chain nonce so sequential nonces don’t collide.
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();
    let start_nonce = {
        let url = format!("{}/api/balance/{}", node_url, address);
        if let Ok(resp) = client.get(&url).send().await {
            if let Ok(j) = resp.json::<serde_json::Value>().await {
                j["nonce"].as_u64().unwrap_or(0)
            } else { 0 }
        } else { 0 }
    };
    println!("        On-chain nonce: {} — submitting from nonce {}", start_nonce, start_nonce + 1);

    let sequential_count = tx_count.min(10); // 10 × 120ms = 1.2s, well within 10 req/sec
    println!("        Sequential latency test ({} txs at 120ms spacing)...", sequential_count);
    let mut latency_samples: Vec<f64> = Vec::new();
    let mut success_count = 0usize;
    let mut error_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();


    for i in 0..sequential_count {
        // Sleep FIRST — ensures we stay under the 10 req/sec rate limit before each request.
        // At 120ms spacing = ~8 req/sec, well within the node’s 10/sec per-IP limit.
        if i > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;
        }

        let tx = build_test_tx(&pk_bytes, &sk_bytes, &address, start_nonce + i as u64 + 1);

        // Local sanity check: verify before sending.
        if !tx.verify() {
            eprintln!("        ⚠️  [CHECK 1] local verify FAILED at nonce={} — signing bug!", start_nonce + i as u64 + 1);
        }

        let url = format!("{}/api/transactions/submit", node_url);
        let t = Instant::now();
        let result = client.post(&url).json(&tx).send().await;
        let elapsed_ms = t.elapsed().as_secs_f64() * 1000.0;
        latency_samples.push(elapsed_ms);

        match result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    success_count += 1;
                } else {
                    // Check 429 by numeric code BEFORE reading body (body is empty for rate limit)
                    let key = if status.as_u16() == 429 {
                        "rate_limited".to_string()
                    } else {
                        let body = resp.text().await.unwrap_or_default();
                        extract_error_key(&body)
                    };
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
        let pk_clone = pk_bytes.clone();
        let sk_clone = sk_bytes.clone();
        let addr_clone = address.clone();
        let base_nonce = start_nonce + (sequential_count as u64) + (task_id * txs_per_task) as u64;
        let url = format!("{}/api/transactions/submit", node_url);
        let client_clone = client.clone();

        let handle = tokio::spawn(async move {
            let mut task_success = 0usize;
            let mut task_errors = 0usize;
            for i in 0..txs_per_task {
                let tx = build_test_tx(&pk_clone, &sk_clone, &addr_clone, base_nonce + i as u64 + 1);
                match client_clone.post(&url).json(&tx).send().await {
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
            let epoch = json.get("current_epoch").and_then(|v| v.as_u64()).unwrap_or(0);
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
                    "Chain height: {} blocks | Epoch: {} | Mempool: {} pending | URL: {}",
                    height, epoch, mempool, node_url
                )),
            });
            println!("        Live node: height={} epoch={} mempool={}", height, epoch, mempool);
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

fn build_test_tx(pk_bytes: &[u8], sk_bytes: &[u8], sender: &str, nonce: u64) -> Transaction {
    // Use faucet index 1 as recipient — guaranteed to exist on chain.
    // The 0xdead burn address has no account state entry and some nodes reject it.
    let recipient = if sender == "0x1683be267318d2ddd8cee8df4a4548dcffb1e088" {
        "0xd528c18ce7a8844e4a4dcd841975b20ae599b020" // faucet 1
    } else {
        "0x1683be267318d2ddd8cee8df4a4548dcffb1e088" // faucet 0
    };
    let mut tx = Transaction {
        sender: sender.to_string(),
        recipient: recipient.to_string(),
        amount: 1_000,
        timestamp: Utc::now().timestamp(),
        signature: vec![],
        public_key: pk_bytes.to_vec(),
        fee: 1_000,
        nonce,
        lock_time: 0,
        tx_type: TransactionType::Transfer,
        sig_scheme: SignatureScheme::Falcon512,
        network_id: TESTNET_NETWORK_ID,
    };
    // Sign with falcon-rust (NOT pqcrypto) to match verify_signature_strict()
    tx.signature = sign_with_falcon_rust(sk_bytes, &tx.get_signing_bytes());
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
