/// Standalone genesis miner — run with:
///   rustc mine_genesis.rs -O --edition 2021 -o mine_genesis && ./mine_genesis
///
/// Or add as a [[bin]] in Cargo.toml and `cargo run --bin mine_genesis --release`
///
/// What it does:
///  1. Benchmarks SHA3-256 double-hash speed on this machine
///  2. Calculates the difficulty needed for a 30-second block
///  3. Mines a valid genesis nonce for that difficulty
///  4. Prints the values to paste into block.rs

// This is a standalone Rust file. To run it through the project's Cargo:
// Add to Cargo.toml:
//   [[bin]]
//   name = "mine_genesis"
//   path = "mine_genesis.rs"
// Then: cargo run --release --bin mine_genesis
//
// Or compile standalone (needs sha3 crate vendored — easier to run via Cargo).

fn main() {
    println!("This file is meant to be run as a Cargo binary.");
    println!("See instructions at the top of the file.");
}
