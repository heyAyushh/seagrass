use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn anchor_rust_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_anchor_rust_files(root, root, &mut files);
    files
}

fn collect_anchor_rust_files(root: &Path, path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if is_ignored_path(root, &path) {
            continue;
        }
        if path.is_dir() {
            collect_anchor_rust_files(root, &path, files);
        } else if is_anchor_source_file(root, &path) {
            files.push(path);
        }
    }
}

fn is_ignored_path(root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(name.as_ref(), "target" | ".anchor" | "node_modules")
    })
}

pub(super) fn is_anchor_source_file(root: &Path, path: &Path) -> bool {
    if path.extension().is_none_or(|extension| extension != "rs") {
        return false;
    }

    let relative = path.strip_prefix(root).unwrap_or(path);
    let parts = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();

    root.file_name().is_some_and(|name| name == "src")
        || parts.first().is_some_and(|part| part == "src")
        || parts
            .windows(3)
            .any(|window| window[0] == "programs" && window[2] == "src")
}
