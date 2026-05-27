use super::*;
use std::path::{Path, PathBuf};

mod references;

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

#[test]
fn reachable_cpi_program_usages_include_called_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/call_external.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn cpi(ctx: Context<Cpi>) -> Result<()> {
        instructions::call_external(ctx)
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let usages = index
        .reachable_cpi_program_usage_names_for_context("Cpi")
        .expect("reachable CPI usages");

    assert!(usages.contains("external_program"));
}

#[test]
fn reachable_cpi_program_usages_ignore_uncalled_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/call_external.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Cpi>) -> Result<()> {
        let _ = ctx.accounts.external_program.key();
        Ok(())
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let usages = index
        .reachable_cpi_program_usage_names_for_context("Cpi")
        .expect("reachable CPI usages");

    assert!(!usages.contains("external_program"));
}

#[test]
fn workspace_symbols_include_declared_program_id() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");"#.to_string(),
        )],
    );

    let symbols = index.workspace_symbols("declare");
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "declare_id!" && symbol.kind == SymbolKind::CONSTANT));
}

#[test]
fn workspace_symbols_include_incomplete_open_document_tree_sitter_symbols() {
    let uri = Url::parse("file:///tmp/incomplete.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri.clone(),
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub rent: Sysvar<'info, s
}
"#
            .to_string(),
        )],
    );

    assert_eq!(index.indexed_file_count(), 1);
    let create = index.symbol_locations_with_kinds("Create", &[SymbolKind::STRUCT]);
    assert_eq!(create.len(), 1);
    assert_eq!(create[0].uri, uri);

    let symbols = index.workspace_symbols("");
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "initialize" && symbol.kind == SymbolKind::FUNCTION));
    assert!(symbols
        .iter()
        .any(|symbol| symbol.name == "rent" && symbol.kind == SymbolKind::FIELD));
}

#[test]
fn references_include_incomplete_open_document_tree_sitter_type_edges() {
    let uri = Url::parse("file:///tmp/incomplete.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub account: Account<'info, State
}

#[account]
pub struct State {}
"#
            .to_string(),
        )],
    );

    let create_refs = index.references_with_kinds("Create", &[SymbolKind::STRUCT]);
    assert!(create_refs
        .iter()
        .any(|location| location.range.start.line == 3));
    assert!(create_refs
        .iter()
        .any(|location| location.range.start.line == 9));

    let state_refs = index.references_with_kinds("State", &[SymbolKind::STRUCT]);
    assert!(state_refs
        .iter()
        .any(|location| location.range.start.line == 10));
    assert!(state_refs
        .iter()
        .any(|location| location.range.start.line == 14));
}

#[test]
fn anchor_type_references_are_symbol_gated_for_incomplete_documents() {
    let uri = Url::parse("file:///tmp/incomplete.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub account: Account<'info, State
}

#[account]
pub struct State {}
"#
            .to_string(),
        )],
    );

    let state_refs = index.anchor_type_references("State");
    assert!(state_refs
        .iter()
        .any(|location| location.range.start.line == 3));
    assert!(state_refs
        .iter()
        .any(|location| location.range.start.line == 7));
    assert!(index.anchor_type_references("Unknown").is_empty());
}

#[test]
fn symbol_locations_can_be_filtered_by_anchor_kind() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub State: AccountInfo<'info>,
}

#[account]
pub struct State {}
"#
            .to_string(),
        )],
    );

    let locations = index.symbol_locations_with_kinds("State", &[SymbolKind::STRUCT]);
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].range.start.line, 7);
}

#[test]
fn symbol_locations_can_be_filtered_by_anchor_container() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[account]
pub struct Counter {
    pub count: u64,
}

#[account]
pub struct Other {
    pub count: u64,
}
"#
            .to_string(),
        )],
    );

    let locations = index.symbol_locations_in_container("count", &[SymbolKind::FIELD], "Counter");
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].range.start.line, 3);
}

#[test]
fn field_info_in_container_includes_anchor_field_type() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[account]
pub struct Counter {
    pub count: u64,
}
"#
            .to_string(),
        )],
    );

    let info = index
        .field_info_in_container("count", "Counter")
        .expect("Counter.count field info");
    assert_eq!(info.location.range.start.line, 3);
    assert_eq!(info.type_display.as_deref(), Some("u64"));
}

#[test]
fn resolves_nested_account_field_paths_from_workspace_accounts_structs() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#
            .to_string(),
        )],
    );

    let resolved = index
        .resolve_account_field_path("Read", &["wrapper".to_string(), "inner".to_string()], 1)
        .expect("nested account field");

    assert_eq!(resolved.container_name, "Wrapped");
    assert_eq!(resolved.field_name, "inner");
    assert_eq!(resolved.field_info.location.range.start.line, 8);
    assert_eq!(
        resolved.field_info.type_display.as_deref(),
        Some("Account<Inner>")
    );
}

#[test]
fn account_context_fields_include_workspace_field_types() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
    pub offer: Account<'info, Offer>,
}
"#
            .to_string(),
        )],
    );

    let fields = index.account_context_fields("MakeOffer");
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "maker");
    assert_eq!(fields[0].type_display.as_deref(), Some("Signer"));
    assert_eq!(fields[1].name, "offer");
    assert_eq!(fields[1].type_display.as_deref(), Some("Account<Offer>"));
}

#[test]
fn indexes_instruction_arguments_by_accounts_context() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri,
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, amount: u64, decimals: u8) -> Result<()> {
        Ok(())
    }
}
"#
            .to_string(),
        )],
    );

    assert!(index.has_program_instruction_for_context("Create"));
    let args = index.instruction_argument_names_for_context("Create");
    assert!(args.contains("amount"));
    assert!(args.contains("decimals"));
    assert!(!args.contains("ctx"));
}

#[test]
fn implementation_locations_include_program_instructions_for_context() {
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            uri.clone(),
            r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }

    pub fn read(ctx: Context<Read>) -> Result<()> {
        Ok(())
    }
}
"#
            .to_string(),
        )],
    );

    let locations = index.implementation_locations_for_context("Create");
    assert_eq!(locations.len(), 1);
    assert_eq!(locations[0].uri, uri);
    assert_eq!(locations[0].range.start.line, 3);

    let instructions = index.program_instructions_for_context("Create");
    assert_eq!(instructions.len(), 1);
    assert_eq!(instructions[0].name, "initialize");
    assert_eq!(instructions[0].location.uri, uri);
    assert!(instructions[0].is_open);
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{nonce}"))
}
