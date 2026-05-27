#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let tokens = String::from_utf8_lossy(data);
    seagrass::fuzz_harness::anchor_attr(&tokens);
});
