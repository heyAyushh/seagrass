use {
    super::diagnostics::CliDiagnostic,
    crate::{
        diagnostics as diagnostic_engine, document::ParsedDocument, evidence, file_text,
        program_artifacts, solana_project, workspace::WorkspaceIndex,
    },
    clap::Args,
    serde::Serialize,
    std::{
        collections::{BTreeMap, BTreeSet, HashSet},
        error::Error,
        fmt,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::Url,
};

mod compute;
mod input;

use compute::{compute_analysis_report, ComputeAnalysisReport};
use input::{display_path, rust_files, workspace_index_for_path};

const RUST_EXTENSION: &str = "rs";
const ANALYZE_FILE_EXAMPLE: &str = "seagrass analyze programs/demo/src/lib.rs --json";
const ANALYZE_DIRECTORY_EXAMPLE: &str = "seagrass analyze programs/demo/src --json";

pub(super) const ANALYZE_HELP: &str = "\
Examples:
  seagrass analyze programs/demo/src/lib.rs --json
  seagrass analyze programs/demo/src --json

Output:
  Prints a JSON object with static codebase intelligence derived from parsed Rust,
  workspace reachability, diagnostics, PDA/seed evidence, CPI/signing fanout, and
  Solana project classification. Runtime CU and traffic sections stay
  notConfigured until explicit telemetry or coverage evidence is imported.";

#[derive(Debug, Args)]
pub(super) struct AnalyzeCommand {
    /// Rust file or directory to analyze.
    #[arg(value_name = "PATH")]
    path: PathBuf,

    /// Print structured JSON. This is the default output format.
    #[arg(long = "json")]
    json: bool,
}

impl AnalyzeCommand {
    pub(super) fn run(self) -> Result<bool, Box<dyn Error>> {
        let report = analyze_path(&self.path)?;
        serde_json::to_writer_pretty(std::io::stdout(), &report)?;
        println!();
        Ok(false)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodebaseIntelligenceReport {
    schema_version: u8,
    path: String,
    static_layer: StaticLayerReport,
    runtime_evidence: RuntimeEvidenceReport,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticLayerReport {
    source_of_truth: Vec<&'static str>,
    files: Vec<FileIntelligenceReport>,
    totals: StaticTotals,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct StaticTotals {
    files: usize,
    instructions: usize,
    account_contexts: usize,
    cpi_program_usages: usize,
    signer_usages: usize,
    signer_checks: usize,
    pda_fields: usize,
    static_seed_pdas: usize,
    diagnostics: usize,
    diagnostics_by_topic: BTreeMap<String, usize>,
    likely_compute_heavy_paths: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FileIntelligenceReport {
    file: String,
    project: Option<ProjectReport>,
    account_flow: serde_json::Value,
    instructions: Vec<InstructionIntelligenceReport>,
    compute_analysis: ComputeAnalysisReport,
    accounts: Vec<AccountContextReport>,
    pda_seed_usage: Vec<PdaSeedUsageReport>,
    framework_boundary: FrameworkBoundaryReport,
    diagnostics: Vec<CliDiagnostic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectReport {
    kind: &'static str,
    name: String,
    program_id: Option<String>,
    root: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstructionIntelligenceReport {
    name: String,
    context: Option<String>,
    account_usages: Vec<AccountUsageReport>,
    cpi_programs: Vec<AccountUsageReport>,
    signer_usages: Vec<AccountUsageReport>,
    signer_checks: Vec<AccountUsageReport>,
    function_calls: Vec<String>,
    likely_compute_heavy: bool,
    compute_review_reasons: Vec<&'static str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountUsageReport {
    name: String,
    mutable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountContextReport {
    name: String,
    field_count: usize,
    used_by_instructions: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PdaSeedUsageReport {
    accounts: String,
    field: String,
    uses_bump: bool,
    static_only_seeds: bool,
    seed_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameworkBoundaryReport {
    program_kind: Option<&'static str>,
    anchor_only_surfaces_applicable: bool,
    boundary: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEvidenceReport {
    status: &'static str,
    compute_units: RuntimeEvidenceBoundary,
    traffic_shape: RuntimeEvidenceBoundary,
    cold_path_deletion: RuntimeEvidenceBoundary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeEvidenceBoundary {
    status: &'static str,
    required_evidence: Vec<&'static str>,
}

#[derive(Debug)]
struct AnalyzeUsageError {
    message: String,
}

impl AnalyzeUsageError {
    fn new(message: String) -> Self {
        Self { message }
    }
}

impl fmt::Display for AnalyzeUsageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for AnalyzeUsageError {}

fn analyze_path(path: &Path) -> Result<CodebaseIntelligenceReport, Box<dyn Error>> {
    let workspace_index = workspace_index_for_path(path);
    let files = rust_files(path)?
        .into_iter()
        .map(|file| analyze_file(&file, workspace_index.as_ref()))
        .collect::<Result<Vec<_>, _>>()?;
    let totals = static_totals(&files);

    Ok(CodebaseIntelligenceReport {
        schema_version: 1,
        path: display_path(path),
        static_layer: StaticLayerReport {
            source_of_truth: vec![
                "parsedRust",
                "workspaceReachability",
                "generatedConstraintCatalogs",
                "localArtifacts",
                "diagnostics",
            ],
            files,
            totals,
        },
        runtime_evidence: runtime_evidence_report(),
    })
}

fn analyze_file(
    path: &Path,
    workspace_index: Option<&WorkspaceIndex>,
) -> Result<FileIntelligenceReport, Box<dyn Error>> {
    let source = file_text::read_limited_text(path)?.ok_or_else(|| {
        AnalyzeUsageError::new(format!(
            "source file exceeds {} bytes: {}",
            file_text::MAX_PROJECT_FILE_BYTES,
            path.display()
        ))
    })?;
    let document = ParsedDocument::parse(source)?;
    let diagnostics = diagnostic_engine::collect_with_workspace(&document, workspace_index);
    let file = display_path(path);
    let uri = Url::from_file_path(path).ok();
    let program = uri
        .as_ref()
        .and_then(|uri| solana_project::detect_for_document(uri, &document));
    let artifact_report = program
        .clone()
        .and_then(program_artifacts::report_for_program);
    let program_kind = program.as_ref().map(|program| program.kind.as_str());
    let project = program.map(project_report);
    let instructions = instruction_reports(&document, workspace_index);
    let compute_analysis = compute_analysis_report(&instructions, artifact_report.as_ref());

    Ok(FileIntelligenceReport {
        file: file.clone(),
        project,
        account_flow: evidence::summary(&document),
        instructions,
        compute_analysis,
        accounts: account_context_reports(&document),
        pda_seed_usage: pda_seed_usage_reports(&document),
        framework_boundary: framework_boundary_report(program_kind),
        diagnostics: diagnostics
            .into_iter()
            .map(|diagnostic| CliDiagnostic::from_lsp(file.clone(), diagnostic))
            .collect(),
    })
}

fn project_report(program: solana_project::SolanaProgram) -> ProjectReport {
    ProjectReport {
        kind: program.kind.as_str(),
        name: program.name,
        program_id: program.id,
        root: program.root.to_string_lossy().into_owned(),
    }
}

fn instruction_reports(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<InstructionIntelligenceReport> {
    document
        .symbols()
        .instructions
        .iter()
        .map(|instruction| {
            let context_name = instruction
                .context
                .as_ref()
                .map(|context| context.name.as_str());
            let cpi_programs = account_usage_reports_with_reachable_names(
                &reachable_account_usages_for_instruction(document, instruction, |function| {
                    &function.cpi_program_usages
                }),
                context_name.and_then(|context| {
                    workspace_index.and_then(|index| {
                        index.reachable_cpi_program_usage_names_for_context(context)
                    })
                }),
            );
            let signer_usages = account_usage_reports_with_reachable_names(
                &reachable_account_usages_for_instruction(document, instruction, |function| {
                    &function.signer_usages
                }),
                context_name.and_then(|context| {
                    workspace_index
                        .and_then(|index| index.reachable_signer_usage_names_for_context(context))
                }),
            );
            let signer_checks = account_usage_reports_with_reachable_names(
                &reachable_account_usages_for_instruction(document, instruction, |function| {
                    &function.signer_checks
                }),
                context_name.and_then(|context| {
                    workspace_index
                        .and_then(|index| index.reachable_signer_check_names_for_context(context))
                }),
            );
            let function_calls = instruction
                .function_calls
                .iter()
                .map(|call| call.name.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let compute_review_reasons = compute_review_reasons(instruction, &cpi_programs);
            InstructionIntelligenceReport {
                name: instruction.name.clone(),
                context: instruction
                    .context
                    .as_ref()
                    .map(|context| context.name.clone()),
                account_usages: account_usage_reports(&instruction.account_usages),
                cpi_programs,
                signer_usages,
                signer_checks,
                function_calls,
                likely_compute_heavy: !compute_review_reasons.is_empty(),
                compute_review_reasons,
            }
        })
        .collect()
}

fn compute_review_reasons(
    instruction: &crate::document::InstructionSymbol,
    cpi_programs: &[AccountUsageReport],
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if !cpi_programs.is_empty() {
        reasons.push("cpiProgramUsage");
    }
    if instruction.account_usages.iter().any(|usage| usage.mutable) {
        reasons.push("mutableAccountFlow");
    }
    if !instruction.function_calls.is_empty() {
        reasons.push("splitHelperCall");
    }
    reasons
}

fn reachable_account_usages_for_instruction(
    document: &ParsedDocument,
    instruction: &crate::document::InstructionSymbol,
    usages: impl Fn(&crate::document::InstructionSymbol) -> &[crate::document::AccountUsage],
) -> Vec<crate::document::AccountUsage> {
    let mut reachable_usages = usages(instruction).to_vec();
    let mut pending = instruction
        .function_calls
        .iter()
        .map(|call| call.name.clone())
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();

    while let Some(function_name) = pending.pop() {
        if !visited.insert(function_name.clone()) {
            continue;
        }
        let Some(function) = document
            .symbols()
            .functions
            .iter()
            .find(|function| function.name == function_name)
        else {
            continue;
        };
        reachable_usages.extend(usages(function).iter().cloned());
        pending.extend(function.function_calls.iter().map(|call| call.name.clone()));
    }

    reachable_usages
}

fn account_usage_reports(usages: &[crate::document::AccountUsage]) -> Vec<AccountUsageReport> {
    usages
        .iter()
        .map(|usage| (usage.name.as_str(), usage.mutable))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|(name, mutable)| AccountUsageReport {
            name: name.to_string(),
            mutable,
        })
        .collect()
}

fn account_usage_reports_with_reachable_names(
    usages: &[crate::document::AccountUsage],
    reachable_names: Option<HashSet<String>>,
) -> Vec<AccountUsageReport> {
    let mut reports = account_usage_reports(usages)
        .into_iter()
        .map(|usage| (usage.name, usage.mutable))
        .collect::<BTreeMap<_, _>>();
    if let Some(reachable_names) = reachable_names {
        for name in reachable_names {
            reports.entry(name).or_insert(false);
        }
    }
    reports
        .into_iter()
        .map(|(name, mutable)| AccountUsageReport { name, mutable })
        .collect()
}

fn account_context_reports(document: &ParsedDocument) -> Vec<AccountContextReport> {
    document
        .symbols()
        .accounts_structs
        .values()
        .map(|accounts| {
            let used_by_instructions = document
                .symbols()
                .instructions
                .iter()
                .filter(|instruction| {
                    instruction
                        .context
                        .as_ref()
                        .is_some_and(|context| context.name == accounts.name)
                })
                .map(|instruction| instruction.name.clone())
                .collect();
            AccountContextReport {
                name: accounts.name.clone(),
                field_count: accounts.fields.len(),
                used_by_instructions,
            }
        })
        .collect()
}

fn pda_seed_usage_reports(document: &ParsedDocument) -> Vec<PdaSeedUsageReport> {
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| {
            accounts.fields.iter().filter_map(|field| {
                field.pda_constraint.as_ref().map(|pda| PdaSeedUsageReport {
                    accounts: accounts.name.clone(),
                    field: field.name.clone(),
                    uses_bump: !matches!(pda.bump, crate::document::PdaBump::Missing),
                    static_only_seeds: pda_seeds_are_static_only(&pda.seeds),
                    seed_count: pda_seed_count(&pda.seeds),
                })
            })
        })
        .collect()
}

fn pda_seeds_are_static_only(seeds: &crate::document::PdaSeeds) -> bool {
    let crate::document::PdaSeeds::List(seeds) = seeds else {
        return false;
    };
    !seeds.is_empty()
        && seeds
            .iter()
            .all(|seed| seed.starts_with("b\"") || seed.starts_with("b'"))
}

fn pda_seed_count(seeds: &crate::document::PdaSeeds) -> usize {
    match seeds {
        crate::document::PdaSeeds::List(seeds) => seeds.len(),
        crate::document::PdaSeeds::Expr(_) => 1,
    }
}

fn framework_boundary_report(program_kind: Option<&'static str>) -> FrameworkBoundaryReport {
    FrameworkBoundaryReport {
        program_kind,
        anchor_only_surfaces_applicable: program_kind == Some("anchor") || program_kind.is_none(),
        boundary: "Anchor-only account constraint surfaces are non-applicable for native Solana and Pinocchio unless equivalent parsed evidence exists.",
    }
}

fn static_totals(files: &[FileIntelligenceReport]) -> StaticTotals {
    let mut totals = StaticTotals {
        files: files.len(),
        ..StaticTotals::default()
    };

    for file in files {
        totals.instructions += file.instructions.len();
        totals.account_contexts += file.accounts.len();
        totals.pda_fields += file.pda_seed_usage.len();
        totals.static_seed_pdas += file
            .pda_seed_usage
            .iter()
            .filter(|pda| pda.static_only_seeds)
            .count();
        totals.diagnostics += file.diagnostics.len();
        totals.likely_compute_heavy_paths += file
            .instructions
            .iter()
            .filter(|instruction| instruction.likely_compute_heavy)
            .count();

        for instruction in &file.instructions {
            totals.cpi_program_usages += instruction.cpi_programs.len();
            totals.signer_usages += instruction.signer_usages.len();
            totals.signer_checks += instruction.signer_checks.len();
        }

        for diagnostic in &file.diagnostics {
            if let Some(topic) = diagnostic.topic.as_ref() {
                *totals
                    .diagnostics_by_topic
                    .entry(topic.clone())
                    .or_default() += 1;
            }
        }
    }

    totals
}

fn runtime_evidence_report() -> RuntimeEvidenceReport {
    RuntimeEvidenceReport {
        status: "notConfigured",
        compute_units: RuntimeEvidenceBoundary {
            status: "notConfigured",
            required_evidence: vec![
                "tridentCoverage",
                "validatorLogs",
                "simulatorTrace",
                "productionTelemetry",
            ],
        },
        traffic_shape: RuntimeEvidenceBoundary {
            status: "notConfigured",
            required_evidence: vec!["indexerTelemetry", "applicationTelemetry"],
        },
        cold_path_deletion: RuntimeEvidenceBoundary {
            status: "notConfigured",
            required_evidence: vec!["coverageWindow", "trafficWindow", "traceWindow"],
        },
    }
}

#[cfg(test)]
mod tests {
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
        let compute =
            compute_analysis_json(report.static_layer.files.first().expect("file report"));

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
}
