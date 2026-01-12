pub mod block;
pub mod transaction;
pub mod merkle;

pub use block::Block;
pub use transaction::{Transaction, TransactionType, AccountState, AccountBalance};
pub use merkle::MerkleTree;

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChainNetwork {
    Mainnet,
    Testnet,
}

