pub mod block_bench;
pub mod chain_bench;
/// Quanta PQC Benchmark Suite — Module Root
///
/// Re-exports all sub-benchmarks and the shared result types.
/// Run via: `cargo run --release --bin quanta-benchmark`
pub mod crypto_bench;
pub mod network_bench;
pub mod report;
pub mod tx_bench;

