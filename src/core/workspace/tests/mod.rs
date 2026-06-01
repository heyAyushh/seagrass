use super::*;
use std::path::{Path, PathBuf};

mod references;
mod workspace_indexing;
mod workspace_lookup;
mod workspace_reachability;
mod workspace_resilience;

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{nonce}"))
}
