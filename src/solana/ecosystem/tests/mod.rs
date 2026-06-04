use {
    super::*,
    crate::{project, solana_project::SolanaProjectKind},
    std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    },
    tower_lsp::lsp_types::Range,
};

#[test]
fn detects_codama_configured_idl_source() {
    let root = unique_temp_dir("seagrass-codama-ecosystem");
    fs::create_dir_all(root.join("idls")).unwrap();
    fs::write(
        root.join("codama.json"),
        r#"{ "idl": "idls/demo.codama.json" }"#,
    )
    .unwrap();
    fs::write(
            root.join("idls/demo.codama.json"),
            r#"{"kind":"rootNode","standard":"codama","program":{"kind":"programNode","name":"demo","publicKey":"Demo111111111111111111111111111111111","instructions":[]}}"#,
        )
        .unwrap();

    let report = report_for_program(&program(&root, "demo"));

    let codama = report
        .idl_sources
        .iter()
        .find(|source| source.kind == IdlSourceKind::Codama)
        .expect("Codama source");
    assert_eq!(codama.status, EcosystemArtifactStatus::Present);
    assert_eq!(codama.program_name.as_deref(), Some("demo"));
    assert_eq!(
        codama.address.as_deref(),
        Some("Demo111111111111111111111111111111111")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reports_missing_codama_idl_from_config() {
    let root = unique_temp_dir("seagrass-codama-missing");
    fs::write(
        root.join("codama.json"),
        r#"{ "idl": "idls/missing.json" }"#,
    )
    .unwrap();

    let report = report_for_program(&program(&root, "demo"));

    assert!(report.idl_sources.iter().any(|source| {
        source.kind == IdlSourceKind::Codama
            && source.status == EcosystemArtifactStatus::Missing
            && source
                .path
                .as_ref()
                .is_some_and(|path| path.ends_with("idls/missing.json"))
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn does_not_report_missing_codama_idl_from_package_marker_only() {
    let root = unique_temp_dir("seagrass-codama-marker-only");
    fs::write(
        root.join("package.json"),
        r#"{"devDependencies":{"codama":"latest"}}"#,
    )
    .unwrap();

    let report = report_for_program(&program(&root, "demo"));

    assert!(!report
        .idl_sources
        .iter()
        .any(|source| source.kind == IdlSourceKind::Codama
            && source.status == EcosystemArtifactStatus::Missing));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn detects_shank_from_dependency_and_idl() {
    let root = unique_temp_dir("seagrass-shank-ecosystem");
    fs::create_dir_all(root.join("target/idl")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "demo"

[lib]
crate-type = ["cdylib"]

[dependencies]
shank = "0.4"
"#,
    )
    .unwrap();
    fs::write(
            root.join("target/idl/demo.json"),
            r#"{"version":"0.1.0","name":"demo","metadata":{"address":"Demo111111111111111111111111111111111"},"instructions":[],"accounts":[],"types":[]}"#,
        )
        .unwrap();

    let report = report_for_program(&program(&root, "demo"));

    assert!(report.idl_sources.iter().any(|source| {
        source.kind == IdlSourceKind::Shank && source.status == EcosystemArtifactStatus::Present
    }));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn reports_program_metadata_package_without_payload() {
    let root = unique_temp_dir("seagrass-program-metadata-missing");
    fs::write(
        root.join("package.json"),
        r#"{"devDependencies":{"@solana-program/program-metadata":"latest"}}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("idls")).unwrap();
    fs::write(root.join("codama.json"), r#"{ "idl": "idls/demo.json" }"#).unwrap();
    fs::write(
            root.join("idls/demo.json"),
            r#"{"kind":"rootNode","standard":"codama","program":{"kind":"programNode","name":"demo","publicKey":"Demo111111111111111111111111111111111","instructions":[]}}"#,
        )
        .unwrap();

    let report = report_for_program(&program(&root, "demo"));

    assert_eq!(
        report.program_metadata.status,
        ProgramMetadataStatus::ConfiguredMissingPayload
    );
    assert!(report
        .program_metadata
        .suggested_idl_command
        .as_deref()
        .is_some_and(|command| command.contains("program-metadata@latest write idl")));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn detects_litesvm_mollusk_and_surfpool() {
    let root = unique_temp_dir("seagrass-test-surfpool");
    fs::create_dir_all(root.join("tests")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "demo"

[lib]
crate-type = ["cdylib"]

[dev-dependencies]
litesvm = "0.6"
mollusk-svm = "0.4"
"#,
    )
    .unwrap();
    fs::write(
        root.join("tests/litesvm.rs"),
        "use litesvm::LiteSVM; #[test] fn it_works() { let _svm = LiteSVM::new(); }",
    )
    .unwrap();
    fs::write(root.join("surfpool.toml"), "[surfpool]\n").unwrap();

    let report = report_for_program(&program(&root, "demo"));

    assert!(report.test_harnesses.iter().any(|harness| {
        harness.kind == TestHarnessKind::LiteSvm && harness.status == TestHarnessStatus::Configured
    }));
    assert!(report.test_harnesses.iter().any(|harness| {
        harness.kind == TestHarnessKind::Mollusk
            && harness.status == TestHarnessStatus::MissingTests
    }));
    assert_eq!(
        report.surfpool.status,
        SurfpoolStatus::ConfiguredMissingDeployArtifact
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn accepts_well_formed_program_ids() {
    assert!(is_valid_program_id(
        "Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS"
    ));
    assert!(is_valid_program_id(
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
    ));
}

#[test]
fn rejects_shell_injection_in_program_id() {
    // Untrusted program ids must never reach a suggested command string.
    assert!(!is_valid_program_id("$(rm -rf ~)"));
    assert!(!is_valid_program_id("Demo111; rm -rf /"));
    assert!(!is_valid_program_id("")); // empty
    assert!(!is_valid_program_id("0OIl00000000000000000000000000000000")); // non-base58 chars
    assert!(!is_valid_program_id("short")); // too short
}

#[test]
fn suppresses_suggested_program_metadata_command_for_invalid_program_id() {
    let root = unique_temp_dir("seagrass-program-metadata-invalid-id");
    fs::write(
        root.join("package.json"),
        r#"{"devDependencies":{"@solana-program/program-metadata":"latest"}}"#,
    )
    .unwrap();
    fs::create_dir_all(root.join("idls")).unwrap();
    fs::write(root.join("codama.json"), r#"{ "idl": "idls/demo.json" }"#).unwrap();
    fs::write(
            root.join("idls/demo.json"),
            r#"{"kind":"rootNode","standard":"codama","program":{"kind":"programNode","name":"demo","publicKey":"Demo111111111111111111111111111111111","instructions":[]}}"#,
        )
        .unwrap();
    let mut program = program(&root, "demo");
    program.id = Some("$(touch /tmp/seagrass-owned)".to_string());

    let report = report_for_program(&program);

    assert_eq!(
        report.program_metadata.status,
        ProgramMetadataStatus::ConfiguredMissingPayload
    );
    assert!(report.program_metadata.suggested_idl_command.is_none());
    let _ = fs::remove_dir_all(root);
}

fn program(root: &Path, name: &str) -> SolanaProgram {
    SolanaProgram {
        kind: SolanaProjectKind::NativeSolana,
        root: root.to_path_buf(),
        source_root: Some(root.join("src")),
        name: project::normalize_program_name(name),
        id: Some("Demo111111111111111111111111111111111".to_string()),
        cluster: None,
        metadata_uri: Url::from_file_path(root.join("Cargo.toml")).unwrap(),
        metadata_range: Range::default(),
    }
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
