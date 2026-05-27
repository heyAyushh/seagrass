use {
    std::{
        fs,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Position, Range},
};

pub(super) fn anchor_toml_paths(root: &Path) -> Vec<PathBuf> {
    let direct = root.join("Anchor.toml");
    if direct.is_file() {
        return vec![direct];
    }
    shallow_files_named(root, "Anchor.toml", 3, 64)
}

pub(super) fn cargo_manifest_paths(root: &Path) -> Vec<PathBuf> {
    let direct = root.join("Cargo.toml");
    if direct.is_file() {
        return vec![direct];
    }
    shallow_files_named(root, "Cargo.toml", 4, 128)
}

pub(super) fn idl_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for relative in ["target/idl", "idls", "idl", ".anchor/idl"] {
        let dir = root.join(relative);
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths
}

pub(super) fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_source_files(root, root, &mut files);
    files.sort_by_key(|path| {
        (
            path.file_name().is_some_and(|name| name == "lib.rs"),
            path.components().count(),
        )
    });
    files.reverse();
    files
}

pub(super) fn range_for_json_value(text: &str, name: &str) -> Option<Range> {
    let quoted = format!("\"{name}\"");
    let offset = text.find(&quoted)? + 1;
    range_for_offset(text, offset, name.chars().count())
}

fn collect_rust_source_files(root: &Path, path: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if should_skip_path(&path) {
            continue;
        }
        if path.is_dir() {
            if path
                .strip_prefix(root)
                .map_or(0, |relative| relative.components().count())
                <= 4
            {
                collect_rust_source_files(root, &path, files);
            }
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn shallow_files_named(
    root: &Path,
    name: &str,
    max_depth: usize,
    max_files: usize,
) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_shallow_files_named(root, root, name, max_depth, max_files, &mut files);
    files
}

fn collect_shallow_files_named(
    root: &Path,
    path: &Path,
    name: &str,
    max_depth: usize,
    max_files: usize,
    files: &mut Vec<PathBuf>,
) {
    if files.len() >= max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= max_files {
            return;
        }
        let path = entry.path();
        if should_skip_path(&path) {
            continue;
        }
        if path.is_dir() {
            if path
                .strip_prefix(root)
                .map_or(0, |relative| relative.components().count())
                < max_depth
            {
                collect_shallow_files_named(root, &path, name, max_depth, max_files, files);
            }
        } else if path.file_name().is_some_and(|file_name| file_name == name) {
            files.push(path);
        }
    }
}

fn should_skip_path(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        matches!(name.as_ref(), "target" | ".git" | "node_modules")
    })
}

fn range_for_offset(text: &str, offset: usize, len: usize) -> Option<Range> {
    let mut line = 0usize;
    let mut character = 0usize;
    for (idx, ch) in text.char_indices() {
        if idx == offset {
            return Some(Range {
                start: Position {
                    line: u32::try_from(line).ok()?,
                    character: u32::try_from(character).ok()?,
                },
                end: Position {
                    line: u32::try_from(line).ok()?,
                    character: u32::try_from(character + len).ok()?,
                },
            });
        }
        if ch == '\n' {
            line += 1;
            character = 0;
        } else {
            character += 1;
        }
    }
    None
}
