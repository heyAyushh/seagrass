use {
    crate::{diagnostics, document::ParsedDocument, server, workspace::WorkspaceIndex},
    clap::{Args, Parser, Subcommand},
    serde::Serialize,
    std::{
        error::Error,
        fs, io,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range, Url},
};

pub const USAGE_ERROR_EXIT_CODE: i32 = 2;
const FINDINGS_ERROR_EXIT_CODE: i32 = 1;
const RUST_EXTENSION: &str = "rs";

pub async fn run_from_env() -> Result<(), Box<dyn Error>> {
    let args = std::env::args_os().collect::<Vec<_>>();
    if args.len() <= 1 {
        server::run_stdio().await;
        return Ok(());
    }

    let has_error = Cli::parse_from(args).run()?;
    if has_error {
        std::process::exit(FINDINGS_ERROR_EXIT_CODE);
    }
    Ok(())
}

#[derive(Debug, Parser)]
#[command(
    name = "seagrass",
    bin_name = "seagrass",
    version = env!("CARGO_PKG_VERSION"),
    about = "Seagrass Anchor language tooling"
)]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

impl Cli {
    fn run(self) -> Result<bool, Box<dyn Error>> {
        match self.command {
            CliCommand::Diagnostics(command) => command.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    /// Run Seagrass diagnostics on a Rust file or directory and print JSON.
    Diagnostics(DiagnosticsCommand),
}

#[derive(Debug, Args)]
struct DiagnosticsCommand {
    /// Rust file or directory to analyze.
    path: PathBuf,

    /// Print structured JSON. This is the default output format.
    #[arg(long = "json")]
    _json: bool,
}

impl DiagnosticsCommand {
    fn run(self) -> Result<bool, Box<dyn Error>> {
        let diagnostics = diagnostics_for_path(&self.path)?;
        serde_json::to_writer_pretty(io::stdout(), &diagnostics)?;
        println!();
        Ok(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SeverityLabel::Error))
    }
}

#[derive(Debug, Serialize)]
struct CliDiagnostic {
    file: String,
    range: Range,
    code: Option<String>,
    severity: SeverityLabel,
    topic: Option<String>,
    confidence: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum SeverityLabel {
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
    let diagnostics = match ParsedDocument::parse(source.clone()) {
        Ok(document) => diagnostics::collect_with_workspace(&document, workspace_index),
        Err(error) => vec![diagnostics::diagnostic_from_parse_error_with_source(
            error, &source,
        )],
    };
    let file = display_path(path);
    Ok(diagnostics
        .into_iter()
        .map(|diagnostic| CliDiagnostic::from_lsp(file.clone(), diagnostic))
        .collect())
}

fn workspace_index_for_path(path: &Path) -> Option<WorkspaceIndex> {
    let roots = workspace_roots_for_path(path)
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
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }
    if metadata.is_dir() {
        let mut files = rust_files_in_dir(path)?;
        files.sort();
        return Ok(files);
    }
    Err(format!("unsupported diagnostics path: {}", path.display()).into())
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

fn display_path(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

impl CliDiagnostic {
    fn from_lsp(file: String, diagnostic: Diagnostic) -> Self {
        Self {
            file,
            range: diagnostic.range,
            code: diagnostic_code(&diagnostic),
            severity: severity_label(diagnostic.severity),
            topic: diagnostic_data_string(&diagnostic, "topic"),
            confidence: diagnostic_data_string(&diagnostic, "confidence"),
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

fn diagnostic_data_string(diagnostic: &Diagnostic, key: &str) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn serializes_security_diagnostic_for_agent_cli() {
        let source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostic = diagnostics::collect_with_workspace(&document, None)
            .into_iter()
            .next()
            .expect("security diagnostic");
        let serialized = CliDiagnostic::from_lsp("demo.rs".to_string(), diagnostic);

        assert_eq!(serialized.file, "demo.rs");
        assert_eq!(serialized.severity, SeverityLabel::Warning);
        assert_eq!(
            serialized.topic.as_deref(),
            Some("seagrass/security.signer.authorization")
        );
        assert_eq!(serialized.confidence.as_deref(), Some("authoritative"));
    }

    #[test]
    fn diagnostics_cli_uses_workspace_index_for_single_file() {
        let temp_root = unique_temp_dir("seagrass-cli-workspace-index");
        let instructions_dir = temp_root.join("programs/whirlpool/src/instructions");
        let state_dir = temp_root.join("programs/whirlpool/src/state");
        fs::create_dir_all(&instructions_dir).unwrap();
        fs::create_dir_all(&state_dir).unwrap();

        let handler_path = instructions_dir.join("close_bundled_position.rs");
        fs::write(
            &handler_path,
            r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<CloseBundledPosition>) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle.s.s;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}
"#,
        )
        .unwrap();
        fs::write(
            state_dir.join("position_bundle.rs"),
            r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
        )
        .unwrap();

        let diagnostics = diagnostics_for_path(&handler_path).unwrap();

        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("`position_bundle.s` does not resolve")),
            "single-file CLI diagnostics should use sibling workspace type evidence: {diagnostics:#?}"
        );

        let _ = fs::remove_dir_all(temp_root);
    }

    fn unique_temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
    }
}
