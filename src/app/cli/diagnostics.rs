use {
    crate::{
        diagnostics as diagnostic_engine, document::ParsedDocument, workspace::WorkspaceIndex,
    },
    clap::Args,
    serde::Serialize,
    std::{
        error::Error,
        fmt, fs,
        io::{self, Read},
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range, Url},
};

const RUST_EXTENSION: &str = "rs";
const STDIN_DISPLAY_PATH: &str = "<stdin>";
const DIAGNOSTICS_FILE_EXAMPLE: &str = "seagrass diagnostics programs/demo/src/lib.rs --json";
const DIAGNOSTICS_DIRECTORY_EXAMPLE: &str = "seagrass diagnostics programs/demo/src --json";
const DIAGNOSTICS_STDIN_EXAMPLE: &str =
    "cat programs/demo/src/lib.rs | seagrass diagnostics --stdin --stdin-path programs/demo/src/lib.rs --json";

pub(super) const DIAGNOSTICS_HELP: &str = "\
Examples:
  seagrass diagnostics programs/demo/src/lib.rs --json
  seagrass diagnostics programs/demo/src --json
  cat programs/demo/src/lib.rs | seagrass diagnostics --stdin --stdin-path programs/demo/src/lib.rs --json

Output:
  Prints a JSON array with file, range, code, severity, topic, confidence, applicability,
  docsUrl, and message. Use --sarif for GitHub code scanning compatible SARIF 2.1.0 output.
  Exits 1 when any diagnostic has ERROR severity; exits 2 for usage or input errors.";

#[derive(Debug, Args)]
pub(super) struct DiagnosticsCommand {
    /// Rust file or directory to analyze.
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Print structured JSON. This is the default output format.
    #[arg(long = "json", conflicts_with = "sarif")]
    json: bool,

    /// Print SARIF 2.1.0 for GitHub code scanning and review tools.
    #[arg(long = "sarif", conflicts_with = "json")]
    sarif: bool,

    /// Read one Rust source file from stdin instead of PATH.
    #[arg(long = "stdin")]
    stdin: bool,

    /// Source path to use for stdin diagnostics and workspace context.
    #[arg(long = "stdin-path", value_name = "PATH")]
    stdin_path: Option<PathBuf>,
}

impl DiagnosticsCommand {
    pub(super) fn run(self) -> Result<bool, Box<dyn Error>> {
        let emit_sarif = self.sarif;
        let diagnostics = match self.input()? {
            DiagnosticsInput::Path(path) => diagnostics_for_path(&path)?,
            DiagnosticsInput::Stdin { source_path } => {
                diagnostics_for_stdin(source_path.as_deref())?
            }
        };
        if emit_sarif {
            super::sarif::write_sarif(&diagnostics)?;
        } else {
            serde_json::to_writer_pretty(io::stdout(), &diagnostics)?;
            println!();
        }
        Ok(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SeverityLabel::Error))
    }

    fn input(self) -> Result<DiagnosticsInput, CliUsageError> {
        match (self.path, self.stdin, self.stdin_path) {
            (Some(_), true, _) => Err(CliUsageError::new(format!(
                "Error: use either PATH or --stdin, not both.\nExamples:\n  {DIAGNOSTICS_FILE_EXAMPLE}\n  {DIAGNOSTICS_STDIN_EXAMPLE}"
            ))),
            (Some(_), false, Some(_)) => Err(CliUsageError::new(format!(
                "Error: --stdin-path only applies with --stdin.\nExample:\n  {DIAGNOSTICS_STDIN_EXAMPLE}\nFor path diagnostics, run:\n  {DIAGNOSTICS_FILE_EXAMPLE}"
            ))),
            (Some(path), false, None) => Ok(DiagnosticsInput::Path(path)),
            (None, true, source_path) => Ok(DiagnosticsInput::Stdin { source_path }),
            (None, false, Some(_)) => Err(CliUsageError::new(format!(
                "Error: --stdin-path only applies with --stdin.\nExample:\n  {DIAGNOSTICS_STDIN_EXAMPLE}"
            ))),
            (None, false, None) => Err(CliUsageError::new(format!(
                "Error: diagnostics requires a Rust file/directory path or --stdin.\nExamples:\n  {DIAGNOSTICS_FILE_EXAMPLE}\n  {DIAGNOSTICS_DIRECTORY_EXAMPLE}\n  {DIAGNOSTICS_STDIN_EXAMPLE}"
            ))),
        }
    }
}

#[derive(Debug)]
enum DiagnosticsInput {
    Path(PathBuf),
    Stdin { source_path: Option<PathBuf> },
}

#[derive(Debug)]
struct CliUsageError {
    message: String,
}

impl CliUsageError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for CliUsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for CliUsageError {}

#[derive(Debug, Serialize)]
pub(super) struct CliDiagnostic {
    pub(super) file: String,
    pub(super) range: Range,
    pub(super) code: Option<String>,
    pub(super) severity: SeverityLabel,
    pub(super) topic: Option<String>,
    pub(super) confidence: Option<String>,
    pub(super) applicability: Option<String>,
    #[serde(rename = "docsUrl")]
    pub(super) docs_url: Option<String>,
    pub(super) message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(super) enum SeverityLabel {
    Error,
    Warning,
    Information,
    Hint,
    Unknown,
}

fn diagnostics_for_path(path: &Path) -> Result<Vec<CliDiagnostic>, Box<dyn Error>> {
    let workspace_index = workspace_index_for_path(path);
    rust_files(path)?
        .into_iter()
        .map(|file| diagnostics_for_file(&file, workspace_index.as_ref()))
        .collect::<Result<Vec<_>, _>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn diagnostics_for_file(
    path: &Path,
    workspace_index: Option<&WorkspaceIndex>,
) -> Result<Vec<CliDiagnostic>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    diagnostics_for_source(display_path(path), source, workspace_index)
}

fn diagnostics_for_stdin(source_path: Option<&Path>) -> Result<Vec<CliDiagnostic>, Box<dyn Error>> {
    let mut source = String::new();
    io::stdin().read_to_string(&mut source)?;
    diagnostics_for_stdin_source(source_path, source)
}

fn diagnostics_for_stdin_source(
    source_path: Option<&Path>,
    source: String,
) -> Result<Vec<CliDiagnostic>, Box<dyn Error>> {
    let workspace_index = workspace_index_for_stdin_path(source_path);
    let display_path = source_path
        .map(display_path)
        .unwrap_or_else(|| STDIN_DISPLAY_PATH.to_string());
    diagnostics_for_source(display_path, source, workspace_index.as_ref())
}

fn diagnostics_for_source(
    file: String,
    source: String,
    workspace_index: Option<&WorkspaceIndex>,
) -> Result<Vec<CliDiagnostic>, Box<dyn Error>> {
    let diagnostics = match ParsedDocument::parse(source.clone()) {
        Ok(document) => diagnostic_engine::collect_with_workspace(&document, workspace_index),
        Err(error) => diagnostics_for_parse_error(error, &source, workspace_index),
    };
    Ok(diagnostics
        .into_iter()
        .map(|diagnostic| CliDiagnostic::from_lsp(file.clone(), diagnostic))
        .collect())
}

fn diagnostics_for_parse_error(
    error: syn::Error,
    source: &str,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse_or_empty(source.to_string());
    let mut diagnostics = vec![diagnostic_engine::diagnostic_from_parse_error_with_source(
        error, source,
    )];
    diagnostics.extend(diagnostic_engine::collect_hot_with_input(
        diagnostic_engine::DiagnosticInput {
            document: &document,
            uri: None,
            workspace_index,
            framework: crate::solana::frameworks::FrameworkContext::from_document(&document),
            manifest: None,
            anchor_toml: None,
            seagrass_toml: None,
            solana_program: None,
            settings: diagnostic_engine::DiagnosticSettings::default(),
        },
    ));
    diagnostic_engine::dedupe(diagnostics)
}

fn workspace_index_for_path(path: &Path) -> Option<WorkspaceIndex> {
    let roots = workspace_roots_for_path(path)
        .into_iter()
        .filter_map(|root| Url::from_directory_path(root).ok())
        .collect::<Vec<_>>();
    (!roots.is_empty()).then(|| WorkspaceIndex::build(&roots, std::iter::empty::<(Url, String)>()))
}

fn workspace_index_for_stdin_path(source_path: Option<&Path>) -> Option<WorkspaceIndex> {
    let roots = source_path
        .and_then(anchor_source_root_for_file)
        .into_iter()
        .filter_map(|root| Url::from_directory_path(root).ok())
        .collect::<Vec<_>>();
    (!roots.is_empty()).then(|| WorkspaceIndex::build(&roots, std::iter::empty::<(Url, String)>()))
}

fn workspace_roots_for_path(path: &Path) -> Vec<PathBuf> {
    if path.is_file() {
        return anchor_source_root_for_file(path)
            .into_iter()
            .collect::<Vec<_>>();
    }
    if path.is_dir() {
        return vec![path.to_path_buf()];
    }
    Vec::new()
}

fn anchor_source_root_for_file(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .skip(1)
        .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "src"))
        .map(Path::to_path_buf)
        .or_else(|| path.parent().map(Path::to_path_buf))
}

fn rust_files(path: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let metadata = fs::metadata(path).map_err(|error| path_metadata_error(path, error))?;
    if metadata.is_file() {
        validate_rust_file(path)?;
        return Ok(vec![path.to_path_buf()]);
    }
    if metadata.is_dir() {
        let mut files = rust_files_in_dir(path)?;
        files.sort();
        if files.is_empty() {
            return Err(CliUsageError::new(format!(
                "Error: no Rust source files found under {}.\nExample:\n  {DIAGNOSTICS_DIRECTORY_EXAMPLE}",
                path.display()
            ))
            .into());
        }
        return Ok(files);
    }
    Err(CliUsageError::new(format!(
        "Error: diagnostics path must be a Rust file or directory: {}.\nExamples:\n  {DIAGNOSTICS_FILE_EXAMPLE}\n  {DIAGNOSTICS_DIRECTORY_EXAMPLE}",
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

fn validate_rust_file(path: &Path) -> Result<(), CliUsageError> {
    if is_rust_file(path) {
        return Ok(());
    }
    Err(CliUsageError::new(format!(
        "Error: diagnostics file must use the .rs extension: {}.\nExample:\n  {DIAGNOSTICS_FILE_EXAMPLE}",
        path.display()
    )))
}

fn path_metadata_error(path: &Path, error: io::Error) -> CliUsageError {
    let action = if error.kind() == io::ErrorKind::NotFound {
        "does not exist"
    } else {
        "cannot be inspected"
    };
    CliUsageError::new(format!(
        "Error: diagnostics path {action}: {}.\nExample:\n  {DIAGNOSTICS_FILE_EXAMPLE}\nCause: {error}",
        path.display()
    ))
}

fn display_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

impl CliDiagnostic {
    fn from_lsp(file: String, diagnostic: Diagnostic) -> Self {
        let topic = diagnostic_data_string(&diagnostic, "topic");
        let docs_url = diagnostic
            .code_description
            .as_ref()
            .map(|description| description.href.to_string())
            .or_else(|| topic.as_deref().and_then(topic_lint_doc_href));
        Self {
            file,
            range: diagnostic.range,
            code: diagnostic_code(&diagnostic),
            severity: severity_label(diagnostic.severity),
            topic,
            confidence: diagnostic_data_string(&diagnostic, "confidence"),
            applicability: diagnostic_data_string(&diagnostic, "applicability"),
            docs_url,
            message: diagnostic.message,
        }
    }
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<String> {
    diagnostic.code.as_ref().map(|code| match code {
        NumberOrString::Number(value) => value.to_string(),
        NumberOrString::String(value) => value.clone(),
    })
}

fn severity_label(severity: Option<DiagnosticSeverity>) -> SeverityLabel {
    match severity {
        Some(DiagnosticSeverity::ERROR) => SeverityLabel::Error,
        Some(DiagnosticSeverity::WARNING) => SeverityLabel::Warning,
        Some(DiagnosticSeverity::INFORMATION) => SeverityLabel::Information,
        Some(DiagnosticSeverity::HINT) => SeverityLabel::Hint,
        _ => SeverityLabel::Unknown,
    }
}

fn topic_lint_doc_href(topic: &str) -> Option<String> {
    let rest = topic.strip_prefix("seagrass/")?;
    let slug = format!("seagrass-{}", rest.replace(['.', '/'], "-"));
    Some(format!(
        "https://github.com/heyAyushh/seagrass/blob/main/docs/lints/{slug}.md"
    ))
}

fn diagnostic_data_string(diagnostic: &Diagnostic, key: &str) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests;
