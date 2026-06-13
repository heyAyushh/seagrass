/// Golden-corpus regression tests: committed and external fixtures must produce
/// zero unexcluded ERROR-severity diagnostics from seagrass.
use {
    crate::{
        diagnostics::collect_with_workspace, document::ParsedDocument, file_text,
        workspace::WorkspaceIndex,
    },
    std::{fs, path::Path, path::PathBuf},
    tower_lsp::lsp_types::{DiagnosticSeverity, Url},
};

const CORPUS_ENABLED_ENV: &str = "CORPUS_ENABLED";
const CORPUS_ENABLED_VALUE: &str = "1";

/// Fetched trees that are not the compiling artifact (sparse checkout,
/// cfg-gated, or generated code). Every entry needs a reason; an empty reason
/// is a test failure. Exclusions are path prefixes relative to
/// `corpus/programs/`.
const EXTERNAL_CORPUS_EXCLUSIONS: &[(&str, &str)] = &[];

/// Locate `fixtures/corpus/` relative to the seagrass library crate manifest.
fn committed_corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/corpus")
}

/// Locate `corpus/programs/` — the gitignored external fetch directory — at the
/// workspace root (one level above the crate manifest directory).
fn external_programs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/programs")
}

/// Return all immediate sub-directories of `dir`, sorted for determinism.
fn immediate_subdirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return dirs,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    dirs
}

/// Find the source roots (directories containing `lib.rs`) inside a corpus
/// program directory.
///
/// The canonical Anchor layout puts source under
/// `<corpus-entry>/programs/<crate-name>/src/` — we walk the tree looking for
/// every `src/lib.rs` and return the parent `src` directories.
fn find_program_src_dirs(corpus_entry: &Path) -> Vec<PathBuf> {
    let mut src_dirs = Vec::new();
    find_lib_rs_dirs(corpus_entry, &mut src_dirs);
    src_dirs.sort();
    src_dirs
}

fn find_lib_rs_dirs(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_lib_rs_dirs(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()) == Some("lib.rs") {
            if let Some(parent) = path.parent() {
                out.push(parent.to_path_buf());
            }
        }
    }
}

/// Recursively collect every `.rs` file under `dir`, sorted for determinism.
fn rust_files_in(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rs_files(dir, &mut files);
    files.sort();
    files
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(it) => it,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[derive(Debug, Default)]
struct CorpusScanResult {
    errors: Vec<(PathBuf, String)>,
    skipped_oversized_files: Vec<PathBuf>,
}

impl CorpusScanResult {
    fn error_count(&self) -> usize {
        self.errors.len()
    }

    fn skipped_oversized_count(&self) -> usize {
        self.skipped_oversized_files.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFailurePolicy {
    Panic,
    Skip,
}

/// Run the diagnostic engine over every `.rs` file in `program_src_dir` and
/// hard-assert that none produce an ERROR-severity diagnostic.
///
/// `program_src_dir` is passed to `WorkspaceIndex::build` — exactly what the
/// CLI does when the user runs `seagrass diagnostics <dir>`.
fn assert_no_errors_in_program(program_src_dir: &Path) {
    let source_files = rust_files_in(program_src_dir);
    assert!(
        !source_files.is_empty(),
        "corpus program has no .rs files under {}",
        program_src_dir.display()
    );

    let scan = scan_program(program_src_dir, ReadFailurePolicy::Panic);

    assert!(
        scan.skipped_oversized_files.is_empty(),
        "committed corpus program '{}' skipped {} oversized source file(s); hard \
         corpus fixtures must fit the same {} byte source limit as the CLI:\n{}",
        program_src_dir.display(),
        scan.skipped_oversized_count(),
        file_text::MAX_PROJECT_FILE_BYTES,
        format_skipped_files(&scan.skipped_oversized_files)
    );

    assert!(
        scan.errors.is_empty(),
        "corpus program '{}' produced {} unexpected ERROR-severity diagnostic(s) \
         (programs that compile cleanly must produce zero errors):\n{}",
        program_src_dir.display(),
        scan.error_count(),
        scan.errors
            .iter()
            .map(|(_, msg)| format!("  - {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn scan_program(
    program_src_dir: &Path,
    read_failure_policy: ReadFailurePolicy,
) -> CorpusScanResult {
    let root_url = Url::from_directory_path(program_src_dir)
        .expect("program src dir must be an absolute path");
    let workspace_index = WorkspaceIndex::build(&[root_url], std::iter::empty::<(Url, String)>());

    let mut result = CorpusScanResult::default();

    for path in rust_files_in(program_src_dir) {
        let source = match file_text::read_limited_text(&path) {
            Ok(Some(source)) => source,
            Ok(None) => {
                result.skipped_oversized_files.push(path);
                continue;
            }
            Err(error) => match read_failure_policy {
                ReadFailurePolicy::Panic => {
                    panic!("failed to read {}: {error}", path.display());
                }
                ReadFailurePolicy::Skip => continue,
            },
        };
        let Ok(document) = ParsedDocument::parse(source) else {
            continue;
        };

        let diagnostics = collect_with_workspace(&document, Some(&workspace_index));

        for diagnostic in diagnostics {
            if diagnostic.severity == Some(DiagnosticSeverity::ERROR) {
                let location = format!(
                    "{}:{}: {}",
                    path.display(),
                    diagnostic.range.start.line + 1,
                    diagnostic.message
                );
                result.errors.push((path.clone(), location));
            }
        }
    }

    result
}

fn format_skipped_files(files: &[PathBuf]) -> String {
    files
        .iter()
        .map(|path| format!("  - {}", path.display()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_error_messages(messages: &[String]) -> String {
    messages
        .iter()
        .map(|message| format!("  - {message}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn external_corpus_error_is_excluded(error_path: &Path, programs_dir: &Path) -> bool {
    let relative_path = error_path.strip_prefix(programs_dir).unwrap_or(error_path);
    EXTERNAL_CORPUS_EXCLUSIONS
        .iter()
        .any(|(prefix, _)| relative_path.starts_with(prefix))
}

fn corpus_enabled() -> bool {
    std::env::var(CORPUS_ENABLED_ENV).as_deref() == Ok(CORPUS_ENABLED_VALUE)
}

// ---------------------------------------------------------------------------
// Committed-fixture tests — hard zero-ERROR gate
// ---------------------------------------------------------------------------

/// Hard zero-ERROR guard for the committed corpus programs. The sweep iterates
/// every sub-directory of
/// `fixtures/corpus/` automatically, so dropping a new program there is
/// sufficient to include it — no test change is needed.
#[test]
fn all_corpus_programs_have_no_error_diagnostics() {
    let root = committed_corpus_root();
    let entries = immediate_subdirs(&root);
    assert!(
        !entries.is_empty(),
        "committed corpus directory is empty — expected at least blueshift_anchor_escrow under {}",
        root.display()
    );

    for entry in &entries {
        let src_dirs = find_program_src_dirs(entry);
        for src_dir in src_dirs {
            assert_no_errors_in_program(&src_dir);
        }
    }
}

/// Named regression guard for the blueshift escrow program.
///
/// This program previously triggered false-positive ERROR diagnostics for
/// raw-account and signer checks on accounts correctly constrained via
/// `has_one`, `seeds`, and `bump`.  Keeping a dedicated test makes bisecting
/// regressions faster even though the sweep above also covers it.
#[test]
fn blueshift_anchor_escrow_has_no_error_diagnostics() {
    let src_dir = committed_corpus_root()
        .join("blueshift_anchor_escrow/programs/blueshift_anchor_escrow/src");
    assert_no_errors_in_program(&src_dir);
}

// ---------------------------------------------------------------------------
// External corpus test — hard zero-ERROR gate, graceful skip when not fetched
// ---------------------------------------------------------------------------

/// Exercise the programs pinned in `corpus/manifest.toml` against the full
/// diagnostic engine.
///
/// This test is skipped silently unless **both** conditions are met:
///
/// 1. The environment variable `CORPUS_ENABLED=1` is set.
/// 2. `corpus/programs/` exists and contains at least one entry (i.e.
///    `scripts/fetch-corpus.sh` has been run).
///
/// In CI, the corpus workflow (`corpus.yml`) sets the variable and runs the
/// fetch step before invoking this test with `--nocapture`.
#[test]
fn external_corpus() {
    let programs_dir = external_programs_dir();
    let programs_present = programs_dir.is_dir()
        && programs_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

    if !corpus_enabled() || !programs_present {
        println!(
            "external_corpus: skipped (CORPUS_ENABLED={:?}, corpus/programs/ exists={}). \
             Set CORPUS_ENABLED=1 and run scripts/fetch-corpus.sh to enable.",
            std::env::var(CORPUS_ENABLED_ENV).ok(),
            programs_present
        );
        return;
    }

    println!(
        "external_corpus: ENABLED — scanning programs under {}",
        programs_dir.display()
    );

    let mut total_programs = 0usize;
    let mut total_unexpected_errors = 0usize;
    let mut total_excluded_errors = 0usize;
    let mut total_skipped_oversized_files = 0usize;
    let mut unexpected_errors = Vec::new();

    for entry in immediate_subdirs(&programs_dir) {
        let program_name = entry
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<unknown>")
            .to_owned();

        let src_dirs = find_program_src_dirs(&entry);
        if src_dirs.is_empty() {
            println!("  [{}] no src dirs found — skipping", program_name);
            continue;
        }

        for src_dir in &src_dirs {
            total_programs += 1;
            let scan = scan_program(src_dir, ReadFailurePolicy::Skip);
            let skipped_oversized_count = scan.skipped_oversized_count();
            total_skipped_oversized_files += skipped_oversized_count;

            let mut source_unexpected_errors = Vec::new();
            let mut source_excluded_error_count = 0usize;
            for (path, message) in &scan.errors {
                if external_corpus_error_is_excluded(path, &programs_dir) {
                    source_excluded_error_count += 1;
                } else {
                    source_unexpected_errors.push(message.clone());
                }
            }

            total_unexpected_errors += source_unexpected_errors.len();
            total_excluded_errors += source_excluded_error_count;
            unexpected_errors.extend(source_unexpected_errors.iter().cloned());

            if source_unexpected_errors.is_empty() && skipped_oversized_count == 0 {
                println!("  [{program_name}] clean corpus source root");
            } else {
                if source_unexpected_errors.is_empty() {
                    println!(
                        "  [{}] {} — clean scanned files; skipped {} oversized source file(s):",
                        program_name,
                        src_dir.display(),
                        skipped_oversized_count
                    );
                } else {
                    println!(
                        "  [{}] {} — {} ERROR(s):",
                        program_name,
                        src_dir.display(),
                        source_unexpected_errors.len()
                    );
                    for message in &source_unexpected_errors {
                        println!("    {message}");
                    }
                    if skipped_oversized_count > 0 {
                        println!(
                            "    skipped {} oversized source file(s):",
                            skipped_oversized_count
                        );
                    }
                }
                for path in &scan.skipped_oversized_files {
                    println!("    - {}", path.display());
                }
                if source_excluded_error_count > 0 {
                    println!("    {source_excluded_error_count} excluded ERROR(s)");
                }
            }
        }
    }

    println!(
        "external_corpus: scanned {} program(s), {} unexpected ERROR-severity diagnostic(s), \
         {} excluded ERROR-severity diagnostic(s), {} oversized source file(s) skipped \
         (hard gate)",
        total_programs,
        total_unexpected_errors,
        total_excluded_errors,
        total_skipped_oversized_files
    );

    assert!(
        unexpected_errors.is_empty(),
        "external corpus produced {} unexpected ERROR-severity diagnostic(s):\n{}",
        unexpected_errors.len(),
        format_error_messages(&unexpected_errors)
    );
}

#[test]
fn external_corpus_exclusions_have_reasons() {
    for (prefix, reason) in EXTERNAL_CORPUS_EXCLUSIONS {
        assert!(
            !prefix.trim().is_empty(),
            "external corpus exclusion prefix must not be empty"
        );
        assert!(
            !reason.trim().is_empty(),
            "external corpus exclusion '{prefix}' must include a reason"
        );
    }

    let programs_dir = external_programs_dir();
    if !corpus_enabled() || !programs_dir.is_dir() {
        return;
    }

    for (prefix, _) in EXTERNAL_CORPUS_EXCLUSIONS {
        let excluded_path = programs_dir.join(prefix);
        assert!(
            excluded_path.exists(),
            "external corpus exclusion '{}' is stale; {} does not exist",
            prefix,
            excluded_path.display()
        );
    }
}

#[test]
fn external_corpus_scan_reports_oversized_source_files() {
    let src_dir = unique_temp_dir("seagrass-corpus-oversized").join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let oversized_file = src_dir.join("codegen.rs");
    fs::write(
        &oversized_file,
        vec![b'a'; file_text::MAX_PROJECT_FILE_BYTES as usize + 1],
    )
    .unwrap();

    let scan = scan_program(&src_dir, ReadFailurePolicy::Skip);

    assert!(scan.errors.is_empty());
    assert_eq!(scan.skipped_oversized_files, vec![oversized_file]);
    fs::remove_dir_all(src_dir.parent().unwrap()).unwrap();
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{}-{}",
        prefix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
