use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        ecosystem::{
            self, EcosystemArtifactStatus, EcosystemReport, IdlSourceKind, IdlSourceReport,
            ProgramMetadataStatus, SurfpoolStatus, TestHarnessStatus,
        },
        solana_project::SolanaProgram,
    },
    std::path::Path,
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Range, Url},
};

pub fn collect(
    document: &ParsedDocument,
    uri: &Url,
    program: Option<&SolanaProgram>,
) -> Vec<Diagnostic> {
    let Some(report) = program
        .map(ecosystem::report_for_program)
        .or_else(|| ecosystem::report_for_document(uri, document))
    else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    diagnostics.extend(idl_diagnostics(document, &report));
    diagnostics.extend(program_metadata_diagnostics(document, &report));
    diagnostics.extend(test_harness_diagnostics(document, &report));
    diagnostics.extend(surfpool_diagnostics(document, &report));
    diagnostics
}

fn idl_diagnostics(document: &ParsedDocument, report: &EcosystemReport) -> Vec<Diagnostic> {
    if !document_anchors_program(document) {
        return Vec::new();
    }

    report
        .idl_sources
        .iter()
        .filter(|source| source.kind != IdlSourceKind::Anchor)
        .filter_map(|source| idl_diagnostic(document, report, source))
        .collect()
}

fn idl_diagnostic(
    document: &ParsedDocument,
    report: &EcosystemReport,
    source: &IdlSourceReport,
) -> Option<Diagnostic> {
    match source.status {
        EcosystemArtifactStatus::Missing | EcosystemArtifactStatus::Invalid => {
            Some(ecosystem_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::SolanaIdlArtifact,
                format!(
                    "{} for `{}` is {}{}.",
                    source.kind.label(),
                    report.program.name,
                    source.status.as_str(),
                    source
                        .reason
                        .as_deref()
                        .map(|reason| format!(": {reason}"))
                        .unwrap_or_default()
                ),
                serde_json::json!({
                    "program": report.program.name,
                    "programId": report.program.id,
                    "programKind": report.program.kind.as_str(),
                    "idlFormat": source.kind.as_str(),
                    "idlOrigin": source.config_path.as_ref().or(source.path.as_ref()).map(|path| path_to_string(path)),
                    "artifact": source.path.as_ref().map(|path| path_to_string(path)),
                    "artifactUri": source.path.as_ref().and_then(|path| file_uri_string(path)),
                    "reason": source.status.as_str(),
                    "detail": source.reason,
                }),
                source.path.as_deref().or(source.config_path.as_deref()),
            ))
        }
        EcosystemArtifactStatus::Present => {
            let name_mismatch = source.program_name.as_ref().is_some_and(|name| {
                crate::project::normalize_program_name(name) != report.program.name
            });
            let id_mismatch = source
                .address
                .as_deref()
                .zip(report.program.id.as_deref())
                .is_some_and(|(idl_id, program_id)| idl_id != program_id);
            if !name_mismatch && !id_mismatch {
                return None;
            }
            Some(ecosystem_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::SolanaIdlArtifact,
                format!(
                    "{} for `{}` does not match the local program declaration.",
                    source.kind.label(),
                    report.program.name
                ),
                serde_json::json!({
                    "program": report.program.name,
                    "programId": report.program.id,
                    "programKind": report.program.kind.as_str(),
                    "idlFormat": source.kind.as_str(),
                    "idlProgramName": source.program_name,
                    "idlAddress": source.address,
                    "artifact": source.path.as_ref().map(|path| path_to_string(path)),
                    "artifactUri": source.path.as_ref().and_then(|path| file_uri_string(path)),
                    "reason": if id_mismatch { "program-id-mismatch" } else { "program-name-mismatch" },
                }),
                source.path.as_deref(),
            ))
        }
    }
}

fn program_metadata_diagnostics(
    document: &ParsedDocument,
    report: &EcosystemReport,
) -> Vec<Diagnostic> {
    if !document_anchors_program(document) {
        return Vec::new();
    }

    if report.program_metadata.status != ProgramMetadataStatus::ConfiguredMissingPayload {
        return Vec::new();
    }
    vec![ecosystem_diagnostic(
        document,
        report,
        AnchorDiagnosticKind::SolanaProgramMetadata,
        format!(
            "`{}` references Program Metadata tooling but no local security.txt or program-metadata payload was found.",
            report.program.name
        ),
        serde_json::json!({
            "program": report.program.name,
            "programId": report.program.id,
            "programKind": report.program.kind.as_str(),
            "reason": report.program_metadata.status.as_str(),
            "metadataSeed": "security",
            "canonical": report.program.id.is_some(),
            "suggestedCommand": report.program_metadata.suggested_idl_command,
        }),
        None,
    )]
}

fn test_harness_diagnostics(
    document: &ParsedDocument,
    report: &EcosystemReport,
) -> Vec<Diagnostic> {
    if !document_anchors_program(document) {
        return Vec::new();
    }

    report
        .test_harnesses
        .iter()
        .filter(|harness| harness.status == TestHarnessStatus::MissingTests)
        .map(|harness| {
            ecosystem_diagnostic(
                document,
                report,
                AnchorDiagnosticKind::SolanaTestHarness,
                format!(
                    "`{}` declares `{}` test tooling but no tests using it were found.",
                    report.program.name,
                    harness.kind.label()
                ),
                serde_json::json!({
                    "program": report.program.name,
                    "programId": report.program.id,
                    "programKind": report.program.kind.as_str(),
                    "harness": harness.kind.as_str(),
                    "reason": harness.status.as_str(),
                }),
                None,
            )
        })
        .collect()
}

fn surfpool_diagnostics(document: &ParsedDocument, report: &EcosystemReport) -> Vec<Diagnostic> {
    if !document_anchors_program(document) {
        return Vec::new();
    }

    if report.surfpool.status != SurfpoolStatus::ConfiguredMissingDeployArtifact {
        return Vec::new();
    }
    vec![ecosystem_diagnostic(
        document,
        report,
        AnchorDiagnosticKind::SolanaSurfpoolWorkspace,
        format!(
            "`{}` has Surfpool/localnet configuration but no SBPF artifact at `{}`.",
            report.program.name,
            path_to_string(&report.surfpool.deploy_path)
        ),
        serde_json::json!({
            "program": report.program.name,
            "programId": report.program.id,
            "programKind": report.program.kind.as_str(),
            "reason": report.surfpool.status.as_str(),
            "deployPath": path_to_string(&report.surfpool.deploy_path),
            "deployUri": file_uri_string(&report.surfpool.deploy_path),
            "surfpoolConfigs": report.surfpool.config_paths.iter().map(|path| path_to_string(path)).collect::<Vec<_>>(),
        }),
        Some(&report.surfpool.deploy_path),
    )]
}

fn ecosystem_diagnostic(
    document: &ParsedDocument,
    report: &EcosystemReport,
    kind: AnchorDiagnosticKind,
    message: String,
    data: serde_json::Value,
    artifact_path: Option<&Path>,
) -> Diagnostic {
    diagnostic_from_range_with_related(
        diagnostic_range(document),
        kind,
        message,
        Some(data),
        Some(related_information(report, artifact_path)),
    )
}

fn related_information(
    report: &EcosystemReport,
    artifact_path: Option<&Path>,
) -> Vec<DiagnosticRelatedInformation> {
    let mut related = vec![DiagnosticRelatedInformation {
        location: Location {
            uri: report.program.metadata_uri.clone(),
            range: report.program.metadata_range,
        },
        message: format!(
            "{} program `{}` is the local ecosystem root.",
            report.program.kind.label(),
            report.program.name
        ),
    }];
    if let Some(uri) = artifact_path.and_then(file_uri) {
        related.push(DiagnosticRelatedInformation {
            location: Location {
                uri,
                range: Range::default(),
            },
            message: "Referenced local Solana ecosystem artifact.".to_string(),
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

fn document_anchors_program(document: &ParsedDocument) -> bool {
    document.symbols().declared_program_id.is_some()
        || !document.symbols().instructions.is_empty()
}

fn file_uri(path: &Path) -> Option<Url> {
    Url::from_file_path(path).ok()
}

fn file_uri_string(path: &Path) -> Option<String> {
    file_uri(path).map(|uri| uri.to_string())
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::solana_project::SolanaProjectKind,
        std::{
            env, fs,
            path::PathBuf,
            time::{SystemTime, UNIX_EPOCH},
        },
        tower_lsp::lsp_types::{NumberOrString, Range},
    };

    #[test]
    fn warns_for_missing_codama_idl_from_config() {
        let root = unique_temp_dir("seagrass-codama-diagnostic");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("codama.json"),
            r#"{ "idl": "idls/missing.json" }"#,
        )
        .unwrap();
        let source = source();
        let document = ParsedDocument::parse(source).unwrap();
        let program = program(&root);

        let diagnostics = collect(
            &document,
            &Url::from_file_path(root.join("src/lib.rs")).unwrap(),
            Some(&program),
        );

        assert!(has_code(&diagnostics, "solana-idl-artifact"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stays_quiet_for_codama_package_marker_without_idl_config() {
        let root = unique_temp_dir("seagrass-codama-marker-diagnostic");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"codama":"latest"}}"#,
        )
        .unwrap();
        let source = source();
        let document = ParsedDocument::parse(source).unwrap();
        let program = program(&root);

        let diagnostics = collect(
            &document,
            &Url::from_file_path(root.join("src/lib.rs")).unwrap(),
            Some(&program),
        );

        assert!(!has_code(&diagnostics, "solana-idl-artifact"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn warns_for_program_metadata_package_without_payload() {
        let root = unique_temp_dir("seagrass-program-metadata-diagnostic");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@solana-program/program-metadata":"latest"}}"#,
        )
        .unwrap();
        let document = ParsedDocument::parse(source()).unwrap();
        let program = program(&root);

        let diagnostics = collect(
            &document,
            &Url::from_file_path(root.join("src/lib.rs")).unwrap(),
            Some(&program),
        );

        assert!(has_code(&diagnostics, "solana-program-metadata"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn warns_for_missing_litesvm_tests_and_surfpool_artifact() {
        let root = unique_temp_dir("seagrass-harness-surfpool-diagnostic");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            r#"
[package]
name = "demo"

[lib]
crate-type = ["cdylib"]

[dev-dependencies]
litesvm = "0.6"
"#,
        )
        .unwrap();
        fs::write(root.join("surfpool.toml"), "[surfpool]\n").unwrap();
        let document = ParsedDocument::parse(source()).unwrap();
        let program = program(&root);

        let diagnostics = collect(
            &document,
            &Url::from_file_path(root.join("src/lib.rs")).unwrap(),
            Some(&program),
        );

        assert!(has_code(&diagnostics, "solana-test-harness"));
        assert!(has_code(&diagnostics, "solana-surfpool-workspace"));
        let _ = fs::remove_dir_all(root);
    }

    fn has_code(diagnostics: &[Diagnostic], expected: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == expected
            )
        })
    }

    fn program(root: &Path) -> SolanaProgram {
        SolanaProgram {
            kind: SolanaProjectKind::NativeSolana,
            root: root.to_path_buf(),
            source_root: Some(root.join("src")),
            name: "demo".to_string(),
            id: Some("Demo111111111111111111111111111111111".to_string()),
            cluster: None,
            metadata_uri: Url::from_file_path(root.join("Cargo.toml")).unwrap(),
            metadata_range: Range::default(),
        }
    }

    fn source() -> &'static str {
        r#"declare_id!("Demo111111111111111111111111111111111");"#
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!("{name}-{nonce}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
