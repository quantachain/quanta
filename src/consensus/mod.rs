pub mod blockchain;
pub mod authorities;
pub mod bft;
pub mod bft_proposer;
pub mod mempool;
pub mod performance; // PERFORMANCE OPTIMIZATIONS


pub use blockchain::Blockchain;
#[allow(unused_imports)]
pub use mempool::{Mempool, MetricsCollector};
#[allow(unused_imports)]
pub use performance::*;
