use quanta::core::block::Block;
use quanta::core::ChainNetwork;

fn main() {
    println!("Mining Testnet Genesis...");
    let mut testnet_genesis = Block::genesis(ChainNetwork::Testnet);
    testnet_genesis.mine();
    println!("TESTNET NONCE: {}", testnet_genesis.nonce);
    println!("TESTNET HASH: {}", testnet_genesis.hash);

    println!("Mining Mainnet Genesis...");
    let mut mainnet_genesis = Block::genesis(ChainNetwork::Mainnet);
    mainnet_genesis.mine();
    println!("MAINNET NONCE: {}", mainnet_genesis.nonce);
    println!("MAINNET HASH: {}", mainnet_genesis.hash);
}
