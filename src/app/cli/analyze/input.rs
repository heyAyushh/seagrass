use {
    super::{AnalyzeUsageError, ANALYZE_DIRECTORY_EXAMPLE, ANALYZE_FILE_EXAMPLE, RUST_EXTENSION},
    crate::workspace::WorkspaceIndex,
    std::{
        error::Error,
        fs,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::Url,
};

pub(super) fn workspace_index_for_path(path: &Path) -> Option<WorkspaceIndex> {
    let roots = workspace_roots_for_path(path)
        .into_iter()
        .filter_map(|root| Url::from_directory_path(root).ok())
        .collect::<Vec<_>>();
    (!roots.is_empty()).then(|| WorkspaceIndex::build(&roots, std::iter::empty::<(Url, String)>()))
}

fn workspace_roots_for_path(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return source_root_for_file(path).into_iter().collect();
    }
    if path.is_dir() {
        return vec![path.to_path_buf()];
    }
    Vec::new()
}

fn source_root_for_file(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "src"))
        .map(Path::to_path_buf)
        .or_else(|| path.parent().map(Path::to_path_buf))
}

pub(super) fn rust_files(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let metadata = fs::metadata(path).map_err(|error| {
        AnalyzeUsageError::new(format!(
            "Error: analyze path cannot be inspected: {}.\nExample:\n  {ANALYZE_FILE_EXAMPLE}\nCause: {error}",
            path.display()
        ))
    })?;
    if metadata.is_file() {
        validate_rust_file(path)?;
        return Ok(vec![path.to_path_buf()]);
    }
    if metadata.is_dir() {
        let mut files = rust_files_in_dir(path)?;
        files.sort();
        if files.is_empty() {
            return Err(AnalyzeUsageError::new(format!(
                "Error: no Rust source files found under {}.\nExample:\n  {ANALYZE_DIRECTORY_EXAMPLE}",
                path.display()
            ))
            .into());
        }
        return Ok(files);
    }
    Err(AnalyzeUsageError::new(format!(
        "Error: analyze path must be a Rust file or directory: {}.\nExamples:\n  {ANALYZE_FILE_EXAMPLE}\n  {ANALYZE_DIRECTORY_EXAMPLE}",
        path.display()
    ))
    .into())
}

fn rust_files_in_dir(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    fs::read_dir(path)?.try_fold(Vec::new(), |mut files, entry| {
        let entry = entry?;
        let entry_path = entry.path();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            files.extend(rust_files_in_dir(&entry_path)?);
        } else if is_rust_file(&entry_path) {
            files.push(entry_path);
        }
        Ok(files)
    })
}

fn is_rust_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == RUST_EXTENSION)
}

fn validate_rust_file(path: &Path) -> Result<(), AnalyzeUsageError> {
    if is_rust_file(path) {
        return Ok(());
    }
    Err(AnalyzeUsageError::new(format!(
        "Error: analyze file must use the .rs extension: {}.\nExample:\n  {ANALYZE_FILE_EXAMPLE}",
        path.display()
    )))
}

pub(super) fn display_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}
