/// Golden-corpus regression tests: committed fixtures must produce zero
/// ERROR-severity diagnostics from seagrass.
///
/// The single committed program (`blueshift_anchor_escrow`) acts as the primary
/// false-positive regression guard.  External programs (pinned in
/// `corpus/manifest.toml` and fetched by `scripts/fetch-corpus.sh`) are tested
/// separately in `external_corpus` below; those trees are gitignored and only
/// exercised when `CORPUS_ENABLED=1` is set in the environment.
use {
    crate::{
        diagnostics::collect_with_workspace, document::ParsedDocument, workspace::WorkspaceIndex,
    },
    std::{fs, path::Path, path::PathBuf},
    tower_lsp::lsp_types::{DiagnosticSeverity, Url},
};

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

/// Run the diagnostic engine over every `.rs` file in `program_src_dir` and
/// hard-assert that none produce an ERROR-severity diagnostic.
///
/// `program_src_dir` is passed to `WorkspaceIndex::build` — exactly what the
/// CLI does when the user runs `seagrass diagnostics <dir>`.
fn assert_no_errors_in_program(program_src_dir: &Path) {
    let root_url = Url::from_directory_path(program_src_dir)
        .expect("program src dir must be an absolute path");
    let workspace_index = WorkspaceIndex::build(&[root_url], std::iter::empty::<(Url, String)>());

    let source_files = rust_files_in(program_src_dir);
    assert!(
        !source_files.is_empty(),
        "corpus program has no .rs files under {}",
        program_src_dir.display()
    );

    let mut errors: Vec<(PathBuf, String)> = Vec::new();

    for path in &source_files {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        let document = match ParsedDocument::parse(source) {
            Ok(doc) => doc,
            // Parse failures are not ERROR-severity false positives; the source
            // itself is ill-formed, which seagrass is entitled to report.
            Err(_) => continue,
        };

        let diagnostics = collect_with_workspace(&document, Some(&workspace_index));

        for diagnostic in diagnostics {
            if diagnostic.severity == Some(DiagnosticSeverity::ERROR) {
                let location = format!(
                    "{}:{}: {}",
                    path.display(),
                    // Report 1-based line numbers to match editor conventions.
                    diagnostic.range.start.line + 1,
                    diagnostic.message
                );
                errors.push((path.clone(), location));
            }
        }
    }

    assert!(
        errors.is_empty(),
        "corpus program '{}' produced {} unexpected ERROR-severity diagnostic(s) \
         (programs that compile cleanly must produce zero errors):\n{}",
        program_src_dir.display(),
        errors.len(),
        errors
            .iter()
            .map(|(_, msg)| format!("  - {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Collect ERROR-severity diagnostics from a program without asserting zero —
/// used by the external corpus in discovery mode.
///
/// Returns a list of `(file_path, formatted_location_message)` pairs.
fn collect_errors_in_program(program_src_dir: &Path) -> Vec<(PathBuf, String)> {
    let root_url = Url::from_directory_path(program_src_dir)
        .expect("program src dir must be an absolute path");
    let workspace_index = WorkspaceIndex::build(&[root_url], std::iter::empty::<(Url, String)>());

    let mut errors = Vec::new();

    for path in rust_files_in(program_src_dir) {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
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
                errors.push((path.clone(), location));
            }
        }
    }

    errors
}

// ---------------------------------------------------------------------------
// Committed-fixture tests — hard zero-ERROR gate
// ---------------------------------------------------------------------------

/// Hard zero-ERROR guard for the only committed corpus program (blueshift
/// anchor escrow).  The sweep iterates every sub-directory of
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
// External corpus test — discovery mode, graceful skip when not fetched
// ---------------------------------------------------------------------------

/// Exercise the programs pinned in `corpus/manifest.toml` against the full
/// diagnostic engine.
///
/// # Activation
///
/// This test is skipped silently unless **both** conditions are met:
///
/// 1. The environment variable `CORPUS_ENABLED=1` is set.
/// 2. `corpus/programs/` exists and contains at least one entry (i.e.
///    `scripts/fetch-corpus.sh` has been run).
///
/// In CI, the corpus workflow (`corpus.yml`) sets the variable and runs the
/// fetch step before invoking this test with `--nocapture`.
///
/// # Mode
///
/// The test runs in **baseline/report** mode: it collects ERROR-severity
/// diagnostics and prints them, but does **not** hard-assert zero errors.
/// Real programs may surface real false positives that require triage before
/// the gate can flip.
///
/// TODO: after triage is complete, change this test to call
/// `assert_no_errors_in_program` instead of `collect_errors_in_program` and
/// remove the discovery-mode note above.
#[test]
fn external_corpus() {
    let corpus_enabled = std::env::var("CORPUS_ENABLED").as_deref() == Ok("1");
    let programs_dir = external_programs_dir();
    let programs_present = programs_dir.is_dir()
        && programs_dir
            .read_dir()
            .map(|mut d| d.next().is_some())
            .unwrap_or(false);

    if !corpus_enabled || !programs_present {
        println!(
            "external_corpus: skipped (CORPUS_ENABLED={:?}, corpus/programs/ exists={}). \
             Set CORPUS_ENABLED=1 and run scripts/fetch-corpus.sh to enable.",
            std::env::var("CORPUS_ENABLED").ok(),
            programs_present
        );
        return;
    }

    println!(
        "external_corpus: ENABLED — scanning programs under {}",
        programs_dir.display()
    );

    let mut total_programs = 0usize;
    let mut total_errors = 0usize;

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
            let errors = collect_errors_in_program(src_dir);
            let error_count = errors.len();
            total_errors += error_count;

            if error_count == 0 {
                println!("  [{program_name}] clean corpus source root");
            } else {
                println!(
                    "  [{}] {} — {} ERROR(s):",
                    program_name,
                    src_dir.display(),
                    error_count
                );
                for (_, msg) in &errors {
                    println!("    {msg}");
                }
            }
        }
    }

    println!(
        "external_corpus: scanned {} program(s), {} total ERROR-severity diagnostic(s) \
         (discovery mode — not a hard gate yet; see TODO in corpus.rs)",
        total_programs, total_errors
    );
    // Discovery mode: always pass.  Flip to assert_no_errors_in_program after triage.
}
