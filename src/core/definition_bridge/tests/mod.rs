use {super::*, crate::solana::artifact_paths, std::fs};

#[test]
fn parses_relevant_dependency_names_without_toml_dependency() {
    let names = parse_dependency_names(
        r#"
[dependencies]
anchor-lang = "1"
serde = "1"
anchor-spl = { version = "1" }
spl-token = "4"

[dev-dependencies]
mpl-token-metadata = "5"
"#,
    );

    assert_eq!(
        names,
        vec![
            "anchor-lang".to_string(),
            "anchor-spl".to_string(),
            "mpl-token-metadata".to_string(),
            "spl-token".to_string()
        ]
    );
}

#[test]
fn parses_relevant_path_dependencies_without_building() {
    let paths = parse_relevant_path_dependencies(
        r#"
[dependencies]
anchor-lang = { path = "../../lang", features = ["init-if-needed"] }
serde = { path = "../serde" }
spl-token = { version = "4", path = "../spl-token" }
"#,
    );

    assert_eq!(
        paths,
        vec![PathBuf::from("../../lang"), PathBuf::from("../spl-token")]
    );
}

#[test]
fn collects_idl_definitions_from_existing_json() {
    let root = unique_temp_dir("seagrass-bridge-idl");
    fs::create_dir_all(artifact_paths::idl_dir(&root)).unwrap();
    fs::write(
        artifact_paths::idl_file(&root, "demo"),
        r#"{
  "instructions": [{
    "name": "makeOffer",
    "args": [{"name": "id", "type": "u64"}]
  }],
  "accounts": [{
    "name": "Offer",
    "type": {
      "kind": "struct",
      "fields": [{"name": "maker", "type": "pubkey"}]
    }
  }],
  "types": [{
    "name": "EscrowState",
    "type": {
      "kind": "struct",
      "fields": [{"name": "bump", "type": "u8"}]
    }
  }]
}"#,
    )
    .unwrap();

    let uri = Url::from_directory_path(&root).unwrap();
    let symbols = collect(&[uri]);

    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "makeOffer" && symbol.kind == SymbolKind::FUNCTION));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "Offer" && symbol.kind == SymbolKind::STRUCT));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "id"
            && symbol.kind == SymbolKind::VARIABLE
            && symbol.container_name.as_deref() == Some("IDL instruction makeOffer")
            && symbol.type_display.as_deref() == Some("u64")
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "maker"
            && symbol.kind == SymbolKind::FIELD
            && symbol.container_name.as_deref() == Some("Offer")
            && symbol.type_display.as_deref() == Some("pubkey")
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "bump"
            && symbol.kind == SymbolKind::FIELD
            && symbol.container_name.as_deref() == Some("EscrowState")
            && symbol.type_display.as_deref() == Some("u8")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collects_codama_program_node_definitions_from_existing_json() {
    let root = unique_temp_dir("seagrass-bridge-codama-idl");
    fs::create_dir_all(root.join("idls")).unwrap();
    fs::write(
        root.join("idls").join("demo.codama.json"),
        r#"{
  "kind": "rootNode",
  "standard": "codama",
  "program": {
    "kind": "programNode",
    "name": "demo",
    "publicKey": "Demo111111111111111111111111111111111",
    "instructions": [{
      "name": "makeOffer",
      "args": [{"name": "id", "type": "u64"}]
    }],
    "accounts": [{
      "name": "Offer",
      "fields": [{"name": "maker", "type": "pubkey"}]
    }],
    "definedTypes": [{
      "name": "EscrowState",
      "fields": [{"name": "bump", "type": "u8"}]
    }],
    "events": [],
    "errors": []
  }
}"#,
    )
    .unwrap();

    let uri = Url::from_directory_path(&root).unwrap();
    let symbols = collect(&[uri]);

    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "makeOffer" && symbol.kind == SymbolKind::FUNCTION));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "Offer" && symbol.kind == SymbolKind::STRUCT));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "bump"
            && symbol.kind == SymbolKind::FIELD
            && symbol.container_name.as_deref() == Some("EscrowState")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collects_generated_artifact_bridge_symbols_without_parsing_binaries() {
    let root = unique_temp_dir("seagrass-bridge-artifacts");
    fs::create_dir_all(artifact_paths::deploy_dir(&root)).unwrap();
    fs::create_dir_all(artifact_paths::types_dir(&root)).unwrap();
    fs::write(artifact_paths::deploy_file(&root, "demo"), [0_u8; 4]).unwrap();
    fs::write(artifact_paths::keypair_file(&root, "demo"), "[]").unwrap();
    fs::write(
        artifact_paths::typescript_file(&root, "demo"),
        "export type Demo = {};",
    )
    .unwrap();

    let uri = Url::from_directory_path(&root).unwrap();
    let symbols = collect(&[uri]);

    assert!(symbols.iter().any(|symbol| {
        symbol.name == "demo.so"
            && symbol.kind == SymbolKind::FILE
            && symbol.container_name.as_deref() == Some("Anchor SBPF artifacts")
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "demo-keypair.json"
            && symbol.kind == SymbolKind::CONSTANT
            && symbol.container_name.as_deref() == Some("Anchor program keypairs")
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "demo.ts"
            && symbol.kind == SymbolKind::FILE
            && symbol.container_name.as_deref() == Some("Anchor TypeScript artifacts")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collects_anchor_toml_program_definitions() {
    let root = unique_temp_dir("seagrass-bridge-anchor-toml");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("Anchor.toml"),
        r#"
[programs.localnet]
escrow = "Escrow111111111111111111111111111111111111"
"#,
    )
    .unwrap();

    let uri = Url::from_directory_path(&root).unwrap();
    let symbols = collect(&[uri]);

    assert!(symbols.iter().any(|symbol| {
        symbol.name == "escrow"
            && symbol.kind == SymbolKind::CONSTANT
            && symbol.type_display.as_deref() == Some("Escrow111111111111111111111111111111111111")
    }));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn collects_public_dependency_source_definitions_without_build() {
    let file = syn::parse_file(
        r#"
pub const DISCRIMINATOR_LENGTH: usize = 8;
pub struct InterfaceAccount<T> {
    pub inner: T,
    private: u8,
}
pub enum TokenState {
    Initialized,
}
pub trait Owners {}
pub type Result<T> = core::result::Result<T, ()>;
impl InterfaceAccount<u8> {
    pub const OWNER: Pubkey = Pubkey::new_from_array([0; 32]);
    pub fn key(&self) -> Pubkey { Pubkey::default() }
}
fn private_helper() {}
"#,
    )
    .unwrap();
    let uri = Url::parse("file:///tmp/anchor-spl/src/lib.rs").unwrap();

    let symbols = public_source_symbols(&file, &uri, "anchor-spl");

    assert!(symbols
        .iter()
        .any(|symbol| { symbol.name == "InterfaceAccount" && symbol.kind == SymbolKind::STRUCT }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "DISCRIMINATOR_LENGTH"
            && symbol.kind == SymbolKind::CONSTANT
            && symbol.type_display.as_deref() == Some("usize")
    }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "inner"
            && symbol.kind == SymbolKind::FIELD
            && symbol.container_name.as_deref() == Some("InterfaceAccount")
            && symbol.type_display.as_deref() == Some("T")
    }));
    assert!(!symbols.iter().any(|symbol| symbol.name == "private"));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "TokenState" && symbol.kind == SymbolKind::ENUM));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "Initialized"
            && symbol.kind == SymbolKind::ENUM_MEMBER
            && symbol.container_name.as_deref() == Some("TokenState")
    }));
    assert!(symbols
        .iter()
        .any(|symbol| { symbol.name == "Owners" && symbol.kind == SymbolKind::INTERFACE }));
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "OWNER"
            && symbol.kind == SymbolKind::CONSTANT
            && symbol.container_name.as_deref() == Some("InterfaceAccount")
    }));
    assert!(!symbols.iter().any(|symbol| symbol.name == "private_helper"));
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!("{name}-{nonce}"))
}
