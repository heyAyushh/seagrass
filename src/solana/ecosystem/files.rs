use {
    crate::file_text,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

use super::MAX_SCAN_DEPTH;

pub(super) fn shallow_files_named(
    root: &Path,
    name: &str,
    max_depth: usize,
    max_files: usize,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_matching_files(root, 0, max_depth, max_files, &mut paths, &|path| {
        path.file_name().and_then(|file_name| file_name.to_str()) == Some(name)
    });
    paths.sort();
    paths
}

pub(super) fn shallow_files_with_extensions(
    root: &Path,
    extensions: &[&str],
    max_depth: usize,
    max_files: usize,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    collect_matching_files(root, 0, max_depth, max_files, &mut paths, &|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extensions.contains(&extension))
    });
    paths.sort();
    paths
}

pub(super) fn rust_files(root: &Path, max_files: usize) -> Vec<PathBuf> {
    shallow_files_with_extensions(root, &["rs"], MAX_SCAN_DEPTH, max_files)
}

pub(super) fn read_limited_text(path: &Path) -> Option<String> {
    file_text::read_limited_text(path).ok().flatten()
}

pub(super) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn collect_matching_files(
    dir: &Path,
    depth: usize,
    max_depth: usize,
    max_files: usize,
    paths: &mut Vec<PathBuf>,
    matches: &dyn Fn(&Path) -> bool,
) {
    if depth > max_depth || paths.len() >= max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        if paths.len() >= max_files {
            return;
        }
        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_matching_files(&path, depth + 1, max_depth, max_files, paths, matches);
        } else if matches(&path) {
            paths.push(path);
        }
    }
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".git" | ".anchor" | "node_modules" | "target")
    )
}
