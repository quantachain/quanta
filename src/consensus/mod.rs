pub mod blockchain;
pub mod mempool;
pub mod performance; // PERFORMANCE OPTIMIZATIONS

pub use blockchain::Blockchain;
pub use mempool::{Mempool, MetricsCollector};
pub use performance::*;
