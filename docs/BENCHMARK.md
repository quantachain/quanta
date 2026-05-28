# Quanta PQC Benchmark Suite — Usage Guide

**Version:** 0.7.1  
**Target Audience:** Developers, Researchers, Government and Institutional Evaluators

---

## Overview

The Quanta benchmark suite measures every critical performance metric of the node's
post-quantum cryptographic stack. It runs entirely standalone — no live node is required
for the offline sections. Results are written as a JSON file (machine-readable) and a
Markdown file (human-readable) to the output directory.

---

## Building

Build the benchmark binary in release mode. Release mode is mandatory for accurate
results — debug builds are 10–50x slower due to disabled optimizations.

```bash
cd /mnt/e/temp/quanta
cargo build --release --bin quanta-benchmark
```

The compiled binary will be at:

```
target/release/quanta-benchmark
```

---

## Running — Command Reference

### Standard Run (Recommended)

500 iterations per test. Offline only. No full PoW solve. Takes approximately 3–5 minutes.

```bash
./target/release/quanta-benchmark
```

### Full Publication Run

1000 iterations per test. Includes the full PoW difficulty solve (may take 5–15 minutes
depending on hardware). This is the run to use when producing results for a paper or
institutional report.

```bash
./target/release/quanta-benchmark --iterations 1000 --full-pow
```

### Full Run With Live Node Stress Test

Requires a running Quanta node on localhost. Start the node first in a separate terminal,
then run the benchmark with the `--live-node` flag. The live test fires real signed
transactions at the node's HTTP API and measures end-to-end latency and throughput.

```bash
# Terminal 1: start the node
./target/release/quanta start

# Terminal 2: run benchmark with live node
./target/release/quanta-benchmark --iterations 1000 --full-pow --live-node http://localhost:3000
```

### Quick Sanity Check

100 iterations, no PoW solve, no live node. Use this to confirm the binary works before
committing to a long run. Takes approximately 30–60 seconds.

```bash
./target/release/quanta-benchmark --quick
```

### JSON Output Only

Skips the Markdown report. Useful in CI pipelines where only the structured data is needed.

```bash
./target/release/quanta-benchmark --json-only
```

### Custom Output Directory

```bash
./target/release/quanta-benchmark --output-dir /path/to/results
```

---

## All Options

| Flag | Type | Default | Description |
|---|---|---|---|
| `--iterations N` | integer | 500 | Iterations per micro-benchmark |
| `--output-dir PATH` | string | `./benchmark_results` | Output directory for reports |
| `--full-pow` | flag | off | Run a full PoW block solve at current difficulty |
| `--live-node URL` | string | (none) | URL of a running node for HTTP stress test |
| `--live-txs N` | integer | 100 | Number of transactions for the live node test |
| `--json-only` | flag | off | Write JSON only, skip Markdown |
| `--quick` | flag | off | 100 iterations, no full-PoW, no live node |
| `--help` | flag | — | Print help and exit |

---

## Alternative: Using the Node CLI

The benchmark is also available as a subcommand of the main `quanta` binary.
This is equivalent to the standalone binary.

```bash
./target/release/quanta benchmark --iterations 1000 --full-pow
```

With live node:

```bash
./target/release/quanta benchmark --iterations 1000 --full-pow --live-node http://localhost:3000
```

---

## Output Files

Both files are written to the output directory (default: `./benchmark_results/`).

```
benchmark_results/
  quanta_benchmark_2026-04-25_17-30-00.json
  quanta_benchmark_2026-04-25_17-30-00.md
```

### JSON File

Machine-readable. Contains all raw measurements: mean, standard deviation, min, max,
p50, p95, p99, throughput (ops/sec), and system information (CPU model, core count,
RAM, OS, Rust version). Use this for regression tracking, CI comparisons, and
automated data pipelines.

### Markdown File

Human-readable. Suitable for direct inclusion in technical papers, institutional
presentations, and GitHub. Contains formatted tables for every benchmark section,
a PQC algorithm comparison table (Falcon-512 vs ECDSA vs RSA vs Dilithium), and
a methodology and references section.

---

## What Is Measured

### Section 1 — Cryptographic Performance (Falcon-512)

- Key generation latency (mean, std dev, p95, p99 over N iterations)
- Signing latency
- Verification latency
- SHA3-256 canonical hash throughput (domain-prefixed, as used in production)
- Signature size distribution (min, max, mean — Falcon-512 is variable-length)

### Section 2 — Transaction Throughput

- Unsigned transaction build rate
- Serial sign throughput (transactions per second)
- Serial verify throughput
- Parallel verify throughput using Rayon across all physical cores
- Parallel speedup factor vs serial
- Transaction wire size in bytes (bincode serialization)

Tested at batch sizes: 50, 100, 500, 1000, 2000 transactions.

### Section 3 — Mempool Stress Test

- Insert throughput (transactions per second)
- Duplicate rejection latency (O(1) via hash map)
- Fee-ordered selection time (top N transactions by fee)
- Eviction throughput when pool is at capacity
- Memory footprint estimate at full capacity

### Section 4 — Block Construction and Mining

- Block hash computation time (double SHA3-256)
- Merkle tree construction time at 1, 10, 100, 500, 1200 transactions
- Block serialization speed (bincode)
- zstd Level 3 compression ratio and throughput
- Block decompression throughput
- PoW hashrate: 10-second timed run with extrapolation to current difficulty
- PoW hashrate: full difficulty solve (when `--full-pow` is specified)

### Section 5 — Chain Validation and State

- State root computation time at 1,000 / 10,000 / 50,000 accounts
- Coinbase maturity unlock throughput (10,000 locked entries)
- Block validation pipeline timing (hash + PoW + Merkle + linkage)
- Parallel vs serial signature verification on a realistic block
- LRU signature cache hit-rate simulation
- Transaction hash throughput (used for mempool deduplication and Merkle leaves)

### Section 6 — Live Node Network Stress Test (optional)

Requires `--live-node`.

- Sequential HTTP submission latency (p50, p95, p99)
- Concurrent flood throughput (10 parallel tasks)
- Error breakdown by category (insufficient balance, invalid nonce, rate limited, etc.)
- Live node state snapshot (chain height, difficulty, mempool size)

---

## Interpreting Results

### Cryptographic Latency

Typical values on a modern x86-64 CPU (2020 or later):

| Operation | Expected Range |
|---|---|
| Falcon-512 Key Generation | 1.5 ms to 4.0 ms |
| Falcon-512 Sign | 1.0 ms to 3.0 ms |
| Falcon-512 Verify | 0.8 ms to 2.0 ms |
| SHA3-256 Hash | 0.5 us to 2.0 us |

Values outside these ranges indicate either a very old CPU, a heavily loaded system,
or a build that was not compiled with `--release`.

### Parallel Speedup

The parallel speedup for signature verification should be approximately equal to the
number of physical CPU cores (not logical threads). A 4-core machine should show
roughly 3.5x to 4.0x speedup. Efficiency below 70% indicates memory bandwidth
saturation or OS scheduling overhead.

### Compression Ratio

zstd Level 3 on Falcon-512 blocks should achieve 1.5x to 3.5x compression depending
on transaction count. Blocks with fewer transactions compress better as a ratio but
the absolute saving is smaller.

---

## Reproducibility

To produce reproducible results for publication:

1. Close all other applications before running.
2. Disable CPU frequency scaling if possible (set the governor to "performance").
3. Run the benchmark three times and compare the JSON outputs. Variation in mean
   values of more than 5% indicates system interference.
4. Record the exact CPU model and RAM from the system info section of the report.
5. Include the Quanta version and Rust version from the report header.

On Linux:

```bash
# Set performance governor (requires root)
echo performance | sudo tee /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor

# Run three times
for i in 1 2 3; do
    ./target/release/quanta-benchmark --iterations 1000 --output-dir ./benchmark_runs/run_$i
done
```

---

## Embedding in a Paper

The generated Markdown file is structured for direct use in a technical paper or
institutional report. The PQC comparison table in Section 0 of the Markdown report
provides NIST-sourced baseline values for ECDSA-P256, RSA-2048, Ed25519, and
CRYSTALS-Dilithium3 alongside the Falcon-512 measurements.

The JSON file can be parsed to extract specific values for inclusion in LaTeX tables.

Example: extract mean Falcon-512 verify latency from JSON using `jq`:

```bash
cat benchmark_results/*.json | jq '
  .sections[]
  | select(.name | contains("Cryptographic"))
  | .stats[]
  | select(.name | contains("Verify"))
  | {name, mean_ms, stddev_ms, p95}
'
```

---

## Troubleshooting

**Binary not found after build**

Ensure you ran `cargo build --release` not `cargo build` (debug). The release binary
is in `target/release/`, not `target/debug/`.

**Results seem too slow**

Confirm you are running the release binary. Debug builds are 10-50x slower and
should not be used for benchmarking.

**Live node test shows all errors**

The wallet generated by the benchmark has no balance. This is expected — the error
will be reported as `insufficient_balance` in the results. To test with real throughput,
fund the benchmark wallet address shown at the start of the live test using the faucet.

**Output directory not created**

The benchmark creates the output directory automatically. If it fails, check that the
path is writable and does not require elevated permissions.

**Compilation fails on sysinfo**

Ensure the `sysinfo` crate in `Cargo.toml` is pinned to version `"0.30"`. Later
versions have a different API.
