use {
    super::*,
    clap::CommandFactory,
    std::time::{SystemTime, UNIX_EPOCH},
    tower_lsp::lsp_types::Diagnostic,
};

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
    let diagnostic = diagnostic_engine::collect_with_workspace(&document, None)
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

#[test]
fn diagnostics_help_includes_agent_examples() {
    let mut command = super::super::Cli::command();
    let diagnostics_command = command
        .find_subcommand_mut("diagnostics")
        .expect("diagnostics subcommand");
    let mut help = Vec::new();

    diagnostics_command.write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();

    assert!(help.contains("Examples:"));
    assert!(help.contains(DIAGNOSTICS_FILE_EXAMPLE));
    assert!(help.contains(DIAGNOSTICS_DIRECTORY_EXAMPLE));
    assert!(help.contains(DIAGNOSTICS_STDIN_EXAMPLE));
    assert!(help.contains("--stdin"));
}

#[test]
fn diagnostics_command_requires_path_or_stdin() {
    let error = DiagnosticsCommand {
        path: None,
        json: false,
        sarif: false,
        stdin: false,
        stdin_path: None,
    }
    .input()
    .unwrap_err()
    .to_string();

    assert!(error.contains("requires a Rust file/directory path or --stdin"));
    assert!(error.contains(DIAGNOSTICS_FILE_EXAMPLE));
    assert!(error.contains(DIAGNOSTICS_DIRECTORY_EXAMPLE));
    assert!(error.contains(DIAGNOSTICS_STDIN_EXAMPLE));
}

#[test]
fn diagnostics_command_rejects_path_with_stdin() {
    let error = DiagnosticsCommand {
        path: Some(PathBuf::from("programs/demo/src/lib.rs")),
        json: false,
        sarif: false,
        stdin: true,
        stdin_path: None,
    }
    .input()
    .unwrap_err()
    .to_string();

    assert!(error.contains("use either PATH or --stdin"));
    assert!(error.contains(DIAGNOSTICS_FILE_EXAMPLE));
    assert!(error.contains(DIAGNOSTICS_STDIN_EXAMPLE));
}

#[test]
fn diagnostics_command_rejects_stdin_path_without_stdin() {
    let error = DiagnosticsCommand {
        path: None,
        json: false,
        sarif: false,
        stdin: false,
        stdin_path: Some(PathBuf::from("programs/demo/src/lib.rs")),
    }
    .input()
    .unwrap_err()
    .to_string();

    assert!(error.contains("--stdin-path only applies with --stdin"));
    assert!(error.contains(DIAGNOSTICS_STDIN_EXAMPLE));
}

#[test]
fn diagnostics_cli_reads_stdin_source() {
    let diagnostics = diagnostics_for_stdin_source(None, "pub fn broken(".to_string()).unwrap();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.file == STDIN_DISPLAY_PATH));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == SeverityLabel::Error),
        "stdin parse errors should be surfaced as CLI diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn diagnostics_cli_honors_file_ignore_for_parse_errors() {
    let diagnostics =
        diagnostics_for_stdin_source(None, "// seagrass-ignore-file\npub fn broken(".to_string())
            .unwrap();

    assert!(
        diagnostics.is_empty(),
        "whole-file ignore should suppress CLI parse diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn diagnostics_cli_honors_cargo_package_metadata_suppress() {
    let temp_root = unique_temp_dir("seagrass-cli-cargo-suppress");
    let source_dir = temp_root.join("programs/demo/src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        temp_root.join("programs/demo/Cargo.toml"),
        r#"
[package]
name = "demo"
version = "0.1.0"

[package.metadata.seagrass]
suppress = true
"#,
    )
    .unwrap();
    let source_path = source_dir.join("lib.rs");
    fs::write(
        &source_path,
        r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Err(ProgramError::InvalidArgument).expect("invalid argument")
}
"#,
    )
    .unwrap();

    let diagnostics = diagnostics_for_path(&source_path).unwrap();

    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.topic.as_deref() != Some("seagrass/solana.code-quality.unsafe-unwrap")
        }),
        "Cargo.toml suppression should suppress CLI diagnostics: {diagnostics:#?}"
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn diagnostics_cli_rejects_missing_path_with_example() {
    let temp_root = unique_temp_dir("seagrass-cli-missing-path");
    let missing_path = temp_root.join("programs/demo/src/lib.rs");

    let error = rust_files(&missing_path).unwrap_err().to_string();

    assert!(error.contains("does not exist"));
    assert!(error.contains(&missing_path.display().to_string()));
    assert!(error.contains(DIAGNOSTICS_FILE_EXAMPLE));
}

#[test]
fn diagnostics_cli_rejects_non_rust_file() {
    let temp_root = unique_temp_dir("seagrass-cli-non-rust");
    fs::create_dir_all(&temp_root).unwrap();
    let readme_path = temp_root.join("README.md");
    fs::write(&readme_path, "# demo").unwrap();

    let error = rust_files(&readme_path).unwrap_err().to_string();

    assert!(error.contains(".rs extension"));
    assert!(error.contains(DIAGNOSTICS_FILE_EXAMPLE));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn diagnostics_cli_rejects_directory_without_rust_files() {
    let temp_root = unique_temp_dir("seagrass-cli-empty-dir");
    fs::create_dir_all(&temp_root).unwrap();

    let error = rust_files(&temp_root).unwrap_err().to_string();

    assert!(error.contains("no Rust source files found"));
    assert!(error.contains(DIAGNOSTICS_DIRECTORY_EXAMPLE));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn cli_diagnostic_includes_docs_url_for_seagrass_topic() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostic = diagnostic_engine::collect_with_workspace(&document, None)
        .into_iter()
        .find(|diagnostic| {
            diagnostic_data_string(diagnostic, "topic")
                .is_some_and(|topic| topic.contains("anchor.init"))
        })
        .expect("init companion diagnostic");
    let serialized = CliDiagnostic::from_lsp("smoke.rs".to_string(), diagnostic);
    assert!(
        serialized
            .docs_url
            .as_deref()
            .is_some_and(|url| url.contains("docs/lints/seagrass-anchor-init")),
        "expected lint catalog docsUrl, got {:?}",
        serialized.docs_url
    );
}

#[test]
fn diagnostics_command_emits_sarif_log() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let file = std::env::temp_dir().join("seagrass-sarif-smoke.rs");
    let diagnostics = diagnostic_engine::collect_with_workspace(&document, None)
        .into_iter()
        .map(|diagnostic| CliDiagnostic::from_lsp(file.display().to_string(), diagnostic))
        .collect::<Vec<_>>();
    let mut buffer = Vec::new();
    serde_json::to_writer_pretty(&mut buffer, &sarif_log(&diagnostics)).unwrap();
    let stdout = String::from_utf8(buffer).unwrap();
    assert!(stdout.contains("\"version\": \"2.1.0\""));
    assert!(stdout.contains("\"name\": \"seagrass\""));

    let sarif = serde_json::from_str::<serde_json::Value>(&stdout).unwrap();
    let driver = &sarif["runs"][0]["tool"]["driver"];
    assert!(driver.get("informationUri").is_some());
    assert!(driver.get("information_uri").is_none());
    assert!(driver["rules"][0].get("shortDescription").is_some());
    assert!(driver["rules"][0].get("short_description").is_none());

    let result = &sarif["runs"][0]["results"][0];
    assert!(result.get("ruleId").is_some());
    assert!(result.get("rule_id").is_none());
    let physical_location = &result["locations"][0]["physicalLocation"];
    assert!(physical_location.get("artifactLocation").is_some());
    assert!(physical_location.get("physical_location").is_none());
    assert!(
        physical_location["artifactLocation"]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("file://")),
        "expected SARIF artifactLocation.uri to be a file URI, got {:?}",
        physical_location["artifactLocation"]["uri"]
    );
    assert!(physical_location["region"].get("startLine").is_some());
    assert!(physical_location["region"].get("start_line").is_none());
}

fn sarif_log(diagnostics: &[CliDiagnostic]) -> serde_json::Value {
    let mut sink = Vec::new();
    super::super::sarif::write_sarif_to_writer(diagnostics, &mut sink).unwrap();
    serde_json::from_slice(&sink).unwrap()
}

fn diagnostic_data_string(diagnostic: &Diagnostic, key: &str) -> Option<String> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get(key))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}
