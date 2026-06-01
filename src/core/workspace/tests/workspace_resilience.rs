use super::*;

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
