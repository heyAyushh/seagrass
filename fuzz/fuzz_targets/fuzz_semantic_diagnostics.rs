#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|source: String| {
    seagrass::fuzz_harness::semantic_diagnostics(&source);
});
