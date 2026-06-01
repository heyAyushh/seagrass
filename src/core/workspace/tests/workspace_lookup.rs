use super::*;

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
