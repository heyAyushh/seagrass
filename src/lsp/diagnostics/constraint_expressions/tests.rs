use {
    super::*,
    crate::workspace::WorkspaceIndex,
    tower_lsp::lsp_types::{NumberOrString, Url},
};

#[test]
fn reports_unresolved_bare_constraint_identifier_on_value_span() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, amount: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = amount > 0,
        constraint = sd,
    )]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`sd` does not resolve"))
        .expect("unresolved identifier diagnostic");

    assert!(matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-constraint-expression"
    ));
    assert_eq!(diagnostic.range.start.line, 10);
    assert_eq!(diagnostic.range.start.character, 21);
    let candidates = diagnostic
        .data
        .as_ref()
        .and_then(|data| data["candidates"].as_array());
    assert!(
        candidates.is_some_and(|candidates| candidates.iter().any(|value| value == "amount")),
        "expected in-scope instruction argument candidate; got {diagnostic:#?}"
    );
}

#[test]
fn glob_import_suppresses_unresolved_identifier_without_workspace() {
    let source = r#"
use state::*;

mod state;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = unknown_const)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    assert!(collect(&document).is_empty());
}

#[test]
fn no_glob_import_reports_unresolved_identifier() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = unknown_const)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("`unknown_const` does not resolve"));
}

#[test]
fn glob_import_with_workspace_evidence_still_reports_absent_name() {
    let source = r#"
use state::*;

mod state;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = unknown_const)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///workspace/program/src/state.rs").unwrap(),
            "pub const KNOWN_CONST: bool = true;".to_string(),
        )],
    );
    let diagnostics = collect_with_workspace(&document, Some(&index));

    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0]
        .message
        .contains("`unknown_const` does not resolve"));
}

#[test]
fn fixture_workspace_glob_import_resolves_cross_file_const() {
    let document = ParsedDocument::parse(include_str!(
        "../../../../tests/fixtures/resolution_fp/program/src/lib.rs"
    ))
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///workspace/program/src/state.rs").unwrap(),
            include_str!("../../../../tests/fixtures/resolution_fp/program/src/state.rs")
                .to_string(),
        )],
    );
    let diagnostics = collect_with_workspace(&document, Some(&index));

    assert!(
        diagnostics.is_empty(),
        "fixture should not emit resolution false positives: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_builtin_member_on_value_span() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = vault.decimals == 6)]
    pub state: Account<'info, State>,
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`vault.decimals` does not resolve")
        })
        .expect("unknown TokenAccount member diagnostic");

    assert!(matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-constraint-expression"
    ));
    assert_eq!(diagnostic.range.start.line, 3);
    assert_eq!(diagnostic.range.start.character, 33);
}

#[test]
fn validates_local_account_data_and_composite_members() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = position.position_mint == mint.key(),
        constraint = position.missing == mint.key(),
        constraint = bundle.inner_mint.key() == mint.key(),
        constraint = bundle.missing.key() == mint.key(),
    )]
    pub state: Account<'info, State>,
    pub position: Account<'info, Position>,
    pub bundle: Bundle<'info>,
    pub mint: Account<'info, Mint>,
}

#[derive(Accounts)]
pub struct Bundle<'info> {
    pub inner_mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`position.missing`")));
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`bundle.missing`")));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("position_mint` does not resolve")));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("inner_mint` does not resolve")));
}

#[test]
fn validates_account_loader_fields_after_load() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = position.load()?.position_mint == mint.key(),
        constraint = position.load_mut()?.bump == 1,
        constraint = position.load()?.missing == mint.key(),
    )]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`position.missing`")));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("position_mint` does not resolve")));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("bump` does not resolve")));
}

#[test]
fn rejects_direct_account_loader_data_field_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = position.position_mint == mint.key())]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`position.position_mint` does not resolve")
            && diagnostic.message.contains("AccountLoader<Position>")
    }));
}

#[test]
fn validates_nested_account_loader_field_chains() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        address = state.load()?.config.owner,
        constraint = state.load()?.config.missing == authority.key(),
    )]
    pub authority: Signer<'info>,
    pub state: AccountLoader<'info, State>,
}

#[account]
pub struct State {
    pub config: StateConfig,
}

pub struct StateConfig {
    pub owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`state.config.missing`")));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("owner` does not resolve")));
}

#[test]
fn validates_nested_direct_account_data_field_chains() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = state.config.missing == authority.key())]
    pub authority: Signer<'info>,
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub config: StateConfig,
}

pub struct StateConfig {
    pub owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`state.config.missing`")));
}
