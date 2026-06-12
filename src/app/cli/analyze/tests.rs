use {
    super::*,
    crate::solana::{
        artifact_paths, program_artifacts::test_fixtures::sbf_elf_with_text_instruction_count,
    },
    clap::CommandFactory,
    std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    },
};

#[test]
fn analyze_help_describes_static_and_runtime_boundaries() {
    let mut command = super::super::Cli::command();
    let analyze_command = command
        .find_subcommand_mut("analyze")
        .expect("analyze subcommand");
    let mut help = Vec::new();

    analyze_command.write_long_help(&mut help).unwrap();
    let help = String::from_utf8(help).unwrap();

    assert!(help.contains(ANALYZE_FILE_EXAMPLE));
    assert!(help.contains("static codebase intelligence"));
    assert!(help.contains("Runtime CU and traffic sections stay"));
}

#[test]
fn analyze_reports_static_intelligence_without_claiming_runtime_data() {
    let temp_root = unique_temp_dir("seagrass-analyze-static-intelligence");
    let source_dir = temp_root.join("programs/demo/src");
    fs::create_dir_all(&source_dir).unwrap();
    let source_path = source_dir.join("lib.rs");
    fs::write(
        &source_path,
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
use super::*;

pub fn route(ctx: Context<Route>) -> Result<()> {
    call_external(&ctx)?;
    let _ = ctx.accounts.vault.key();
    Ok(())
}
}

pub fn call_external(ctx: &Context<Route>) -> Result<()> {
let _cpi = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
Ok(())
}

pub fn dead_helper(ctx: &Context<Route>) -> Result<()> {
let _cpi = CpiContext::new(ctx.accounts.dead_program.to_account_info(), ());
Ok(())
}

#[derive(Accounts)]
pub struct Route<'info> {
#[account(mut, seeds = [b"vault", authority.key().as_ref()], bump)]
pub vault: AccountInfo<'info>,
pub authority: Signer<'info>,
pub external_program: AccountInfo<'info>,
pub dead_program: AccountInfo<'info>,
}
"#,
    )
    .unwrap();

    let report = analyze_path(&source_path).unwrap();

    assert_eq!(report.runtime_evidence.status, "notConfigured");
    assert_eq!(report.static_layer.totals.files, 1);
    assert_eq!(report.static_layer.totals.instructions, 1);
    assert_eq!(report.static_layer.totals.pda_fields, 1);
    assert_eq!(report.static_layer.totals.cpi_program_usages, 1);
    assert_eq!(report.static_layer.totals.likely_compute_heavy_paths, 1);

    let file = report.static_layer.files.first().expect("file report");
    let compute = compute_analysis_json(file);
    let route = file
        .instructions
        .iter()
        .find(|instruction| instruction.name == "route")
        .expect("route instruction");
    assert_eq!(compute["status"].as_str(), Some("sourceOnly"));
    assert_eq!(
        compute["runtimeMeasurementStatus"].as_str(),
        Some("notConfigured")
    );
    assert!(compute_has_source_signal(&compute, "cpiProgramUsage"));
    assert!(compute_has_source_signal(&compute, "splitHelperCall"));
    assert!(compute["bytecode"]["sbfInstructionCount"].is_null());
    assert!(route.compute_review_reasons.contains(&"splitHelperCall"));
    assert!(route.compute_review_reasons.contains(&"cpiProgramUsage"));
    assert!(route
        .cpi_programs
        .iter()
        .any(|usage| usage.name == "external_program"));
    assert!(!route
        .cpi_programs
        .iter()
        .any(|usage| usage.name == "dead_program"));
    assert!(file
        .pda_seed_usage
        .iter()
        .any(|pda| pda.field == "vault" && pda.uses_bump));
    assert!(!file.diagnostics.iter().any(|diagnostic| {
        diagnostic.topic.as_deref() == Some("seagrass/security.cpi.program")
            && diagnostic.message.contains("dead_program")
    }));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn analyze_reports_static_bytecode_compute_floor_when_sbf_artifact_exists() {
    let temp_root = unique_temp_dir("seagrass-analyze-sbf-compute");
    let source_dir = temp_root.join("programs/demo/src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(artifact_paths::deploy_dir(&temp_root)).unwrap();
    fs::write(
        temp_root.join("Anchor.toml"),
        r#"
[programs.localnet]
demo = "Demo111111111111111111111111111111111"
"#,
    )
    .unwrap();
    let source_path = source_dir.join("lib.rs");
    fs::write(
        &source_path,
        r#"
use anchor_lang::prelude::*;

declare_id!("Demo111111111111111111111111111111111");

#[program]
pub mod demo {
use super::*;

pub fn route(ctx: Context<Route>) -> Result<()> {
    let _ = ctx.accounts.vault.key();
    Ok(())
}
}

#[derive(Accounts)]
pub struct Route<'info> {
#[account(mut)]
pub vault: AccountInfo<'info>,
}
"#,
    )
    .unwrap();
    fs::write(
        artifact_paths::deploy_file(&temp_root, "demo"),
        sbf_elf_with_text_instruction_count(5),
    )
    .unwrap();

    let report = analyze_path(&source_path).unwrap();
    let compute = compute_analysis_json(report.static_layer.files.first().expect("file report"));
    assert_eq!(compute["status"].as_str(), Some("staticBytecodeAvailable"));
    assert_eq!(compute["bytecode"]["sbfInstructionCount"].as_u64(), Some(5));
    assert_eq!(
        compute["bytecode"]["estimatedProgramExecutionCuFloor"].as_u64(),
        Some(5)
    );
    assert_eq!(compute["bytecode"]["textSectionBytes"].as_u64(), Some(40));
    assert_eq!(
        compute["runtimeMeasurementStatus"].as_str(),
        Some("notConfigured")
    );

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn analyze_directory_aggregates_multiple_files() {
    let temp_root = unique_temp_dir("seagrass-analyze-directory");
    fs::create_dir_all(&temp_root).unwrap();
    fs::write(
        temp_root.join("one.rs"),
        r#"
#[program]
pub mod one {
pub fn create(ctx: Context<Create>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
#[account(init)]
pub state: AccountInfo<'info>,
}
"#,
    )
    .unwrap();
    fs::write(
        temp_root.join("two.rs"),
        r#"
#[derive(Accounts)]
pub struct Empty<'info> {
pub authority: Signer<'info>,
}
"#,
    )
    .unwrap();

    let report = analyze_path(&temp_root).unwrap();
    assert_eq!(report.static_layer.totals.files, 2);
    assert_eq!(report.static_layer.totals.instructions, 1);
    assert_eq!(report.static_layer.totals.account_contexts, 2);
    assert!(report.static_layer.totals.diagnostics > 0);

    let _ = fs::remove_dir_all(temp_root);
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nanos}", std::process::id()))
}

fn compute_analysis_json(file: &FileIntelligenceReport) -> serde_json::Value {
    serde_json::to_value(&file.compute_analysis).expect("compute analysis serializes")
}

fn compute_has_source_signal(compute: &serde_json::Value, signal: &str) -> bool {
    compute["sourceSignals"]
        .as_array()
        .expect("source signals are serialized as an array")
        .iter()
        .any(|value| value.as_str() == Some(signal))
}
