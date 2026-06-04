use {
    super::{collect, collect_with_workspace},
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::Url,
};

#[test]
fn validates_address_expression_member_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = state.config.missing)]
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

#[test]
fn validates_address_expression_before_custom_error() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = state.config.missing @ ErrorCode::InvalidAuthority)]
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

#[test]
fn validates_owner_expression_identifier_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(owner = missing_owner)]
    pub metadata_program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`missing_owner` does not resolve")));
}

#[test]
fn reports_unbound_const_like_address_identifier() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`TOKEN_METADATA_PROGRAM_ID` does not resolve")));
}

#[test]
fn accepts_imported_const_like_address_identifier() {
    let source = r#"
use mpl_token_metadata::ID as TOKEN_METADATA_PROGRAM_ID;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "imported const-like address identifiers should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn reports_unbound_multisegment_address_path() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = fake_metadata::TOKEN_METADATA_PROGRAM_ID)]
    pub metadata_program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`fake_metadata::TOKEN_METADATA_PROGRAM_ID` does not resolve")));
}

#[test]
fn accepts_imported_external_address_path() {
    let source = r#"
use mpl_token_metadata;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = mpl_token_metadata::ID)]
    pub metadata_program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "imported external path should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn accepts_crate_declared_program_id_path() {
    let source = r#"
declare_id!("11111111111111111111111111111111");

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = crate::ID)]
    pub program: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "declared program id path should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn reports_unbound_space_identifier() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = random_jargon)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`random_jargon` does not resolve")));
}

#[test]
fn accepts_space_type_constant_path() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "valid space expression should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn validates_instruction_argument_struct_members() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(
        constraint = params.position_mint == mint.key(),
        constraint = params.missing == mint.key(),
    )]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`params.missing`")));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("position_mint` does not resolve")));
}

#[test]
fn reports_instruction_argument_field_called_as_method() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(constraint = params.position_mint() == Pubkey::default())]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("calls `position_mint` as a method")));
}

#[test]
fn reports_unknown_associated_space_constant() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = 8 + State::FAKE_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`State::FAKE_SPACE` does not resolve")));
}

#[test]
fn accepts_declared_associated_space_constant_and_function() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = State::SPACE + State::dynamic_space())]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}

impl State {
    const SPACE: usize = 8 + 8;

    fn dynamic_space() -> usize {
        Self::SPACE
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "declared associated values should resolve: {diagnostics:#?}"
    );
}

#[test]
fn accepts_workspace_associated_space_constant() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = SharedState::SPACE)]
    pub state: Account<'info, SharedState>,
    #[account(mut)]
    pub payer: Signer<'info>,
}
"#;
    let workspace_source = r#"
#[account]
pub struct SharedState {
    pub value: u64,
}

impl SharedState {
    const SPACE: usize = 8 + 8;
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_document = ParsedDocument::parse(workspace_source).unwrap();
    let mut workspace = WorkspaceIndex::default();
    workspace.upsert_parsed_open_document(
        Url::parse("file:///workspace/shared_state.rs").unwrap(),
        &workspace_document,
    );
    let diagnostics = collect_with_workspace(&document, Some(&workspace));

    assert!(
        diagnostics.is_empty(),
        "workspace associated constants should resolve: {diagnostics:#?}"
    );
}

#[test]
fn reports_unbound_unknown_assignment_value() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(random_key = random_jargon)]
    pub state: UncheckedAccount<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`random_jargon` does not resolve")));
}

#[test]
fn reports_unbound_program_reference_path() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump, seeds::program = fake_program::ID)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`fake_program::ID`")));
}

#[test]
fn accepts_keyword_assignment_value() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(zero, rent_exempt = skip)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`skip` does not resolve")),
        "keyword values are catalog literals, not variables: {diagnostics:#?}"
    );
}

#[test]
fn validates_constraint_call_identifier_bindings() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        constraint = authority_is_allowed(authority.key()),
        constraint = fake_checker(authority.key()),
    )]
    pub authority: Signer<'info>,
}

fn authority_is_allowed(authority: Pubkey) -> bool {
    authority != Pubkey::default()
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`fake_checker` does not resolve")));
    assert!(!diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`authority_is_allowed` does not resolve")));
}

#[test]
fn accepts_imported_constraint_call_identifier() {
    let source = r#"
use anchor_lang::prelude::*;
use crate::auth::admin::is_admin_key;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = is_admin_key(funder.key))]
    pub funder: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        !diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`is_admin_key` does not resolve")),
        "imported constraint helper calls should resolve: {diagnostics:#?}"
    );
}

#[test]
fn validates_unresolved_seed_identifier() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, name: String) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Run<'info> {
    #[account(seeds = [name.as_bytes(), missing_seed.as_ref()], bump)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`missing_seed` does not resolve")));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`name` does not resolve")));
}

#[test]
fn validates_seed_account_data_member_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(
        seeds = [
            state.config.owner.as_ref(),
            state.config.missing.as_ref(),
        ],
        bump,
    )]
    pub vault: SystemAccount<'info>,
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
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("owner` does not resolve")));
}

#[test]
fn validates_bump_account_data_member_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump = state.missing_bump)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`state.missing_bump`")));
}

#[test]
fn validates_bump_account_data_member_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump = state.nonce)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub nonce: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`state.nonce` resolves to `u64`, but `bump` requires `u8`")
    }));
}

#[test]
fn validates_workspace_account_data_member_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = state.missing_owner)]
    pub authority: Signer<'info>,
    pub state: Account<'info, SharedState>,
}
"#;
    let workspace_source = r#"
#[account]
pub struct SharedState {
    pub owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_document = ParsedDocument::parse(workspace_source).unwrap();
    let mut workspace = WorkspaceIndex::default();
    workspace.upsert_parsed_open_document(
        Url::parse("file:///workspace/shared_state.rs").unwrap(),
        &workspace_document,
    );
    let diagnostics = collect_with_workspace(&document, Some(&workspace));

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`state.missing_owner`")));
}

#[test]
fn accepts_ambiguous_workspace_account_data_member_access_as_unknown() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = state.missing_owner)]
    pub authority: Signer<'info>,
    pub state: Account<'info, SharedState>,
}
"#;
    let first_workspace_source = r#"
#[account]
pub struct SharedState {
    pub owner: Pubkey,
}
"#;
    let second_workspace_source = r#"
#[account]
pub struct SharedState {
    pub alternate_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let first_workspace_document = ParsedDocument::parse(first_workspace_source).unwrap();
    let second_workspace_document = ParsedDocument::parse(second_workspace_source).unwrap();
    let mut workspace = WorkspaceIndex::default();
    workspace.upsert_parsed_open_document(
        Url::parse("file:///workspace/state_a.rs").unwrap(),
        &first_workspace_document,
    );
    workspace.upsert_parsed_open_document(
        Url::parse("file:///workspace/state_b.rs").unwrap(),
        &second_workspace_document,
    );
    let diagnostics = collect_with_workspace(&document, Some(&workspace));

    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`state.missing_owner`")),
        "ambiguous workspace account data should stay unknown: {diagnostics:#?}"
    );
}

#[test]
fn validates_workspace_bump_member_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump = state.nonce)]
    pub state: Account<'info, SharedState>,
}
"#;
    let workspace_source = r#"
#[account]
pub struct SharedState {
    pub nonce: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_document = ParsedDocument::parse(workspace_source).unwrap();
    let mut workspace = WorkspaceIndex::default();
    workspace.upsert_parsed_open_document(
        Url::parse("file:///workspace/shared_state.rs").unwrap(),
        &workspace_document,
    );
    let diagnostics = collect_with_workspace(&document, Some(&workspace));

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("`state.nonce` resolves to `u64`, but `bump` requires `u8`")
    }));
}

#[test]
fn accepts_bump_instruction_argument_identifier() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bump: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump = bump)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "simple bump instruction argument should stay on the existing instruction-argument path: {diagnostics:#?}"
    );
}

#[test]
fn accepts_bound_complex_helper_seed_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [some_helper(user.key().as_ref())], bump)]
    pub vault: SystemAccount<'info>,
    pub user: Signer<'info>,
}

fn some_helper(seed: &[u8]) -> &[u8] {
    seed
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "complex helper seeds should stay on the PDA IDL-visibility diagnostic path: {diagnostics:#?}"
    );
}

#[test]
fn reports_unbound_complex_helper_seed_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [fake_helper(user.key().as_ref())], bump)]
    pub vault: SystemAccount<'info>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`fake_helper` does not resolve")));
}

#[test]
fn reports_unbound_explicit_bump_identifier() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump = FAKE_BUMP)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`FAKE_BUMP` does not resolve")));
}
