/// Quanta PQC Benchmark Suite — Module Root
///
/// Re-exports all sub-benchmarks and the shared result types.
/// Run via: `cargo run --release --bin quanta-benchmark`

pub mod crypto_bench;
pub mod tx_bench;
pub mod block_bench;
pub mod chain_bench;
pub mod network_bench;
pub mod report;

pub use report::{BenchmarkReport, BenchmarkSection, BenchmarkStat, run_all_benchmarks};
