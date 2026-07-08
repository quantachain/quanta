#![no_main]

use libfuzzer_sys::fuzz_target;
use quanta::network::protocol::deserialize_message;

fuzz_target!(|data: &[u8]| {
    // We just want to ensure that throwing completely random garbage
    // at the deserializer never causes a panic, out-of-memory, or crash.
    let _ = deserialize_message(data);
});
