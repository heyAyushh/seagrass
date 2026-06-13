use {
    super::*,
    crate::document::ParsedDocument,
    std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    },
};

#[test]
fn detects_pinocchio_from_manifest_dependency() {
    let root = unique_temp_dir("seagrass-pinocchio-project");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "pinocchio-demo"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
pinocchio = "0.8"
"#,
    )
    .unwrap();
    let source = r#"
use pinocchio::{entrypoint, ProgramResult};
entrypoint!(process_instruction);
fn process_instruction() -> ProgramResult { Ok(()) }
"#;
    let lib = root.join("src/lib.rs");
    fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::Pinocchio);
    assert_eq!(program.name, "pinocchio_demo");
    assert_eq!(program.root, root);
}

#[test]
fn detects_pinocchio_from_split_account_view_manifest_dependency() {
    let root = unique_temp_dir("seagrass-pinocchio-view-project");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "pinocchio-view-demo"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
solana-account-view = "3"
solana-instruction-view = "3"
solana-program-error = "3"
"#,
    )
    .unwrap();
    let source = r#"
use {
    solana_account_view::AccountView,
    solana_address::Address,
    solana_program_error::ProgramResult,
};

fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let _ = (program_id, accounts, instruction_data);
    Ok(())
}
"#;
    let lib = root.join("src/lib.rs");
    fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::Pinocchio);
    assert_eq!(program.name, "pinocchio_view_demo");
}

#[test]
fn detects_native_solana_from_manifest_dependency() {
    let root = unique_temp_dir("seagrass-native-project");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "native-demo"

[lib]
name = "native_sbf"
crate-type = ["cdylib", "lib"]

[dependencies]
solana-program = "3"
"#,
    )
    .unwrap();
    let source = r#"
use solana_program::entrypoint;
entrypoint!(process_instruction);
fn process_instruction() {}
"#;
    let lib = root.join("src/lib.rs");
    fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::NativeSolana);
    assert_eq!(program.name, "native_sbf");
}

#[test]
fn parse_manifest_deps_records_keys_and_package_renames() {
    let deps = parse_manifest_deps(
        r#"
[dependencies]
clock = { package = "solana-clock", version = "2" }
solana-rent.workspace = true
"#,
    );

    assert!(deps.contains_crate("clock"));
    assert!(deps.contains_crate("solana-clock"));
    assert!(deps.contains_crate("solana-rent"));
    assert!(deps.import_root_matches_crate("clock", "solana-clock"));
    assert!(deps.import_root_matches_crate("solana_rent", "solana-rent"));
    assert!(!deps.import_root_matches_crate("clock", "solana-rent"));
}

#[test]
fn detects_anchor_from_toml_presence() {
    let root = unique_temp_dir("seagrass-anchor-toml-presence");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Anchor.toml"), "[workspace]\nmembers = []\n").unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[package]
name = "anchor-demo"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
anchor-lang = "0.31"
"#,
    )
    .unwrap();
    let source = r#"
use anchor_lang::prelude::*;
declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
#[program]
mod anchor_demo {}
"#;
    let lib = root.join("src/lib.rs");
    fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::Anchor);
    assert_eq!(program.name, "anchor_demo");
    assert_eq!(program.root, root);
}

#[test]
fn detects_anchor_program_from_anchor_toml_without_cargo_manifest() {
    let root = unique_temp_dir("seagrass-anchor-project");
    let source_dir = root.join("programs/artifact-demo/src");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        root.join("Anchor.toml"),
        r#"
[programs.localnet]
artifact_demo = "Artifact1111111111111111111111111111111"
"#,
    )
    .unwrap();
    let source = r#"
use anchor_lang::prelude::*;

declare_id!("Artifact1111111111111111111111111111111");
"#;
    let lib = source_dir.join("lib.rs");
    fs::write(&lib, source).unwrap();
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let program = detect_for_document(&uri, &document).unwrap();

    assert_eq!(program.kind, SolanaProjectKind::Anchor);
    assert_eq!(program.name, "artifact_demo");
    assert_eq!(program.root, root);
    assert_eq!(program.source_root, Some(source_dir));
}

#[test]
fn nearest_workspace_manifest_finds_workspace_root() {
    let root = unique_temp_dir("seagrass-workspace-manifest");
    let program_src = root.join("programs/demo/src");
    fs::create_dir_all(&program_src).unwrap();
    fs::write(
        root.join("Cargo.toml"),
        r#"
[workspace]
members = ["programs/demo"]
"#,
    )
    .unwrap();
    fs::write(
        root.join("programs/demo/Cargo.toml"),
        r#"
[package]
name = "demo"
"#,
    )
    .unwrap();
    let lib = program_src.join("lib.rs");
    fs::write(&lib, "pub fn entry() {}\n").unwrap();
    let uri = Url::from_file_path(lib).unwrap();

    let (manifest_uri, manifest_text) = nearest_workspace_manifest(&uri).unwrap();

    assert_eq!(
        manifest_uri,
        Url::from_file_path(root.join("Cargo.toml")).unwrap()
    );
    assert!(manifest_text.contains("[workspace]"));

    let _ = fs::remove_dir_all(root);
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
