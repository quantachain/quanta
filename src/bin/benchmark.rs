// Silence warnings in binary-specific code
#![allow(unused_imports)]

/// Quanta PQC Benchmark Binary
///
/// Standalone binary — does NOT require a running node for offline tests.
/// Run with: cargo run --release --bin quanta-benchmark [OPTIONS]
///
/// Options:
///   --iterations N        Number of iterations per micro-benchmark (default: 500)
///   --output-dir PATH     Directory for JSON/Markdown output (default: ./benchmark_results)
///   --full-pow            Include full PoW difficulty solve (may take minutes)
///   --live-node URL       Enable live HTTP stress test against a running node
///   --wallet-file PATH    Path to a faucet wallet JSON file (faucet_accounts_export.json)
///                         Keeps private keys out of git — file never committed
///   --wallet-index N      Which wallet to use from the file (default: 1)
///   --json-only           Only write JSON, skip Markdown
///   --quick               Quick mode: 100 iterations, no full-PoW, no live node
///   --help                Print this help

mod core {
    pub use quanta::core::*;
}
mod consensus {
    pub use quanta::consensus::*;
}
mod crypto {
    pub use quanta::crypto::*;
}
mod storage {
    pub use quanta::storage::*;
}
mod network {
    pub use quanta::network::*;
}
mod api {
    pub use quanta::api::*;
}
mod config {
    pub use quanta::config::*;
}
mod rpc {
    pub use quanta::rpc::*;
}
mod benchmark {
    pub use quanta::benchmark::*;
}

use quanta::benchmark::report::{run_all_benchmarks, write_json, write_markdown, BenchmarkSection};
use std::env;

#[tokio::main]
async fn main() {
    // ── Parse CLI args ────────────────────────────────────────────────────────
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let quick = args.iter().any(|a| a == "--quick");
    let full_pow = args.iter().any(|a| a == "--full-pow") && !quick;
    let json_only = args.iter().any(|a| a == "--json-only");

    let iterations = if quick {
        100
    } else {
        parse_arg(&args, "--iterations").unwrap_or(500)
    };

    let output_dir = args
        .windows(2)
        .find(|w| w[0] == "--output-dir")
        .map(|w| w[1].clone())
        .unwrap_or_else(|| "./benchmark_results".to_string());

    let live_node_url = if quick {
        None
    } else {
        args.windows(2)
            .find(|w| w[0] == "--live-node")
            .map(|w| w[1].clone())
    };

    // Optional: path to faucet_accounts_export.json (keeps private keys OUT of git)
    let wallet_file = args
        .windows(2)
        .find(|w| w[0] == "--wallet-file")
        .map(|w| w[1].clone());
    let wallet_index: usize = args
        .windows(2)
        .find(|w| w[0] == "--wallet-index")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(1); // default = Faucet 1 (Faucet 0 is the live faucet sender)

    // ── Run offline benchmarks ────────────────────────────────────────────────
    let (mut report, mut sections) = run_all_benchmarks(iterations, full_pow);

    // ── Live node section (async) ─────────────────────────────────────────────
    let network_section = if let Some(ref url) = live_node_url {
        let tx_count = if quick {
            20
        } else {
            parse_arg(&args, "--live-txs").unwrap_or(100)
        };
        quanta::benchmark::network_bench::run(url, tx_count, wallet_file.as_deref(), wallet_index)
            .await
    } else {
        quanta::benchmark::network_bench::run_skipped()
    };
    sections.push(network_section.clone());
    report.sections.push(network_section);

    // ── Print summary to stdout ───────────────────────────────────────────────
    println!("\n{}", "═".repeat(70));
    println!(" BENCHMARK SUMMARY");
    println!("{}", "═".repeat(70));

    for section in &report.sections {
        println!("\n▸ {}", section.name);
        for stat in &section.stats {
            if stat.iterations == 0 {
                continue;
            }
            let tp_str = stat
                .throughput
                .map(|t| format!("  [{:.0} ops/s]", t))
                .unwrap_or_default();
            println!(
                "    {:<55} mean={:>10.3} {}{}",
                truncate(&stat.name, 55),
                stat.mean_ms,
                stat.unit,
                tp_str,
            );
        }
    }

    // ── Write output files ────────────────────────────────────────────────────
    println!("\n{}", "═".repeat(70));
    match write_json(&report, &output_dir) {
        Ok(path) => println!(" ✅ JSON report: {}", path),
        Err(e) => eprintln!(" ❌ Failed to write JSON: {}", e),
    }
    if !json_only {
        match write_markdown(&report, &output_dir) {
            Ok(path) => println!(" ✅ Markdown report: {}", path),
            Err(e) => eprintln!(" ❌ Failed to write Markdown: {}", e),
        }
    }

    println!("\n Quanta PQC Benchmark Complete.");
    println!(" Share the Markdown report with your government / defense / banking contacts.");
    println!(" The JSON file is machine-readable for CI pipelines and regression tracking.\n");
}

fn parse_arg(args: &[String], flag: &str) -> Option<usize> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .and_then(|w| w[1].parse().ok())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max - 1])
    }
}

fn print_help() {
    println!(
        r#"
Quanta PQC Benchmark Suite — {version}

USAGE:
    cargo run --release --bin quanta-benchmark [OPTIONS]

OPTIONS:
    --iterations N      Iterations per micro-benchmark (default: 500)
    --output-dir PATH   Output directory for reports (default: ./benchmark_results)
    --full-pow          Run full PoW solve at current difficulty (may take minutes)
    --live-node URL     Live HTTP stress test against a running node
                        Example: --live-node http://localhost:3000
    --wallet-file PATH  Path to faucet_accounts_export.json (optional)
                        Loads a pre-funded wallet so txs are accepted by the node.
                        This file is NEVER committed to git — keep it local.
    --wallet-index N    Which wallet index to use from the file (default: 1)
    --live-txs N        Number of txs for live test (default: 100)
    --json-only         Write JSON only (skip Markdown)
    --quick             100 iterations, no full-PoW, no live node
    -h, --help          Show this help

EXAMPLES:
    # Standard run (500 iterations, no PoW solve, offline only)
    cargo run --release --bin quanta-benchmark

    # Full suite with live node and PoW solve
    cargo run --release --bin quanta-benchmark -- --full-pow --live-node http://localhost:3000

    # Quick sanity check
    cargo run --release --bin quanta-benchmark -- --quick

OUTPUT:
    benchmark_results/quanta_benchmark_<timestamp>.json
    benchmark_results/quanta_benchmark_<timestamp>.md
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
