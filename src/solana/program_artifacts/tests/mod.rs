use {
    super::*,
    std::{env, thread, time::Duration},
};

#[test]
fn parses_sbf_elf_header() {
    let header = minimal_sbf_elf();

    let elf = parse_elf_summary(&header).unwrap();

    assert_eq!(elf.class, ElfClass::Elf64);
    assert_eq!(elf.endian, ElfEndian::Little);
    assert_eq!(elf.machine, EM_BPF);
}

#[test]
fn rejects_non_bpf_elf_header() {
    let mut header = minimal_sbf_elf();
    header[18] = 62;
    header[19] = 0;

    let error = parse_elf_summary(&header).unwrap_err();

    assert!(error.contains("not eBPF/SBF"));
}

#[test]
fn reports_anchor_build_artifact_paths_from_anchor_toml() {
    let root = unique_temp_dir("seagrass-artifacts");
    let source_dir = root.join("programs/my-program/src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(root.join("target/idl")).unwrap();
    let source_text = r#"declare_id!("Demo111111111111111111111111111111111");"#;
    fs::write(source_dir.join("lib.rs"), source_text).unwrap();
    let anchor_toml = root.join("Anchor.toml");
    fs::write(
        &anchor_toml,
        r#"
[provider]
cluster = "localnet"

[programs.localnet]
my_program = "Demo111111111111111111111111111111111"
"#,
    )
    .unwrap();
    let uri = Url::from_file_path(source_dir.join("lib.rs")).unwrap();

    let report = report_for_document(&uri, &ParsedDocument::parse(source_text).unwrap()).unwrap();

    assert!(report.build_surface_exists());
    assert!(matches!(report.deploy, DeployArtifactState::Missing));
    assert!(report.deploy_path.ends_with("target/deploy/my_program.so"));
    assert!(report.idl_path.ends_with("target/idl/my_program.json"));
    assert_eq!(report.source_inputs.len(), 1);
}

#[test]
fn detects_stale_deploy_artifact_from_source_inputs() {
    let older = UNIX_EPOCH + Duration::from_secs(10);
    let newer = UNIX_EPOCH + Duration::from_secs(20);
    let report = ProgramArtifactReport {
        program: SolanaProgram {
            kind: SolanaProjectKind::Anchor,
            root: PathBuf::from("/workspace"),
            source_root: Some(PathBuf::from("/workspace/programs/demo/src")),
            name: "demo".to_string(),
            id: Some("Demo111111111111111111111111111111111".to_string()),
            cluster: Some("localnet".to_string()),
            metadata_uri: Url::parse("file:///workspace/Anchor.toml").unwrap(),
            metadata_range: tower_lsp::lsp_types::Range::default(),
        },
        root: PathBuf::from("/workspace"),
        source_root: Some(PathBuf::from("/workspace/programs/demo/src")),
        source_inputs: vec![ArtifactInput {
            path: PathBuf::from("/workspace/programs/demo/src/lib.rs"),
            modified: newer,
        }],
        deploy_path: PathBuf::from("/workspace/target/deploy/demo.so"),
        keypair_path: PathBuf::from("/workspace/target/deploy/demo-keypair.json"),
        idl_path: PathBuf::from("/workspace/target/idl/demo.json"),
        types_path: PathBuf::from("/workspace/target/types/demo.ts"),
        deploy: DeployArtifactState::Present {
            file: ArtifactFile {
                path: PathBuf::from("/workspace/target/deploy/demo.so"),
                byte_len: 64,
                modified: older,
            },
            elf: ElfSummary {
                class: ElfClass::Elf64,
                endian: ElfEndian::Little,
                machine: EM_BPF,
                entry: 0,
                section_count: 0,
            },
        },
        keypair: ProgramKeypairState::Missing,
        idl: IdlArtifactState::Missing,
        typescript: FilePresence::Missing,
    };

    assert_eq!(report.deploy_stale_inputs().len(), 1);
}

#[test]
fn validates_real_artifact_files_when_present() {
    let root = unique_temp_dir("seagrass-artifacts-present");
    let source_dir = root.join("programs/demo/src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(root.join("target/deploy")).unwrap();
    fs::create_dir_all(root.join("target/idl")).unwrap();
    fs::create_dir_all(root.join("target/types")).unwrap();
    let source_text = r#"declare_id!("Demo111111111111111111111111111111111");"#;
    fs::write(source_dir.join("lib.rs"), source_text).unwrap();
    fs::write(root.join("target/deploy/demo.so"), minimal_sbf_elf()).unwrap();
    fs::write(
        root.join("target/idl/demo.json"),
        r#"{"metadata":{"name":"demo"},"instructions":[]}"#,
    )
    .unwrap();
    fs::write(root.join("target/types/demo.ts"), "export type Demo = {};").unwrap();
    thread::sleep(Duration::from_millis(2));
    let anchor_toml = root.join("Anchor.toml");
    fs::write(
        &anchor_toml,
        r#"
[programs.localnet]
demo = "Demo111111111111111111111111111111111"
"#,
    )
    .unwrap();
    let uri = Url::from_file_path(source_dir.join("lib.rs")).unwrap();

    let report = report_for_document(&uri, &ParsedDocument::parse(source_text).unwrap()).unwrap();

    assert!(matches!(report.deploy, DeployArtifactState::Present { .. }));
    assert!(matches!(report.keypair, ProgramKeypairState::Missing));
    assert!(matches!(report.idl, IdlArtifactState::Present { .. }));
    assert!(matches!(report.typescript, FilePresence::Present(_)));
}

#[test]
fn reports_pinocchio_manifest_without_anchor_toml() {
    let root = unique_temp_dir("seagrass-pinocchio-artifacts");
    let program_root = root.join("programs/pinocchio-counter");
    let source_dir = program_root.join("src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(root.join("target/deploy")).unwrap();
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
        root.join("target/deploy/pinocchio_counter.so"),
        "not an elf",
    )
    .unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let report = report_for_document(&uri, &ParsedDocument::parse(source).unwrap()).unwrap();
    let json = report.to_json();

    assert_eq!(report.program.kind, SolanaProjectKind::Pinocchio);
    assert_eq!(report.program.name, "pinocchio_counter");
    assert_eq!(
        report.program.id.as_deref(),
        Some("Pino111111111111111111111111111111111111")
    );
    assert_eq!(report.root, root);
    assert!(matches!(report.deploy, DeployArtifactState::Invalid { .. }));
    assert!(matches!(report.keypair, ProgramKeypairState::Missing));
    assert_eq!(json["idl"]["status"], "not-applicable");
    assert_eq!(json["typescript"]["status"], "not-applicable");
    assert!(json["paths"]["idl"].is_null());
    assert!(json["paths"]["typescript"].is_null());
}

#[test]
fn uses_lib_name_override_for_native_sbf_artifact() {
    let root = unique_temp_dir("seagrass-native-artifacts");
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "native-counter-package"
version = "0.1.0"
edition = "2021"

[lib]
name = "native_counter_program"
crate-type = ["lib", "cdylib"]

[dependencies]
solana-program = "3"
"#,
    )
    .unwrap();
    let source = native_source();
    let lib = source_dir.join("lib.rs");
    fs::write(&lib, source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let report = report_for_document(&uri, &ParsedDocument::parse(source).unwrap()).unwrap();

    assert_eq!(report.program.kind, SolanaProjectKind::NativeSolana);
    assert_eq!(report.program.name, "native_counter_program");
    assert!(report
        .deploy_path
        .ends_with("target/deploy/native_counter_program.so"));
    assert!(report
        .keypair_path
        .ends_with("target/deploy/native_counter_program-keypair.json"));
}

#[test]
fn ignores_non_sbf_library_even_with_pinocchio_dependency() {
    let root = unique_temp_dir("seagrass-pinocchio-helper");
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "pinocchio-helper"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["lib"]

[dependencies]
pinocchio = "0.11"
"#,
    )
    .unwrap();
    let source = "pub fn helper() {}";
    let lib = source_dir.join("lib.rs");
    fs::write(&lib, source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    assert!(report_for_document(&uri, &ParsedDocument::parse(source).unwrap()).is_none());
}

#[test]
fn decodes_solana_keypair_public_key() {
    let mut bytes = (0_u8..64).collect::<Vec<_>>();
    bytes[32..].fill(0);
    bytes[63] = 1;
    let json = serde_json::to_string(&bytes).unwrap();

    let public_key = keypair_public_key(&json).unwrap();

    assert_eq!(public_key, "11111111111111111111111111111112");
}

fn minimal_sbf_elf() -> Vec<u8> {
    let mut header = vec![0_u8; 64];
    header[0..4].copy_from_slice(b"\x7fELF");
    header[4] = 2;
    header[5] = 1;
    header[6] = 1;
    header[16] = 2;
    header[18] = (EM_BPF & 0xff) as u8;
    header[19] = (EM_BPF >> 8) as u8;
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
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!("{name}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}
