use {
    crate::{
        document::ParsedDocument,
        solana::{artifact_paths, idl},
        solana_project::{self, SolanaProgram},
    },
    cargo_toml::{DepsSet, Manifest},
    serde_json::Value,
    std::{
        collections::HashSet,
        path::{Path, PathBuf},
    },
    tower_lsp::lsp_types::Url,
};

mod files;
mod package_markers;

use self::{
    files::{
        path_to_string, read_limited_text, rust_files, shallow_files_named,
        shallow_files_with_extensions,
    },
    package_markers::{package_markers, PackageMarkers},
};

const MAX_SCAN_DEPTH: usize = 5;
const MAX_SCAN_FILES: usize = 256;
const MAX_TEST_FILES: usize = 96;
const MAX_SOURCE_FILES: usize = 128;
const IDL_SOURCE_TOO_LARGE_REASON: &str = "IDL source exceeds project file read limit";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcosystemReport {
    pub program: SolanaProgram,
    pub idl_sources: Vec<IdlSourceReport>,
    pub program_metadata: ProgramMetadataReport,
    pub test_harnesses: Vec<TestHarnessReport>,
    pub surfpool: SurfpoolReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdlSourceReport {
    pub kind: IdlSourceKind,
    pub status: EcosystemArtifactStatus,
    pub path: Option<PathBuf>,
    pub config_path: Option<PathBuf>,
    pub reason: Option<String>,
    pub program_name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdlSourceKind {
    Anchor,
    Codama,
    Shank,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcosystemArtifactStatus {
    Present,
    Missing,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramMetadataReport {
    pub status: ProgramMetadataStatus,
    pub package_marker: bool,
    pub payloads: Vec<ProgramMetadataPayload>,
    pub suggested_idl_command: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramMetadataStatus {
    NotConfigured,
    ConfiguredMissingPayload,
    Present,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgramMetadataPayload {
    pub path: PathBuf,
    pub seed: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestHarnessReport {
    pub kind: TestHarnessKind,
    pub status: TestHarnessStatus,
    pub dependency_marker: bool,
    pub test_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestHarnessKind {
    LiteSvm,
    Mollusk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestHarnessStatus {
    Configured,
    MissingTests,
    Detected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfpoolReport {
    pub status: SurfpoolStatus,
    pub config_paths: Vec<PathBuf>,
    pub script_marker: bool,
    pub deploy_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfpoolStatus {
    NotConfigured,
    Configured,
    ConfiguredMissingDeployArtifact,
}

#[derive(Debug, Default)]
struct WorkspaceHints {
    dependencies: HashSet<String>,
    package_markers: PackageMarkers,
    shank_source_marker: bool,
    litesvm_test_files: Vec<PathBuf>,
    mollusk_test_files: Vec<PathBuf>,
    surfpool_config_paths: Vec<PathBuf>,
}

impl EcosystemReport {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "idlSources": self.idl_sources.iter().map(IdlSourceReport::to_json).collect::<Vec<_>>(),
            "programMetadata": self.program_metadata.to_json(&self.program),
            "testHarnesses": self.test_harnesses.iter().map(TestHarnessReport::to_json).collect::<Vec<_>>(),
            "surfpool": self.surfpool.to_json(),
        })
    }
}

impl IdlSourceReport {
    pub fn to_json(&self) -> Value {
        serde_json::json!({
            "kind": self.kind.as_str(),
            "label": self.kind.label(),
            "status": self.status.as_str(),
            "path": self.path.as_ref().map(|path| path_to_string(path)),
            "configPath": self.config_path.as_ref().map(|path| path_to_string(path)),
            "programName": self.program_name,
            "address": self.address,
            "reason": self.reason,
        })
    }
}

impl IdlSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Anchor => "anchor",
            Self::Codama => "codama",
            Self::Shank => "shank",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Anchor => "Anchor IDL",
            Self::Codama => "Codama IDL",
            Self::Shank => "Shank IDL",
        }
    }
}

impl EcosystemArtifactStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
        }
    }
}

impl ProgramMetadataReport {
    fn to_json(&self, program: &SolanaProgram) -> Value {
        serde_json::json!({
            "status": self.status.as_str(),
            "packageMarker": self.package_marker,
            "payloads": self.payloads.iter().map(ProgramMetadataPayload::to_json).collect::<Vec<_>>(),
            "suggestedIdlCommand": self.suggested_idl_command,
            "canonical": program.id.is_some(),
        })
    }
}

impl ProgramMetadataStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not-configured",
            Self::ConfiguredMissingPayload => "configured-missing-payload",
            Self::Present => "present",
        }
    }
}

impl ProgramMetadataPayload {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "path": path_to_string(&self.path),
            "seed": self.seed,
        })
    }
}

impl TestHarnessReport {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "kind": self.kind.as_str(),
            "label": self.kind.label(),
            "status": self.status.as_str(),
            "dependencyMarker": self.dependency_marker,
            "testFiles": self.test_files.iter().map(|path| path_to_string(path)).collect::<Vec<_>>(),
        })
    }
}

impl TestHarnessKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiteSvm => "litesvm",
            Self::Mollusk => "mollusk",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LiteSvm => "LiteSVM",
            Self::Mollusk => "Mollusk",
        }
    }
}

impl TestHarnessStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured-with-tests",
            Self::MissingTests => "configured-missing-tests",
            Self::Detected => "detected-in-tests",
        }
    }
}

impl SurfpoolReport {
    fn to_json(&self) -> Value {
        serde_json::json!({
            "status": self.status.as_str(),
            "configPaths": self.config_paths.iter().map(|path| path_to_string(path)).collect::<Vec<_>>(),
            "scriptMarker": self.script_marker,
            "deployPath": path_to_string(&self.deploy_path),
        })
    }
}

impl SurfpoolStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotConfigured => "not-configured",
            Self::Configured => "configured",
            Self::ConfiguredMissingDeployArtifact => "configured-missing-deploy-artifact",
        }
    }
}

pub fn report_for_document(uri: &Url, document: &ParsedDocument) -> Option<EcosystemReport> {
    let program = solana_project::detect_for_document(uri, document)?;
    Some(report_for_program(&program))
}

pub fn report_for_program(program: &SolanaProgram) -> EcosystemReport {
    let hints = workspace_hints(&program.root, program.source_root.as_deref());
    let idl_sources = idl_sources(program, &hints);
    let program_metadata = program_metadata(program, &hints, &idl_sources);
    let test_harnesses = test_harnesses(&hints);
    let surfpool = surfpool_report(program, &hints);

    EcosystemReport {
        program: program.clone(),
        idl_sources,
        program_metadata,
        test_harnesses,
        surfpool,
    }
}

fn idl_sources(program: &SolanaProgram, hints: &WorkspaceHints) -> Vec<IdlSourceReport> {
    let mut sources = Vec::new();
    if let Some(anchor) = idl_report_from_path(
        IdlSourceKind::Anchor,
        artifact_paths::idl_file(&program.root, &program.name),
        None,
    ) {
        sources.push(anchor);
    }

    sources.extend(codama_idl_reports(program));
    if shank_configured(program, hints) {
        sources.push(shank_idl_report(program));
    }

    sources.sort_by(|left, right| {
        left.kind
            .as_str()
            .cmp(right.kind.as_str())
            .then_with(|| left.path.cmp(&right.path))
    });
    sources.dedup_by(|left, right| left.kind == right.kind && left.path == right.path);
    sources
}

fn codama_idl_reports(program: &SolanaProgram) -> Vec<IdlSourceReport> {
    let mut reports = Vec::new();
    for config_path in codama_config_paths(&program.root) {
        let Some(idl_path) = codama_config_idl_path(&config_path) else {
            reports.push(IdlSourceReport {
                kind: IdlSourceKind::Codama,
                status: EcosystemArtifactStatus::Invalid,
                path: None,
                config_path: Some(config_path),
                reason: Some("codama.json does not contain a string `idl` path".to_string()),
                program_name: None,
                address: None,
            });
            continue;
        };
        let path = config_path
            .parent()
            .map(|parent| parent.join(&idl_path))
            .unwrap_or(idl_path);
        if let Some(report) =
            idl_report_from_path(IdlSourceKind::Codama, path, Some(config_path.clone()))
        {
            reports.push(report);
        }
    }

    for candidate in [
        program
            .root
            .join("idls")
            .join(format!("{}.codama.json", program.name)),
        program
            .root
            .join("idls")
            .join(format!("{}.json", program.name)),
        program
            .root
            .join("idl")
            .join(format!("{}.codama.json", program.name)),
    ] {
        if let Some(report) = idl_report_from_path(IdlSourceKind::Codama, candidate, None) {
            reports.push(report);
        }
    }

    reports
}

fn shank_idl_report(program: &SolanaProgram) -> IdlSourceReport {
    let path = artifact_paths::idl_file(&program.root, &program.name);
    idl_report_from_path(IdlSourceKind::Shank, path.clone(), None).unwrap_or(IdlSourceReport {
        kind: IdlSourceKind::Shank,
        status: EcosystemArtifactStatus::Missing,
        path: Some(path),
        config_path: None,
        reason: Some(format!(
            "Shank markers were found, but the expected {} artifact is missing",
            artifact_paths::TARGET_IDL_DIR
        )),
        program_name: None,
        address: None,
    })
}

fn idl_report_from_path(
    kind: IdlSourceKind,
    path: PathBuf,
    config_path: Option<PathBuf>,
) -> Option<IdlSourceReport> {
    if !path.is_file() {
        return config_path.map(|config_path| IdlSourceReport {
            kind,
            status: EcosystemArtifactStatus::Missing,
            path: Some(path),
            config_path: Some(config_path),
            reason: Some(format!(
                "{} path configured but file is missing",
                kind.label()
            )),
            program_name: None,
            address: None,
        });
    }

    match read_limited_text(&path)
        .ok_or_else(|| IDL_SOURCE_TOO_LARGE_REASON.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|err| err.to_string()))
    {
        Ok(value) => {
            let actual_kind = classify_idl_format(&value).unwrap_or(kind);
            Some(IdlSourceReport {
                kind: actual_kind,
                status: EcosystemArtifactStatus::Present,
                path: Some(path),
                config_path,
                reason: None,
                program_name: idl::program_name(&value),
                address: idl::address(&value),
            })
        }
        Err(reason) => Some(IdlSourceReport {
            kind,
            status: EcosystemArtifactStatus::Invalid,
            path: Some(path),
            config_path,
            reason: Some(reason),
            program_name: None,
            address: None,
        }),
    }
}

fn classify_idl_format(value: &Value) -> Option<IdlSourceKind> {
    if value.get("standard").and_then(Value::as_str) == Some("codama")
        || value.get("kind").and_then(Value::as_str) == Some("rootNode")
        || value
            .get("program")
            .and_then(|program| program.get("kind"))
            .and_then(Value::as_str)
            == Some("programNode")
    {
        return Some(IdlSourceKind::Codama);
    }

    if value
        .get("metadata")
        .and_then(|metadata| metadata.get("origin"))
        .and_then(Value::as_str)
        == Some("shank")
        || value
            .get("metadata")
            .and_then(|metadata| metadata.get("address"))
            .is_some()
            && value.get("types").is_some()
            && value.get("accounts").is_some()
    {
        return Some(IdlSourceKind::Shank);
    }

    if value.get("instructions").is_some() || value.get("metadata").is_some() {
        return Some(IdlSourceKind::Anchor);
    }
    None
}

/// Validates that an untrusted program id is a well-formed Solana public key
/// before it is interpolated into a suggested shell command. Program ids reach
/// this layer from `declare_id!` and `Anchor.toml` in the analysed project, so
/// a value like `$(rm -rf ~)` must never be allowed into a command string.
fn is_valid_program_id(program_id: &str) -> bool {
    // base58 alphabet: omits 0, O, I, and l. A 32-byte key encodes to 32–44 chars.
    const BASE58_ALPHABET: &str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    const MIN_PUBKEY_LEN: usize = 32;
    const MAX_PUBKEY_LEN: usize = 44;
    (MIN_PUBKEY_LEN..=MAX_PUBKEY_LEN).contains(&program_id.len())
        && program_id.chars().all(|c| BASE58_ALPHABET.contains(c))
}

fn program_metadata(
    program: &SolanaProgram,
    hints: &WorkspaceHints,
    idl_sources: &[IdlSourceReport],
) -> ProgramMetadataReport {
    let payloads = program_metadata_payloads(&program.root, &program.name);
    let configured = hints.package_markers.program_metadata || !payloads.is_empty();
    let status = if !payloads.is_empty() {
        ProgramMetadataStatus::Present
    } else if configured {
        ProgramMetadataStatus::ConfiguredMissingPayload
    } else {
        ProgramMetadataStatus::NotConfigured
    };
    let suggested_idl_command = program
        .id
        .as_deref()
        .filter(|program_id| is_valid_program_id(program_id))
        .and_then(|program_id| {
            idl_sources.iter().find_map(|source| {
                (source.status == EcosystemArtifactStatus::Present)
                    .then_some(source.path.as_ref())
                    .flatten()
                    .map(|path| {
                        format!(
                            "npx @solana-program/program-metadata@latest write idl {program_id} {}",
                            path_to_string(path)
                        )
                    })
            })
        });

    ProgramMetadataReport {
        status,
        package_marker: hints.package_markers.program_metadata,
        payloads,
        suggested_idl_command,
    }
}

fn program_metadata_payloads(root: &Path, program_name: &str) -> Vec<ProgramMetadataPayload> {
    let mut payloads = Vec::new();
    for (path, seed) in [
        (root.join("security.json"), "security"),
        (root.join("security.txt"), "security"),
        (root.join("program-metadata.json"), "metadata"),
        (
            root.join("program-metadata")
                .join(format!("{program_name}.json")),
            "metadata",
        ),
        (
            root.join(".program-metadata")
                .join(format!("{program_name}.json")),
            "metadata",
        ),
        (
            root.join("target")
                .join("program-metadata")
                .join(format!("{program_name}.json")),
            "metadata",
        ),
    ] {
        if path.is_file() {
            payloads.push(ProgramMetadataPayload {
                path,
                seed: seed.to_string(),
            });
        }
    }
    payloads.sort_by(|left, right| left.path.cmp(&right.path));
    payloads.dedup_by(|left, right| left.path == right.path);
    payloads
}

fn test_harnesses(hints: &WorkspaceHints) -> Vec<TestHarnessReport> {
    let mut reports = Vec::new();
    let litesvm_dependency =
        hints.dependencies.contains("litesvm") || hints.dependencies.contains("litesvm-token");
    if litesvm_dependency || !hints.litesvm_test_files.is_empty() {
        reports.push(TestHarnessReport {
            kind: TestHarnessKind::LiteSvm,
            status: harness_status(litesvm_dependency, &hints.litesvm_test_files),
            dependency_marker: litesvm_dependency,
            test_files: hints.litesvm_test_files.clone(),
        });
    }

    let mollusk_dependency = hints.dependencies.contains("mollusk-svm")
        || hints.dependencies.contains("mollusk-svm-bencher")
        || hints.dependencies.contains("mollusk");
    if mollusk_dependency || !hints.mollusk_test_files.is_empty() {
        reports.push(TestHarnessReport {
            kind: TestHarnessKind::Mollusk,
            status: harness_status(mollusk_dependency, &hints.mollusk_test_files),
            dependency_marker: mollusk_dependency,
            test_files: hints.mollusk_test_files.clone(),
        });
    }
    reports
}

fn harness_status(dependency_marker: bool, test_files: &[PathBuf]) -> TestHarnessStatus {
    if dependency_marker && test_files.is_empty() {
        TestHarnessStatus::MissingTests
    } else if dependency_marker {
        TestHarnessStatus::Configured
    } else {
        TestHarnessStatus::Detected
    }
}

fn surfpool_report(program: &SolanaProgram, hints: &WorkspaceHints) -> SurfpoolReport {
    let deploy_path = artifact_paths::deploy_file(&program.root, &program.name);
    let configured = hints.package_markers.surfpool || !hints.surfpool_config_paths.is_empty();
    let status = if !configured {
        SurfpoolStatus::NotConfigured
    } else if deploy_path.is_file() {
        SurfpoolStatus::Configured
    } else {
        SurfpoolStatus::ConfiguredMissingDeployArtifact
    };

    SurfpoolReport {
        status,
        config_paths: hints.surfpool_config_paths.clone(),
        script_marker: hints.package_markers.surfpool,
        deploy_path,
    }
}

fn workspace_hints(root: &Path, source_root: Option<&Path>) -> WorkspaceHints {
    let mut hints = WorkspaceHints::default();
    for manifest_path in shallow_files_named(root, "Cargo.toml", MAX_SCAN_DEPTH, MAX_SCAN_FILES) {
        let Some(text) = read_limited_text(&manifest_path) else {
            continue;
        };
        let Ok(manifest) = Manifest::from_str(&text) else {
            continue;
        };
        extend_dependency_names(&manifest.dependencies, &mut hints.dependencies);
        extend_dependency_names(&manifest.dev_dependencies, &mut hints.dependencies);
        extend_dependency_names(&manifest.build_dependencies, &mut hints.dependencies);
        for target in manifest.target.values() {
            extend_dependency_names(&target.dependencies, &mut hints.dependencies);
            extend_dependency_names(&target.dev_dependencies, &mut hints.dependencies);
            extend_dependency_names(&target.build_dependencies, &mut hints.dependencies);
        }
    }

    hints.package_markers = package_markers(root);
    hints.shank_source_marker = source_root.is_some_and(source_has_shank_markers);
    hints.litesvm_test_files = test_files_containing(root, &["LiteSVM", "litesvm::"]);
    hints.mollusk_test_files = test_files_containing(root, &["Mollusk", "mollusk_svm"]);
    hints.surfpool_config_paths = surfpool_config_paths(root);
    hints
}

fn extend_dependency_names(dependencies: &DepsSet, names: &mut HashSet<String>) {
    for (name, dependency) in dependencies {
        names.insert(name.clone());
        if let Some(package) = dependency
            .detail()
            .and_then(|detail| detail.package.as_ref())
        {
            names.insert(package.clone());
        }
    }
}

fn shank_configured(program: &SolanaProgram, hints: &WorkspaceHints) -> bool {
    hints.shank_source_marker
        || hints.dependencies.iter().any(|dependency| {
            matches!(
                dependency.as_str(),
                "shank" | "shank-macro" | "shank-idl" | "shank-account" | "shank-instruction"
            )
        })
        || program.root.join("Shank.toml").is_file()
}

fn codama_config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for name in ["codama.json", "codama.config.json"] {
        let direct = root.join(name);
        if direct.is_file() {
            paths.push(direct);
        }
    }
    paths.extend(shallow_files_named(root, "codama.json", 3, 32));
    paths.extend(shallow_files_named(root, "codama.config.json", 3, 32));
    paths.sort();
    paths.dedup();
    paths
}

fn codama_config_idl_path(path: &Path) -> Option<PathBuf> {
    let text = read_limited_text(path)?;
    let value = serde_json::from_str::<Value>(&text).ok()?;
    value.get("idl").and_then(Value::as_str).map(PathBuf::from)
}

fn source_has_shank_markers(source_root: &Path) -> bool {
    rust_files(source_root, MAX_SOURCE_FILES)
        .into_iter()
        .filter_map(|path| read_limited_text(&path))
        .any(|text| {
            text.contains("ShankInstruction")
                || text.contains("ShankAccount")
                || text.contains("ShankBuilder")
                || text.contains("ShankContext")
                || text.contains("ShankType")
        })
}

fn test_files_containing(root: &Path, needles: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let tests_dir = root.join("tests");
    if tests_dir.is_dir() {
        files.extend(rust_files(&tests_dir, MAX_TEST_FILES));
    }
    for manifest in shallow_files_named(root, "Cargo.toml", MAX_SCAN_DEPTH, MAX_SCAN_FILES) {
        if let Some(package_root) = manifest.parent() {
            let tests_dir = package_root.join("tests");
            if tests_dir.is_dir() {
                files.extend(rust_files(&tests_dir, MAX_TEST_FILES));
            }
        }
    }
    files.sort();
    files.dedup();
    files
        .into_iter()
        .filter(|path| {
            read_limited_text(path)
                .is_some_and(|text| needles.iter().any(|needle| text.contains(needle)))
        })
        .collect()
}

fn surfpool_config_paths(root: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for name in [
        "surfpool.toml",
        ".surfpool.toml",
        "surfpool.yaml",
        "surfpool.yml",
        "Surfpool.toml",
    ] {
        let direct = root.join(name);
        if direct.is_file() {
            paths.push(direct);
        }
        paths.extend(shallow_files_named(root, name, 3, 32));
    }
    for path in shallow_files_with_extensions(root, &["tx", "txtx"], 3, 96) {
        if read_limited_text(&path).is_some_and(|text| text.contains("svm::deploy_program")) {
            paths.push(path);
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests;
