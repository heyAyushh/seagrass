#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    seagrass::fuzz_harness::anchor_preflight(data);
});
