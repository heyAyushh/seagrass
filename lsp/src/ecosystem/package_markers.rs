use {
    serde_json::Value,
    std::{fs, path::Path},
};

use super::files::shallow_files_named;

#[derive(Debug, Default)]
pub(super) struct PackageMarkers {
    pub(super) codama: bool,
    pub(super) program_metadata: bool,
    pub(super) surfpool: bool,
}

pub(super) fn package_markers(root: &Path) -> PackageMarkers {
    let mut markers = PackageMarkers::default();
    for path in shallow_files_named(root, "package.json", 4, 64) {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if package_json_contains(&value, "codama") {
            markers.codama = true;
        }
        if package_json_contains(&value, "@solana-program/program-metadata")
            || package_json_contains(&value, "program-metadata")
        {
            markers.program_metadata = true;
        }
        if package_json_contains(&value, "surfpool") {
            markers.surfpool = true;
        }
    }
    markers
}

fn package_json_contains(value: &Value, needle: &str) -> bool {
    for key in [
        "dependencies",
        "devDependencies",
        "peerDependencies",
        "optionalDependencies",
        "scripts",
    ] {
        let Some(object) = value.get(key).and_then(Value::as_object) else {
            continue;
        };
        if object.iter().any(|(name, value)| {
            name.contains(needle) || value.as_str().is_some_and(|value| value.contains(needle))
        }) {
            return true;
        }
    }
    false
}
