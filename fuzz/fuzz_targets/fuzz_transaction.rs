#![no_main]

use libfuzzer_sys::fuzz_target;
use quanta::core::transaction::Transaction;

fuzz_target!(|data: &[u8]| {
    // Fuzz the Transaction deserializer to ensure malicious bytes
    // don't cause panics or out of memory issues.
    if let Ok(tx) = Transaction::from_payload(data) {
        // Also ensure verification doesn't panic on malformed internal state
        let _ = tx.verify();
    }
});
