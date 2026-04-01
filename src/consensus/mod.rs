pub mod blockchain;
pub mod mempool;
pub mod performance; // PERFORMANCE OPTIMIZATIONS

pub use blockchain::Blockchain;
#[allow(unused_imports)]
pub use mempool::{Mempool, MetricsCollector};
#[allow(unused_imports)]
pub use performance::*;
