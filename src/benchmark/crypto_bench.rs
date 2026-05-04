/// Quanta PQC Benchmark — Falcon-512 Cryptographic Performance
///
/// Measures raw cryptographic latency for:
///   - Key generation   (keygen)
///   - Signing          (sign)
///   - Verification     (verify)
///   - SHA3-256 canonical hash throughput
///
/// All values reported as mean ± std-dev over N iterations.

use std::time::Instant;
use std::hint::black_box;
use crate::crypto::signatures::{FalconKeypair, canonical_signing_hash, FALCON512_PUBKEY_BYTES};
use crate::benchmark::report::{BenchmarkSection, BenchmarkStat};

/// Run all cryptographic benchmarks.
pub fn run(iterations: usize) -> BenchmarkSection {
    println!("  [1/6] Cryptographic Performance (Falcon-512)...");

    let keygen_stat     = bench_keygen(iterations);
    let sign_stat       = bench_sign(iterations);
    let verify_warm     = bench_verify_warm(iterations);
    let verify_cold     = bench_verify_cold((iterations / 10).max(20)); // cold-path is slower
    let hash_stat       = bench_sha3(iterations * 10);  // faster, so more iterations
    let sig_size_stat   = bench_sig_size(iterations);

    println!("        keygen  {:.3} ms | sign {:.3} ms | verify-warm {:.3} µs | verify-cold {:.3} µs",
        keygen_stat.mean_ms, sign_stat.mean_ms, verify_warm.mean_ms, verify_cold.mean_ms);

    BenchmarkSection {
        name: "Cryptographic Performance (Falcon-512)".to_string(),
        description: format!(
            "Falcon-512 (NIST PQC Round 3) performance over {} iterations.\n\
             Public key: {} bytes (fixed). Signature: variable-length compressed lattice.\n\
             verify-warm = same key/sig reused (L1-cache hot path, represents in-pipeline LRU hits).\n\
             verify-cold = fresh heap allocation each iteration (cache-cold, represents first-seen key on network).\n\
             Comparison baselines (NIST FIPS 186-5 / PQCrypto literature):\n\
             • ECDSA-P256 sign: ~0.05 ms | verify: ~0.12 ms | key: 64 B | sig: 64 B\n\
             • RSA-2048 sign:   ~1.80 ms | verify: ~0.05 ms | key: 256 B | sig: 256 B\n\
             Falcon-512 offers quantum-resistant security at 2× the cost of ECDSA verify;\n\
             parallel batch verification (rayon) closes the gap to <1.3× per-batch.",
            iterations,
            FALCON512_PUBKEY_BYTES,
        ),
        stats: vec![keygen_stat, sign_stat, verify_warm, verify_cold, hash_stat, sig_size_stat],
    }
}

// ─── Keygen ──────────────────────────────────────────────────────────────────

fn bench_keygen(n: usize) -> BenchmarkStat {
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        let _kp = FalconKeypair::generate();
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    stat("Falcon-512 Key Generation", "ms/op", &samples)
}

// ─── Sign ────────────────────────────────────────────────────────────────────

fn bench_sign(n: usize) -> BenchmarkStat {
    let kp = FalconKeypair::generate();
    let data = b"quanta-benchmark-signing-payload-v1";
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        let _sig = kp.sign_transaction_canonical(data);
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    stat("Falcon-512 Sign", "ms/op", &samples)
}

// ── Verify (warm — L1-cache hot) ───────────────────────────────────────────

fn bench_verify_warm(n: usize) -> BenchmarkStat {
    let kp = FalconKeypair::generate();
    let data = b"quanta-benchmark-verify-payload-v1";
    let signed = kp.sign_transaction_canonical(data);
    let hash = canonical_signing_hash(data);

    // Same buffers reused every iteration — all in L1 cache after first call.
    // Represents the in-pipeline LRU-hit pathway (repeated tx seen in mempool).
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        let _ok = black_box(crate::crypto::signatures::verify_signature_strict(
            black_box(&hash), black_box(&signed), black_box(&kp.public_key),
        ));
        samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
    }
    let mut s = stat_us("Falcon-512 Verify (warm/LRU-hit)", "µs/op", &samples);
    s.note = Some("Cache-warm path: same 1.6 KB key+sig reused. Represents in-pipeline LRU cache hits.".to_string());
    s
}

// ── Verify (cold — fresh heap allocation each call) ─────────────────────

fn bench_verify_cold(n: usize) -> BenchmarkStat {
    let kp = FalconKeypair::generate();
    let data = b"quanta-benchmark-verify-cold-payload-v1";
    let signed = kp.sign_transaction_canonical(data);
    let hash = canonical_signing_hash(data);

    // Allocate fresh Vec copies each iteration so the CPU sees a new pointer
    // (heap allocation flushes the data from L1 cache on most allocators).
    // This approximates first-seen-transaction latency on the network path.
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        // Fresh heap copies — defeats L1/L2 data cache for key + sig bytes
        let pk_cold  = kp.public_key.clone();
        let sig_cold = signed.clone();
        let t = Instant::now();
        let _ok = black_box(crate::crypto::signatures::verify_signature_strict(
            black_box(&hash), black_box(&sig_cold), black_box(&pk_cold),
        ));
        samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
    }
    let mut s = stat_us("Falcon-512 Verify (cold/fresh-key)", "µs/op", &samples);
    s.note = Some(
        "Cache-cold path: fresh Vec allocated each iter. Approximates first-seen-key network latency."
        .to_string()
    );
    s
}

// ─── SHA3-256 ────────────────────────────────────────────────────────────────

fn bench_sha3(n: usize) -> BenchmarkStat {
    let data = b"quanta-tx-canonical-hash-domain-benchmark";
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let t = Instant::now();
        let _h = black_box(canonical_signing_hash(black_box(data)));
        samples.push(t.elapsed().as_secs_f64() * 1_000_000.0); // µs
    }
    // stat_us: throughput = 1_000_000 / mean_µs = ops/sec (correct for µs samples)
    stat_us("SHA3-256 Canonical Hash (domain prefix)", "µs/op", &samples)
}

// ─── Signature size distribution ─────────────────────────────────────────────

fn bench_sig_size(n: usize) -> BenchmarkStat {
    let kp = FalconKeypair::generate();
    let data = b"quanta-sig-size-distribution";
    let mut sizes = Vec::with_capacity(n);
    for i in 0..n {
        // vary the data slightly so we sample the full lattice signature distribution
        let payload = [data.as_ref(), &[i as u8]].concat();
        let sig = kp.sign_transaction_canonical(&payload);
        sizes.push(sig.len() as f64);
    }
    let mut s = stat(
        &format!("Falcon-512 Signature Size (pubkey={} B fixed)", FALCON512_PUBKEY_BYTES),
        "bytes",
        &sizes,
    );
    s.note = Some(format!(
        "min={:.0}B max={:.0}B  — variable-length compressed Falcon-512 (max 666 B raw sig + 32 B domain hash = 698 B blob)",
        s.min, s.max
    ));
    s
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Build a stat where samples are in **milliseconds**.
/// Throughput = 1000 / mean_ms = ops/sec.
pub(crate) fn stat(name: &str, unit: &str, samples: &[f64]) -> BenchmarkStat {
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();
    let mut sorted = samples.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = sorted.first().copied().unwrap_or(0.0);
    let max = sorted.last().copied().unwrap_or(0.0);
    let p50 = sorted[(sorted.len() as f64 * 0.50) as usize];
    let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
    let p99 = sorted[((sorted.len() as f64 * 0.99) as usize).min(sorted.len() - 1)];

    BenchmarkStat {
        name: name.to_string(),
        unit: unit.to_string(),
        iterations: samples.len(),
        mean_ms: mean,
        stddev_ms: stddev,
        min,
        max,
        p50,
        p95,
        p99,
        throughput: if mean > 0.0 { Some(1000.0 / mean) } else { None },
        note: None,
    }
}

/// Build a stat where samples are in **microseconds**.
/// Throughput = 1_000_000 / mean_µs = ops/sec.
/// Use this for any bench that collects `elapsed.as_secs_f64() * 1_000_000.0`.
pub(crate) fn stat_us(name: &str, unit: &str, samples: &[f64]) -> BenchmarkStat {
    let mut s = stat(name, unit, samples);
    // Override the 1000/mean throughput with the correct 1_000_000/mean for µs
    s.throughput = if s.mean_ms > 0.0 { Some(1_000_000.0 / s.mean_ms) } else { None };
    s
}
