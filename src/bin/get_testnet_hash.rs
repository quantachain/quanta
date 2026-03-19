use quanta::core::{block::Block, ChainNetwork};

fn main() {
    let genesis = Block::genesis(ChainNetwork::Testnet);
    println!("REAL_TESTNET_GENESIS_HASH: {}", genesis.hash);
}
