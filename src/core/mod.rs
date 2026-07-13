pub mod block;
pub mod contracts;
pub mod merkle;
pub mod transaction;

#[allow(unused_imports)]
pub use block::Block;
#[allow(unused_imports)]
pub use merkle::MerkleTree;
#[allow(unused_imports)]
pub use transaction::{
    AccountBalance, AccountState, SignatureScheme, Transaction, TransactionType,
};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Chain network identity
// ---------------------------------------------------------------------------

/// Integer network identifiers committed into every transaction's signing
/// payload. This prevents cross-chain replay attacks: a signature made on
/// Testnet (network_id = 0) is mathematically invalid on Mainnet (1) and
/// vice-versa.
///
/// FROZEN VALUES — never renumber or reuse:
///   0 = Testnet  (QUA7 and future testnets)
///   1 = Mainnet
pub const TESTNET_NETWORK_ID: u32 = 0;
pub const MAINNET_NETWORK_ID: u32 = 1;
pub const DEVNET_NETWORK_ID: u32 = 99;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ChainNetwork {
    Mainnet,
    Testnet,
    Devnet(u32),
}

impl ChainNetwork {
    /// Returns the canonical `network_id` for this chain.
    /// This value is included in every transaction's signing payload.
    pub fn network_id(self) -> u32 {
        match self {
            ChainNetwork::Testnet => TESTNET_NETWORK_ID,
            ChainNetwork::Mainnet => MAINNET_NETWORK_ID,
            ChainNetwork::Devnet(_) => DEVNET_NETWORK_ID,
        }
    }
}
