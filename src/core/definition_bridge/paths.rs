use {
    crate::solana::artifact_paths,
    std::{
        fs,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Position, Range},
};

const RUST_SOURCE_TRAVERSAL_BUDGET: TraversalBudget = TraversalBudget {
    max_depth: 4,
    max_files: 512,
};

#[derive(Clone, Copy)]
struct TraversalBudget {
    max_depth: usize,
    max_files: usize,
}

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
    for relative in [artifact_paths::TARGET_IDL_DIR, "idls", "idl", ".anchor/idl"] {
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
    rust_source_files_with_budget(root, RUST_SOURCE_TRAVERSAL_BUDGET)
}

fn rust_source_files_with_budget(root: &Path, budget: TraversalBudget) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_source_files(root, root, budget, &mut files);
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

fn collect_rust_source_files(
    root: &Path,
    path: &Path,
    budget: TraversalBudget,
    files: &mut Vec<PathBuf>,
) {
    if files.len() >= budget.max_files {
        return;
    }
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    for entry in entries.flatten() {
        if files.len() >= budget.max_files {
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
                <= budget.max_depth
            {
                collect_rust_source_files(root, &path, budget, files);
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{
            fs::{create_dir_all, remove_dir_all, write},
            time::{SystemTime, UNIX_EPOCH},
        },
    };

    const TEST_TRAVERSAL_BUDGET: TraversalBudget = TraversalBudget {
        max_depth: 2,
        max_files: 2,
    };

    #[test]
    fn rust_source_collection_respects_depth_budget() {
        let root = temp_root("rust-source-depth");
        let shallow = root.join("src").join("lib.rs");
        let deep = root
            .join("src")
            .join("generated")
            .join("nested")
            .join("ignored.rs");
        create_file(&shallow);
        create_file(&deep);

        let files = rust_source_files_with_budget(&root, TEST_TRAVERSAL_BUDGET);

        assert!(
            files.contains(&shallow),
            "missing shallow Rust file: {files:?}"
        );
        assert!(
            !files.contains(&deep),
            "collected Rust file beyond depth budget: {files:?}"
        );
        cleanup_temp_root(&root);
    }

    #[test]
    fn rust_source_collection_respects_file_budget() {
        let root = temp_root("rust-source-file-count");
        for name in ["one.rs", "two.rs", "three.rs"] {
            create_file(&root.join("src").join(name));
        }

        let files = rust_source_files_with_budget(&root, TEST_TRAVERSAL_BUDGET);

        assert_eq!(
            files.len(),
            TEST_TRAVERSAL_BUDGET.max_files,
            "Rust source collection should stop at the file budget: {files:?}"
        );
        cleanup_temp_root(&root);
    }

    fn create_file(path: &Path) {
        create_dir_all(path.parent().expect("test file should have a parent"))
            .expect("test directory should be created");
        write(path, "pub fn smoke() {}\n").expect("test file should be written");
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("seagrass-{label}-{nonce}"))
    }

    fn cleanup_temp_root(root: &Path) {
        if root.exists() {
            remove_dir_all(root).expect("test directory should be removed");
        }
    }
}
