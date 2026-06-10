use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        program_artifacts::{
            self, ArtifactInput, DeployArtifactState, FilePresence, IdlArtifactState,
            ProgramArtifactReport, ProgramKeypairState,
        },
        solana::artifact_paths,
        solana_project::{SolanaProgram, SolanaProjectKind},
    },
    std::{path::Path, time::SystemTime},
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url},
};

pub fn collect(
    document: &ParsedDocument,
    uri: &Url,
    program: Option<&SolanaProgram>,
) -> Vec<Diagnostic> {
    let Some(report) = program
        .cloned()
        .and_then(program_artifacts::report_for_program)
        .or_else(|| program_artifacts::report_for_document(uri, document))
    else {
        return Vec::new();
    };

    if !report.build_surface_exists() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    diagnostics.extend(deploy_diagnostics(document, &report));
    diagnostics.extend(keypair_diagnostics(document, &report));
    diagnostics.extend(idl_diagnostics(document, &report));
    diagnostics.extend(typescript_diagnostics(document, &report));
    diagnostics
}

fn deploy_diagnostics(
    document: &ParsedDocument,
    report: &ProgramArtifactReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match &report.deploy {
        DeployArtifactState::Missing => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorSbfArtifact,
                format!(
                    "No local SBPF artifact found for `{}` at `{}`; `{}` normally writes the program ELF there.",
                    report.program.name,
                    display_path(&report.deploy_path),
                    report.program.kind.build_command()
                ),
                "missing",
                Some(&report.deploy_path),
                None,
            ));
        }
        DeployArtifactState::Invalid { reason, .. } => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorSbfArtifact,
                format!(
                    "Local SBPF artifact for `{}` is not a valid Solana SBPF ELF: {reason}.",
                    report.program.name
                ),
                "invalid",
                Some(&report.deploy_path),
                Some(reason),
            ));
        }
        DeployArtifactState::Present { .. } => {
            if let Some(newest) = newest_input(report.deploy_stale_inputs()) {
                diagnostics.push(stale_diagnostic(
                    document,
                    report,
                    AnchorDiagnosticKind::AnchorSbfArtifact,
                    "deploy",
                    &report.deploy_path,
                    newest,
                ));
            }
        }
    }
    diagnostics
}

fn keypair_diagnostics(
    document: &ParsedDocument,
    report: &ProgramArtifactReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match &report.keypair {
        ProgramKeypairState::Missing if report.deploy_file().is_some() => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorProgramKeypair,
                format!(
                    "No local program keypair found for `{}` at `{}`; Solana build/deploy workflows normally keep the program keypair beside the SBPF artifact.",
                    report.program.name,
                    display_path(&report.keypair_path)
                ),
                "missing",
                Some(&report.keypair_path),
                None,
            ));
        }
        ProgramKeypairState::Invalid { reason, .. } => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorProgramKeypair,
                format!(
                    "Local program keypair for `{}` could not be read: {reason}.",
                    report.program.name
                ),
                "invalid",
                Some(&report.keypair_path),
                Some(reason),
            ));
        }
        ProgramKeypairState::Present { public_key, .. } => {
            let expected = document
                .symbols()
                .declared_program_id
                .as_ref()
                .map(|declared| declared.value.as_str())
                .or(report.program.id.as_deref());
            if expected.is_some_and(|expected| public_key != expected) {
                diagnostics.push(artifact_diagnostic(
                    document,
                    report,
                    AnchorDiagnosticKind::AnchorProgramKeypair,
                    format!(
                        "Local program keypair public key `{public_key}` does not match `{}` for `{}`.",
                        expected.unwrap_or("unknown"),
                        report.program.name
                    ),
                    "mismatch",
                    Some(&report.keypair_path),
                    None,
                ));
            }
        }
        ProgramKeypairState::Missing => {}
    }
    diagnostics
}

fn idl_diagnostics(document: &ParsedDocument, report: &ProgramArtifactReport) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match &report.idl {
        IdlArtifactState::Missing if should_expect_idl(report) => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorIdlArtifact,
                format!(
                    "No local {} found for `{}` at `{}`; `{}` normally generates this file.",
                    report.program.kind.idl_label(),
                    report.program.name,
                    display_path(&report.idl_path),
                    report.program.kind.build_command()
                ),
                "missing",
                Some(&report.idl_path),
                None,
            ));
        }
        IdlArtifactState::Invalid { reason, .. } => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorIdlArtifact,
                format!(
                    "Local {} for `{}` is not valid JSON: {reason}.",
                    report.program.kind.idl_label(),
                    report.program.name
                ),
                "invalid",
                Some(&report.idl_path),
                Some(reason),
            ));
        }
        IdlArtifactState::Present {
            program_name,
            address,
            ..
        } => {
            if let Some(name) = program_name {
                if crate::project::normalize_program_name(name) != report.program.name {
                    diagnostics.push(artifact_diagnostic(
                        document,
                        report,
                        AnchorDiagnosticKind::AnchorIdlArtifact,
                        format!(
                            "Local {} name `{name}` does not match program `{}`.",
                            report.program.kind.idl_label(),
                            report.program.name
                        ),
                        "mismatch",
                        Some(&report.idl_path),
                        None,
                    ));
                }
            }
            if let (Some(address), Some(program_id)) = (address, report.program.id.as_deref()) {
                if address != program_id {
                    diagnostics.push(artifact_diagnostic(
                        document,
                        report,
                        AnchorDiagnosticKind::AnchorIdlArtifact,
                        format!(
                            "Local {} address `{address}` does not match program id `{program_id}` for `{}`.",
                            report.program.kind.idl_label(),
                            report.program.name
                        ),
                        "mismatch",
                        Some(&report.idl_path),
                        None,
                    ));
                }
            }
            if let Some(newest) = newest_input(report.idl_stale_inputs()) {
                diagnostics.push(stale_diagnostic(
                    document,
                    report,
                    AnchorDiagnosticKind::AnchorIdlArtifact,
                    "idl",
                    &report.idl_path,
                    newest,
                ));
            }
        }
        IdlArtifactState::Missing => {}
    }
    diagnostics
}

fn typescript_diagnostics(
    document: &ParsedDocument,
    report: &ProgramArtifactReport,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    match &report.typescript {
        FilePresence::Missing if should_expect_typescript(report) => {
            diagnostics.push(artifact_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::AnchorTypesArtifact,
                format!(
                    "No {} found for `{}` at `{}`; `{}` normally writes client types there.",
                    report.program.kind.types_label(),
                    report.program.name,
                    display_path(&report.types_path),
                    report.program.kind.build_command()
                ),
                "missing",
                Some(&report.types_path),
                None,
            ));
        }
        FilePresence::Present(_) => {
            if let Some(newest) = newest_input(report.typescript_stale_inputs()) {
                diagnostics.push(stale_diagnostic(
                    document,
                    report,
                    AnchorDiagnosticKind::AnchorTypesArtifact,
                    "typescript",
                    &report.types_path,
                    newest,
                ));
            }
        }
        FilePresence::Missing => {}
    }
    diagnostics
}

fn artifact_diagnostic(
    document: &ParsedDocument,
    report: &ProgramArtifactReport,
    kind: AnchorDiagnosticKind,
    message: String,
    reason: &'static str,
    artifact_path: Option<&Path>,
    detail: Option<&str>,
) -> Diagnostic {
    diagnostic_from_range_with_related(
        diagnostic_range(document),
        kind,
        message,
        Some(serde_json::json!({
            "program": report.program.name,
            "cluster": report.program.cluster,
            "programId": report.program.id,
            "programKind": report.program.kind.as_str(),
            "framework": report.program.kind.label(),
            "artifact": artifact_path.map(display_path),
            "artifactUri": artifact_path.and_then(file_uri_string),
            "reason": reason,
            "detail": detail,
            "buildCommand": report.program.kind.build_command(),
        })),
        Some(related_information(report, artifact_path, None)),
    )
}

fn stale_diagnostic(
    document: &ParsedDocument,
    report: &ProgramArtifactReport,
    kind: AnchorDiagnosticKind,
    artifact_kind: &'static str,
    artifact_path: &Path,
    newest: &ArtifactInput,
) -> Diagnostic {
    diagnostic_from_range_with_related(
        diagnostic_range(document),
        kind,
        format!(
            "Local {} artifact for `{}` is older than `{}`; generated build evidence may be stale.",
            artifact_kind,
            report.program.name,
            display_path(&newest.path)
        ),
        Some(serde_json::json!({
            "program": report.program.name,
            "cluster": report.program.cluster,
            "programId": report.program.id,
            "programKind": report.program.kind.as_str(),
            "framework": report.program.kind.label(),
            "artifact": display_path(artifact_path),
            "artifactUri": file_uri_string(artifact_path),
            "newerInput": display_path(&newest.path),
            "newerInputUri": file_uri_string(&newest.path),
            "reason": "stale",
            "buildCommand": report.program.kind.build_command(),
        })),
        Some(related_information(
            report,
            Some(artifact_path),
            Some(newest),
        )),
    )
}

fn related_information(
    report: &ProgramArtifactReport,
    artifact_path: Option<&Path>,
    newer_input: Option<&ArtifactInput>,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related = vec![DiagnosticRelatedInformation {
        location: Location {
            uri: report.program.metadata_uri.clone(),
            range: report.program.metadata_range,
        },
        message: metadata_message(report),
    }];
    if let Some(path) = artifact_path.and_then(file_uri) {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: path,
                range: Range::default(),
            },
            message: "Expected local Solana build artifact.".to_string(),
        });
    }
    if let Some(input) = newer_input.and_then(|input| file_uri(&input.path).map(|uri| (input, uri)))
    {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri: input.1,
                range: Range::default(),
            },
            message: "Newer source input for this artifact.".to_string(),
        });
    }
    related
}

fn diagnostic_range(document: &ParsedDocument) -> Range {
    document
        .symbols()
        .declared_program_id
        .as_ref()
        .map(|declared| declared.range)
        .or_else(|| {
            document
                .symbols()
                .instructions
                .first()
                .map(|instruction| instruction.selection_range)
        })
        .unwrap_or_default()
}

fn should_expect_idl(report: &ProgramArtifactReport) -> bool {
    (report.program.kind == SolanaProjectKind::Anchor && report.deploy_file().is_some())
        || artifact_paths::idl_dir(&report.root).is_dir()
}

fn should_expect_typescript(report: &ProgramArtifactReport) -> bool {
    (report.program.kind == SolanaProjectKind::Anchor && report.idl_file().is_some())
        || artifact_paths::types_dir(&report.root).is_dir()
}

fn metadata_message(report: &ProgramArtifactReport) -> String {
    if report.program.kind == SolanaProjectKind::Anchor {
        return format!(
            "Anchor.toml declares `{}` for `{}` on `{}`.",
            report.program.id_display(),
            report.program.name,
            report.program.cluster_display()
        );
    }

    match report.program.id.as_deref() {
        Some(program_id) => format!(
            "Cargo.toml identifies {} program `{}`; `declare_id!` is `{program_id}`.",
            report.program.kind.label(),
            report.program.name
        ),
        None => format!(
            "Cargo.toml identifies {} program `{}`.",
            report.program.kind.label(),
            report.program.name
        ),
    }
}

fn newest_input(inputs: Vec<&ArtifactInput>) -> Option<&ArtifactInput> {
    inputs.into_iter().max_by_key(|input| {
        input
            .modified
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    })
}

fn file_uri(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

fn file_uri_string(path: &Path) -> Option<String> {
    file_uri(path).map(|uri| uri.to_string())
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        std::{env, fs, path::PathBuf, thread, time::Duration},
        tower_lsp::lsp_types::NumberOrString,
    };

    #[test]
    fn stays_quiet_before_local_build_surface_exists() {
        let root = unique_temp_dir("seagrass-artifacts-quiet");
        let source_dir = root.join("programs/demo/src");
        fs::create_dir_all(&source_dir).unwrap();
        let source = source("Demo111111111111111111111111111111111");
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();
        let anchor_toml = anchor_toml("Demo111111111111111111111111111111111");
        fs::write(root.join("Anchor.toml"), &anchor_toml).unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn reports_missing_deploy_after_build_surface_exists() {
        let root = unique_temp_dir("seagrass-artifacts-missing");
        let source_dir = root.join("programs/demo/src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(artifact_paths::idl_dir(&root)).unwrap();
        let source = source("Demo111111111111111111111111111111111");
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();
        let anchor_toml = anchor_toml("Demo111111111111111111111111111111111");
        fs::write(root.join("Anchor.toml"), &anchor_toml).unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(has_code(&diagnostics, "anchor-sbf-artifact"));
    }

    #[test]
    fn reports_invalid_deploy_and_idl_address_mismatch() {
        let root = unique_temp_dir("seagrass-artifacts-invalid");
        let source_dir = root.join("programs/demo/src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(artifact_paths::deploy_dir(&root)).unwrap();
        fs::create_dir_all(artifact_paths::idl_dir(&root)).unwrap();
        let source = source("Demo111111111111111111111111111111111");
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();
        fs::write(artifact_paths::deploy_file(&root, "demo"), "not elf").unwrap();
        fs::write(
            artifact_paths::idl_file(&root, "demo"),
            r#"{"address":"Other11111111111111111111111111111111","metadata":{"name":"demo"}}"#,
        )
        .unwrap();
        let anchor_toml = anchor_toml("Demo111111111111111111111111111111111");
        fs::write(root.join("Anchor.toml"), &anchor_toml).unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(has_code(&diagnostics, "anchor-sbf-artifact"));
        assert!(has_code(&diagnostics, "anchor-idl-artifact"));
    }

    #[test]
    fn reports_stale_deploy() {
        let root = unique_temp_dir("seagrass-artifacts-stale");
        let source_dir = root.join("programs/demo/src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(artifact_paths::deploy_dir(&root)).unwrap();
        fs::write(
            artifact_paths::deploy_file(&root, "demo"),
            minimal_sbf_elf(),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(5));
        let source = source("Demo111111111111111111111111111111111");
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();
        let anchor_toml = anchor_toml("Demo111111111111111111111111111111111");
        fs::write(root.join("Anchor.toml"), &anchor_toml).unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("older")
                && matches!(
                    diagnostic.code.as_ref(),
                    Some(NumberOrString::String(code)) if code == "anchor-sbf-artifact"
                )
        }));
    }

    #[test]
    fn reports_pinocchio_invalid_deploy_without_anchor_idl_or_types() {
        let root = unique_temp_dir("seagrass-pinocchio-diagnostics");
        let program_root = root.join("programs/pinocchio-counter");
        let source_dir = program_root.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(artifact_paths::deploy_dir(&root)).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            r#"
[workspace]
members = ["programs/pinocchio-counter"]
"#,
        )
        .unwrap();
        fs::write(
            program_root.join("Cargo.toml"),
            r#"
[package]
name = "pinocchio-counter"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
pinocchio = { version = "0.11", default-features = false }
pinocchio-pubkey = "0.3"
"#,
        )
        .unwrap();
        let source = pinocchio_source();
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();
        fs::write(
            artifact_paths::deploy_file(&root, "pinocchio_counter"),
            "not elf",
        )
        .unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(has_code(&diagnostics, "anchor-sbf-artifact"));
        assert!(!has_code(&diagnostics, "anchor-idl-artifact"));
        assert!(!has_code(&diagnostics, "anchor-types-artifact"));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("anchor-sbf-artifact".to_string()))
                && diagnostic.data.as_ref().is_some_and(|data| {
                    data["programKind"] == "pinocchio"
                        && data["reason"] == "invalid"
                        && data["buildCommand"] == "cargo build-sbf"
                })
        }));
    }

    #[test]
    fn marks_native_deploy_stale_when_source_is_newer() {
        let root = unique_temp_dir("seagrass-native-diagnostics");
        let source_dir = root.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(artifact_paths::deploy_dir(&root)).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "native-counter-package"
version = "0.1.0"
edition = "2021"

[lib]
name = "native_counter_program"
crate-type = ["cdylib", "lib"]

[dependencies]
solana-program = "3"
"#,
        )
        .unwrap();
        fs::write(
            artifact_paths::deploy_file(&root, "native_counter_program"),
            minimal_sbf_elf(),
        )
        .unwrap();
        thread::sleep(Duration::from_millis(5));
        let source = native_source();
        let lib = source_dir.join("lib.rs");
        fs::write(&lib, source).unwrap();

        let diagnostics = collect(
            &ParsedDocument::parse(source).unwrap(),
            &Url::from_file_path(lib).unwrap(),
            None,
        );

        assert!(!has_code(&diagnostics, "anchor-idl-artifact"));
        assert!(!has_code(&diagnostics, "anchor-types-artifact"));
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.code == Some(NumberOrString::String("anchor-sbf-artifact".to_string()))
                && diagnostic.data.as_ref().is_some_and(|data| {
                    data["programKind"] == "native"
                        && data["reason"] == "stale"
                        && data["buildCommand"] == "cargo build-sbf"
                        && data["newerInput"]
                            .as_str()
                            .is_some_and(|path| path.ends_with("src/lib.rs"))
                })
        }));
    }

    fn has_code(diagnostics: &[Diagnostic], expected: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == expected
            )
        })
    }

    fn source(program_id: &str) -> &'static str {
        let _ = program_id;
        r#"
use anchor_lang::prelude::*;

declare_id!("Demo111111111111111111111111111111111");

#[program]
pub mod demo {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
"#
    }

    fn anchor_toml(program_id: &str) -> String {
        format!(
            r#"
[programs.localnet]
demo = "{program_id}"
"#
        )
    }

    fn minimal_sbf_elf() -> Vec<u8> {
        let mut header = vec![0_u8; 64];
        header[0..4].copy_from_slice(b"\x7fELF");
        header[4] = 2;
        header[5] = 1;
        header[6] = 1;
        header[16] = 2;
        header[18] = 247;
        header[60] = 3;
        header
    }

    fn pinocchio_source() -> &'static str {
        r#"
#![no_std]

use pinocchio::{entrypoint, AccountView, Address, ProgramResult};
use pinocchio_pubkey::declare_id;

declare_id!("Pino111111111111111111111111111111111111");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    _instruction_data: &[u8],
) -> ProgramResult {
    Ok(())
}
"#
    }

    fn native_source() -> &'static str {
        r#"
use solana_program::{
    account_info::AccountInfo,
    declare_id,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

declare_id!("Native1111111111111111111111111111111111");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    Ok(())
}
"#
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
