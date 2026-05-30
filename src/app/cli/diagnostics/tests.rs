use {
    super::*,
    clap::CommandFactory,
    std::time::{SystemTime, UNIX_EPOCH},
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
        _json: false,
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
        _json: false,
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
        _json: false,
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

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}
