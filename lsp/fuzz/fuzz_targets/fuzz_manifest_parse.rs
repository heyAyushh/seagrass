#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let first_split = data.len() / 3;
    let second_split = first_split.saturating_mul(2);
    let cargo_toml = String::from_utf8_lossy(&data[..first_split]);
    let anchor_toml = String::from_utf8_lossy(&data[first_split..second_split]);
    let source = String::from_utf8_lossy(&data[second_split..]);

    seagrass::fuzz_harness::manifest_parse(&cargo_toml, &anchor_toml, &source);
});
