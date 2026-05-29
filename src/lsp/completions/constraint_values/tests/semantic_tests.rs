use super::*;

#[test]
fn filters_workspace_has_one_to_pubkey_account_data_fields() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = a)]
    pub state: Account<'info, SplitState>,
    pub authority: Signer<'info>,
    pub amount: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/split-state.rs").unwrap(),
            r#"
#[account]
pub struct SplitState {
    pub authority: Pubkey,
    pub amount: u64,
}
"#
            .to_string(),
        )],
    );
    let completions = completions_with_workspace(
        &document,
        position_after(source, "has_one = a"),
        Some(&workspace_index),
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "authority"));
    assert!(!completions.iter().any(|item| item.label == "amount"));
}

#[test]
fn falls_back_for_has_one_when_account_data_is_unknown() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = a)]
    pub state: Account<'info, MissingState>,
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "has_one = a")).unwrap();

    assert!(completions.iter().any(|item| item.label == "authority"));
    assert!(!completions.iter().any(|item| item.label == "payer"));
}

#[test]
fn completes_token_account_members_in_constraint_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = vault.)]
    pub state: Account<'info, State>,
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "constraint = vault.")).unwrap();

    assert!(completions.iter().any(|item| item.label == "owner"));
    assert!(completions.iter().any(|item| item.label == "mint"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "mint_authority"));
}

#[test]
fn completes_mint_members_in_constraint_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = mint.)]
    pub state: Account<'info, State>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "constraint = mint.")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "mint_authority"));
    assert!(completions.iter().any(|item| item.label == "decimals"));
    assert!(!completions.iter().any(|item| item.label == "owner"));
}

#[test]
fn filters_member_completion_by_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = vault.m)]
    pub state: Account<'info, State>,
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "constraint = vault.m")).unwrap();

    assert_eq!(completions[0].label, "mint");
    assert!(!completions.iter().any(|item| item.label == "owner"));
}

#[test]
fn completes_account_data_members_in_constraint_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = state.c)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub count: u64,
    pub authority: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "constraint = state.c")).unwrap();

    assert_eq!(completions[0].label, "count");
    assert!(!completions.iter().any(|item| item.label == "authority"));
}

#[test]
fn completes_account_loader_members_after_load() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = position.load()?.p)]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "constraint = position.load()?.p"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "position_mint");
    assert!(!completions.iter().any(|item| item.label == "bump"));
}

#[test]
fn completes_account_loader_members_after_load_mut() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = position.load_mut()?.p)]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "constraint = position.load_mut()?.p"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "position_mint");
    assert!(!completions.iter().any(|item| item.label == "bump"));
}

#[test]
fn completes_account_loader_load_method_before_data_access() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = position.lo)]
    pub state: Account<'info, State>,
    pub position: AccountLoader<'info, Position>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "constraint = position.lo"),
    )
    .expect("AccountLoader method completions");

    assert_eq!(completions[0].label, "load()?");
    assert!(!completions.iter().any(|item| item.label == "position_mint"));
}

#[test]
fn completes_nested_account_loader_members_after_load() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(address = state.load()?.config.o)]
    pub authority: Signer<'info>,
    pub state: AccountLoader<'info, State>,
}

#[account]
pub struct State {
    pub config: StateConfig,
    pub vault_nonce: u8,
}

pub struct StateConfig {
    pub owner: Pubkey,
    pub token_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "address = state.load()?.config.o"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "owner");
    assert!(!completions.iter().any(|item| item.label == "token_mint"));
}

#[test]
fn completes_nested_account_data_members() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(address = state.config.o)]
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
    let completions = completions(
        &document,
        position_after(source, "address = state.config.o"),
    )
    .expect("nested account-data completions");

    assert_eq!(completions[0].label, "owner");
}

#[test]
fn completes_u8_account_data_members_for_explicit_bump() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state"], bump = state.)]
    pub state: Account<'info, State>,
}

#[account]
pub struct State {
    pub bump: u8,
    pub nonce: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "bump = state."))
        .expect("bump account-data completions");

    assert_eq!(completions[0].label, "bump");
    assert!(!completions.iter().any(|item| item.label == "nonce"));
}

#[test]
fn completes_generated_init_space_associated_const() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = State::)]
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
    let completions = completions(&document, position_after(source, "space = State::"))
        .expect("associated init space completions");

    assert_eq!(completions[0].label, "INIT_SPACE");
}

#[test]
fn completes_declared_associated_space_values_after_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = State::SP)]
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
    let completions = completions(&document, position_after(source, "space = State::SP"))
        .expect("associated space completions");

    assert_eq!(completions[0].label, "SPACE");
    assert!(!completions
        .iter()
        .any(|item| item.label == "dynamic_space()"));
}

#[test]
fn completes_workspace_associated_space_values() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = SharedState::)]
    pub state: Account<'info, SharedState>,
    #[account(mut)]
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/shared_state.rs").unwrap(),
            r#"
#[account]
pub struct SharedState {
    pub value: u64,
}

impl SharedState {
    const SPACE: usize = 8 + 8;

    fn dynamic_space() -> usize {
        Self::SPACE
    }
}
"#
            .to_string(),
        )],
    );
    let completions = completions_with_workspace(
        &document,
        position_after(source, "space = SharedState::"),
        Some(&workspace_index),
    )
    .expect("workspace associated space completions");

    assert!(completions.iter().any(|item| item.label == "SPACE"));
    assert!(completions
        .iter()
        .any(|item| item.label == "dynamic_space()"));
}

#[test]
fn completes_workspace_nested_account_data_members() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(address = state.config.o)]
    pub authority: Signer<'info>,
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
#[account]
pub struct State {
    pub config: StateConfig,
}

pub struct StateConfig {
    pub owner: Pubkey,
}
"#
            .to_string(),
        )],
    );
    let completions = completions_with_workspace(
        &document,
        position_after(source, "address = state.config.o"),
        Some(&workspace_index),
    )
    .expect("workspace nested account-data completions");

    assert_eq!(completions[0].label, "owner");
}

#[test]
fn suppresses_workspace_member_completion_for_ambiguous_account_data_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(address = state.o)]
    pub authority: Signer<'info>,
    pub state: Account<'info, SharedState>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/state_a.rs").unwrap(),
                r#"
#[account]
pub struct SharedState {
    pub owner: Pubkey,
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state_b.rs").unwrap(),
                r#"
#[account]
pub struct SharedState {
    pub other_owner: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );
    let completions = completions_with_workspace(
        &document,
        position_after(source, "address = state.o"),
        Some(&workspace_index),
    );

    assert!(
        completions.is_none(),
        "ambiguous workspace account data should not produce guessed members: {completions:#?}"
    );
}

#[test]
fn completes_composite_members_in_constraint_expression() {
    let source = r#"
#[derive(Accounts)]
pub struct Check<'info> {
    #[account(constraint = bundle.inner)]
    pub state: Account<'info, State>,
    pub bundle: Bundle<'info>,
}

#[derive(Accounts)]
pub struct Bundle<'info> {
    pub inner_mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "constraint = bundle.inner"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "inner_mint");
}

#[test]
fn ranks_token_program_by_program_name_role() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::token_program = t)]
    pub vault: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_program: Program<'info, Token>,
    pub token_program_wrong: Program<'info, System>,
    pub token_program_unchecked: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "token::token_program = t"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "token_program");
    assert_eq!(completions[0].preselect, Some(true));
    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "associated_token_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "token_program_wrong"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "token_program_unchecked"));
}

#[test]
fn filters_associated_token_token_program_to_token_programs() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::token_program = t)]
    pub vault: Account<'info, TokenAccount>,
    pub system_program: Program<'info, System>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub token_2022_program: Program<'info, Token2022>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "associated_token::token_program = t"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "token_2022_program");
    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "associated_token_program"));
}

#[test]
fn filters_interface_account_token_program_to_token_interface() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::token_program = t)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub legacy_token_program: Program<'info, Token>,
    pub token_2022_program: Program<'info, Token2022>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "token::token_program = t"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "token_program");
    assert!(completions.iter().any(|item| item.label == "token_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "legacy_token_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "token_2022_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program"));
}

#[test]
fn filters_interface_associated_token_program_to_token_interface() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::token_program = t)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    pub legacy_token_program: Program<'info, Token>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "associated_token::token_program = t"),
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "token_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "legacy_token_program"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "associated_token_program"));
}

#[test]
fn keeps_general_program_completion_for_seeds_program() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds::program = m)]
    pub metadata: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds::program = m")).unwrap();

    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program"));
    assert!(completions
        .iter()
        .any(|item| item.label == "metadata_program"));
}

#[test]
fn completes_pda_seed_account_key_after_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [u])]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds = [u")).unwrap();

    assert_eq!(completions[0].label, "user.key().as_ref()");
    assert!(!completions
        .iter()
        .any(|item| item.label == "state.key().as_ref()"));
}

#[test]
fn completes_multiline_constraint_values_after_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = u
    )]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "payer = u")).unwrap();

    assert!(completions.iter().any(|item| item.label == "user"));
}

#[test]
fn completes_static_seed_from_current_account_field() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b])]
    pub vault_state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds = [b")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "b\"vault_state\""));
}

#[test]
fn completes_pda_seed_instruction_arguments_by_type() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, name: String, id: u64, raw: Vec<u8>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [n])]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let name_items = completions(&document, position_after(source, "seeds = [n")).unwrap();
    assert_eq!(name_items[0].label, "name.as_bytes()");

    let id_source = source.replace("seeds = [n", "seeds = [i");
    let id_document = ParsedDocument::parse(&id_source).unwrap();
    let id_items = completions(&id_document, position_after(&id_source, "seeds = [i")).unwrap();
    assert_eq!(id_items[0].label, "id.to_le_bytes().as_ref()");
    assert!(!id_items.iter().any(|item| item.label.starts_with("raw")));
}

#[test]
fn completes_empty_seed_list_after_space_or_manual_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [])]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let items = completions(&document, position_after(source, "seeds = ["))
        .expect("expected seed completions after list opener");

    assert!(items.iter().any(|item| item.label == "user.key().as_ref()"));
}
