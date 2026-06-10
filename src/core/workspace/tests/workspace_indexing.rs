use super::*;
use std::fs;

#[test]
fn indexes_open_document_symbols() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(uri.clone(), "#[account]\npub struct State {}".to_string())],
    );

    assert_eq!(index.indexed_file_count(), 1);
    assert_eq!(index.symbol_locations("State")[0].uri, uri);
}

#[test]
fn indexes_unopened_root_workspace_files_into_lookup_maps() {
    let root = unique_temp_dir("seagrass-root-index");
    let program_src = root.join("programs").join("demo").join("src");
    fs::create_dir_all(&program_src).unwrap();
    fs::write(
        program_src.join("lib.rs"),
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, State>,
}

#[account]
pub struct State {}
"#,
    )
    .unwrap();

    let root_uri = Url::from_directory_path(&root).unwrap();
    let index = WorkspaceIndex::build(&[root_uri], []);

    assert!(!index.symbol_locations("initialize").is_empty());
    assert!(!index.symbol_locations("Create").is_empty());
    assert!(index.has_program_instruction_for_context("Create"));
    assert_eq!(index.indexed_file_count(), 1);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn indexes_workspace_even_when_root_has_target_ancestor() {
    let outer = unique_temp_dir("seagrass-root-index-outer");
    let root = outer.join("target").join("seagrass-root-index");
    let program_src = root.join("programs").join("demo").join("src");
    fs::create_dir_all(&program_src).unwrap();
    fs::write(
        program_src.join("lib.rs"),
        r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, State>,
}

#[account]
pub struct State {}
"#,
    )
    .unwrap();

    let root_uri = Url::from_directory_path(&root).unwrap();
    let index = WorkspaceIndex::build(&[root_uri], []);

    assert_eq!(index.indexed_file_count(), 1);
    assert!(!index.symbol_locations("Create").is_empty());

    let _ = fs::remove_dir_all(outer);
}

#[test]
fn workspace_symbols_include_no_build_idl_bridge_definitions() {
    let root = unique_temp_dir("seagrass-idl-bridge-index");
    fs::create_dir_all(root.join("target").join("idl")).unwrap();
    fs::write(
            root.join("target").join("idl").join("escrow.json"),
            r#"{"instructions":[{"name":"makeOffer","args":[{"name":"id","type":"u64"}]}],"accounts":[{"name":"Offer","type":{"kind":"struct","fields":[{"name":"maker","type":"pubkey"}]}}]}"#,
        )
        .unwrap();

    let root_uri = Url::from_directory_path(&root).unwrap();
    let index = WorkspaceIndex::build(std::slice::from_ref(&root_uri), []);

    assert!(index.workspace_symbols("make").iter().any(|symbol| {
        symbol.name == "makeOffer"
            && symbol.kind == SymbolKind::FUNCTION
            && symbol
                .location
                .uri
                .as_str()
                .ends_with("/target/idl/escrow.json")
    }));
    assert!(!index
        .symbol_locations_with_kinds("Offer", &[SymbolKind::STRUCT])
        .is_empty());
    assert!(index
        .symbol_locations_in_container("maker", &[SymbolKind::FIELD], "Offer")
        .iter()
        .any(|location| location.uri.as_str().ends_with("/target/idl/escrow.json")));
    let maker = index
        .field_info_in_container("maker", "Offer")
        .expect("IDL account field info");
    assert_eq!(maker.type_display.as_deref(), Some("pubkey"));

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_file_filter_prefers_anchor_program_sources() {
    let root = Path::new("/workspace/examples");
    assert!(is_anchor_source_file(
        root,
        Path::new("/workspace/examples/tutorial/basic-1/programs/basic-1/src/lib.rs")
    ));
    assert!(!is_anchor_source_file(
        root,
        Path::new("/workspace/examples/tutorial/basic-1/tests/basic.rs")
    ));
    assert!(!is_anchor_source_file(
        root,
        Path::new("/workspace/examples/client/src/generated.rs")
    ));

    assert!(is_anchor_source_file(
        Path::new("/workspace/programs/basic-1"),
        Path::new("/workspace/programs/basic-1/src/lib.rs")
    ));
    assert!(is_anchor_source_file(
        Path::new("/workspace/programs/basic-1/src"),
        Path::new("/workspace/programs/basic-1/src/instructions/create.rs")
    ));
}

#[test]
fn upsert_open_document_replaces_existing_indexed_file() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let mut index = WorkspaceIndex::build(
        &[],
        [(
            uri.clone(),
            "#[account]\npub struct OldState {}".to_string(),
        )],
    );

    let document = ParsedDocument::parse_or_empty("#[account]\npub struct NewState {}");
    index.upsert_parsed_open_document(uri.clone(), &document);

    assert!(index.symbol_locations("OldState").is_empty());
    assert_eq!(index.symbol_locations("NewState")[0].uri, uri);
    assert_eq!(index.indexed_file_count(), 1);
}

#[test]
fn workspace_file_update_indexes_only_changed_rust_file() {
    let root = unique_temp_dir("seagrass-incremental-index");
    let program_src = root.join("programs").join("demo").join("src");
    fs::create_dir_all(&program_src).unwrap();
    let source_path = program_src.join("lib.rs");
    fs::write(&source_path, "#[account]\npub struct IncrementalState {}").unwrap();
    let root_uri = Url::from_directory_path(&root).unwrap();
    let uri = Url::from_file_path(&source_path).unwrap();

    let mut index = WorkspaceIndex::default();
    let update =
        WorkspaceIndex::update_for_workspace_file(std::slice::from_ref(&root_uri), uri.clone())
            .expect("workspace Rust file should produce an index update");
    index.upsert_open_document_update(update);

    assert_eq!(index.symbol_locations("IncrementalState")[0].uri, uri);
    assert_eq!(index.indexed_file_count(), 1);

    index.remove_document(&uri);

    assert!(index.symbol_locations("IncrementalState").is_empty());
    assert_eq!(index.indexed_file_count(), 0);

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_file_update_ignores_non_anchor_rust_file() {
    let root = unique_temp_dir("seagrass-incremental-ignore");
    let tests_dir = root.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    let source_path = tests_dir.join("helper.rs");
    fs::write(&source_path, "#[account]\npub struct IgnoredState {}").unwrap();
    let root_uri = Url::from_directory_path(&root).unwrap();
    let uri = Url::from_file_path(&source_path).unwrap();

    assert!(WorkspaceIndex::update_for_workspace_file(&[root_uri], uri).is_none());

    let _ = fs::remove_dir_all(root);
}

#[test]
fn workspace_document_update_does_not_mutate_index_until_applied() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let mut index = WorkspaceIndex::build(
        &[],
        [(
            uri.clone(),
            "#[account]\npub struct OldState {}".to_string(),
        )],
    );
    let document = ParsedDocument::parse_or_empty("#[account]\npub struct NewState {}");
    let update = WorkspaceDocumentUpdate::from_parsed_open_document(uri.clone(), &document);

    assert_eq!(index.symbol_locations("OldState")[0].uri, uri);
    assert!(index.symbol_locations("NewState").is_empty());

    index.upsert_open_document_update(update);

    assert!(index.symbol_locations("OldState").is_empty());
    assert_eq!(index.symbol_locations("NewState")[0].uri, uri);
    assert_eq!(index.indexed_file_count(), 1);
}

#[test]
fn upsert_open_document_clears_stale_secondary_index_entries() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let mut index = WorkspaceIndex::build(
        &[],
        [(
            uri.clone(),
            r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub account: Account<'info, OldState>,
}

#[account]
pub struct OldState {}
"#
            .to_string(),
        )],
    );
    let document = ParsedDocument::parse_or_empty(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub account: Account<'info, NewState>,
}

#[account]
pub struct NewState {}
"#,
    );

    index.upsert_parsed_open_document(uri.clone(), &document);

    assert!(index
        .symbol_locations_with_kinds("OldState", &[SymbolKind::STRUCT])
        .is_empty());
    assert!(index
        .references_with_kinds("OldState", &[SymbolKind::STRUCT])
        .is_empty());
    assert!(!index
        .workspace_symbols("OldState")
        .iter()
        .any(|symbol| symbol.name == "OldState"));
    assert_eq!(
        index
            .symbol_locations_with_kinds("NewState", &[SymbolKind::STRUCT])
            .len(),
        1
    );
    assert_eq!(
        index
            .references_with_kinds("NewState", &[SymbolKind::STRUCT])
            .len(),
        2
    );
    assert_eq!(index.indexed_file_count(), 1);
}

#[test]
fn workspace_symbols_include_anchor_fields_and_instruction_args() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
"#
            .to_string(),
        )],
    );

    let symbols = index.workspace_symbols("");
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "payer" && symbol.kind == SymbolKind::FIELD));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "amount" && symbol.kind == SymbolKind::VARIABLE));
}

#[test]
fn workspace_symbols_include_anchor_helper_functions() {
    let uri = Url::parse("file:///tmp/instructions/make_offer.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub offer: Account<'info, Offer>,
}

pub fn save_offer(context: Context<MakeOffer>, amount: u64) -> Result<()> {
    context.accounts.offer.token_b_wanted_amount = amount;
    Ok(())
}
"#
            .to_string(),
        )],
    );

    let symbols = index.workspace_symbols("save");
    assert!(symbols.iter().any(|symbol| {
        symbol.name == "save_offer"
            && symbol.kind == SymbolKind::FUNCTION
            && symbol.container_name.as_deref() == Some("Context<MakeOffer>")
    }));
    assert!(index
        .workspace_symbols("amount")
        .iter()
        .any(|symbol| symbol.name == "amount" && symbol.kind == SymbolKind::VARIABLE));
}

#[test]
fn function_references_include_split_module_helper_calls() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/make_offer.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri.clone(),
                r#"
#[program]
pub mod escrow {
    pub fn make_offer(context: Context<MakeOffer>, amount: u64) -> Result<()> {
        instructions::make_offer::save_offer(context, amount)?;
        Ok(())
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri.clone(),
                r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
}

pub fn save_offer(context: Context<MakeOffer>, amount: u64) -> Result<()> {
    let _ = context.accounts.maker.key();
    let _ = amount;
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let references = index.function_references("save_offer");
    assert!(references.iter().any(|location| location.uri == helper_uri));
    assert!(references.iter().any(|location| location.uri == lib_uri));
}

/// The module-path trie maps `crate::module::Symbol` paths across file
/// boundaries.  Given a multi-file workspace layout:
///
/// ```text
/// src/
///   lib.rs        — declares #[program] mod, imports via `use state::*`
///   state.rs      — declares `#[account] pub struct Escrow`
///   instructions/
///     mod.rs
///     make.rs     — declares `#[derive(Accounts)] pub struct Make`
/// ```
///
/// The trie should resolve:
///  - `["crate", "state", "Escrow"]`     → true  (Escrow in src/state.rs)
///  - `["crate", "instructions", "make", "Make"]` → true  (Make in src/instructions/make.rs)
///  - `["crate", "state", "Missing"]`    → false (no such symbol)
#[test]
fn trie_resolves_multi_segment_module_path_across_files() {
    let root = unique_temp_dir("seagrass-module-path-trie");
    let src = root.join("programs").join("demo").join("src");
    let instructions_dir = src.join("instructions");
    fs::create_dir_all(&instructions_dir).unwrap();

    fs::write(
        src.join("lib.rs"),
        "#[program]\npub mod demo {}\nuse state::*;",
    )
    .unwrap();
    fs::write(
        src.join("state.rs"),
        "#[account]\npub struct Escrow { pub amount: u64 }",
    )
    .unwrap();
    fs::write(instructions_dir.join("mod.rs"), "pub mod make;").unwrap();
    fs::write(
        instructions_dir.join("make.rs"),
        "#[derive(Accounts)]\npub struct Make<'info> {}",
    )
    .unwrap();

    let root_uri = Url::from_directory_path(&root).unwrap();
    let index = WorkspaceIndex::build(&[root_uri], []);

    // Symbol in src/state.rs — reachable as crate::state::Escrow
    assert!(index.symbol_exists_at_qualified_path(&[
        "crate".to_string(),
        "state".to_string(),
        "Escrow".to_string(),
    ]));

    // Symbol in src/instructions/make.rs — reachable as crate::instructions::make::Make
    assert!(index.symbol_exists_at_qualified_path(&[
        "crate".to_string(),
        "instructions".to_string(),
        "make".to_string(),
        "Make".to_string(),
    ]));

    // Symbol that does not exist under that path
    assert!(!index.symbol_exists_at_qualified_path(&[
        "crate".to_string(),
        "state".to_string(),
        "Missing".to_string(),
    ]));

    let _ = fs::remove_dir_all(root);
}
