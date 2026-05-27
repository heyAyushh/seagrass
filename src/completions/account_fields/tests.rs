use {super::*, crate::workspace::WorkspaceIndex, tower_lsp::lsp_types::Url};

#[test]
fn completes_sysvar_type_inside_accounts_struct_after_typing() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[derive(Accounts)]
pub struct ReadClock<'info> {
    pub clock: Sy
}
"#,
    );

    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 17,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "Sysvar<'info, Clock>"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "Program<'info, System>"));
}

#[test]
fn completes_sysvar_field_snippet_inside_accounts_struct_after_typing() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub r
}
"#,
    );

    let completions = completions(
        &document,
        Position {
            line: 3,
            character: 9,
        },
    )
    .unwrap();

    let rent = completions
        .iter()
        .find(|item| item.label == "rent: Sysvar<'info, Rent>")
        .expect("expected rent sysvar field completion");
    assert_eq!(
        rent.insert_text.as_deref(),
        Some("rent: Sysvar<'info, Rent>,")
    );
}

#[test]
fn completes_field_snippets_after_pub_space_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub 
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "pub ")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "rent: Sysvar<'info, Rent>"));
    assert!(completions
        .iter()
        .any(|item| item.label == "system_program: Program<'info, System>"));
}

#[test]
fn filters_field_name_snippets_by_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub r
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "pub r")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "rent: Sysvar<'info, Rent>"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "clock: Sysvar<'info, Clock>"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program: Program<'info, System>"));
}

#[test]
fn filters_field_type_completions_by_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sy
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "pub rent: Sy")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "Sysvar<'info, Rent>"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "Program<'info, System>"));
}

#[test]
fn ranks_field_type_completions_by_field_role() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sy
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "pub rent: Sy")).unwrap();

    assert_eq!(completions[0].label, "Sysvar<'info, Rent>");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn completes_field_type_after_single_character_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: S
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "pub rent: S")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "Sysvar<'info, Rent>"));
}

#[test]
fn completes_sysvar_generic_argument_without_field_snippets() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, R
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Sysvar<'info, R")).unwrap();

    assert!(completions.iter().any(|item| item.label == "Rent"));
    assert!(!completions.iter().any(|item| item.label == "Rewards"));
    assert!(!completions
        .iter()
        .any(|item| item.label.starts_with("slot_hashes:")));
    assert!(!completions
        .iter()
        .any(|item| item.label.starts_with("rent:")));
}

#[test]
fn stays_quiet_for_lowercase_type_argument_that_does_not_match_field_role() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, s
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(completions(&document, position_after(source, "Sysvar<'info, s")).is_none());
}

#[test]
fn completes_field_role_type_argument_from_lowercase_field_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, r
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Sysvar<'info, r")).unwrap();

    assert!(completions.iter().any(|item| item.label == "Rent"));
    assert!(!completions.iter().any(|item| item.label == "Rewards"));
}

#[test]
fn sysvar_generic_arguments_match_anchor_parser_sysvar_types() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRecentBlockhashes<'info> {
    pub recent_blockhashes: Sysvar<'info, R
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Sysvar<'info, R")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "RecentBlockhashes"));
    assert!(!completions.iter().any(|item| item.label == "Rewards"));
    assert!(!completions.iter().any(|item| item.label == "Rent"));
}

#[test]
fn completes_generic_argument_from_field_role_after_space_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct ReadRent<'info> {
    pub rent: Sysvar<'info, 
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Sysvar<'info, ")).unwrap();

    assert_eq!(completions[0].label, "Rent");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn completes_program_generic_argument_after_space_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub token_program: Program<'info, 
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Program<'info, ")).unwrap();

    assert!(completions.iter().any(|item| item.label == "System"));
    assert!(completions.iter().any(|item| item.label == "Token"));
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn ranks_interface_account_mint_generic_above_workspace_data_on_empty_slot() {
    let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    pub whirlpool: Box<Account<'info, Whirlpool>>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, 
}

#[account]
pub struct AdaptiveFeeTier {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(
        &document,
        position_after(source, "pub token_mint_b: InterfaceAccount<'info, "),
    )
    .unwrap();

    assert_eq!(completions[0].label, "Mint");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn ranks_interface_account_token_account_generic_from_field_role() {
    let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    #[account(mut, token::mint = mint, token::authority = token_owner_account_b)]
    pub token_owner_account_b: InterfaceAccount<'info, 
    pub mint: InterfaceAccount<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(
        &document,
        position_after(source, "InterfaceAccount<'info, "),
    )
    .unwrap();

    assert_eq!(completions[0].label, "TokenAccount");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn ranks_token_account_generic_from_custom_constraint_data_field_access() {
    let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    pub position: Account<'info, Position>,
    #[account(
        constraint = position_token_account.mint == position.position_mint,
        constraint = position_token_account.amount == 1,
    )]
    pub position_token_account: InterfaceAccount<'info, 
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(
        &document,
        position_after(
            source,
            "pub position_token_account: InterfaceAccount<'info, ",
        ),
    )
    .unwrap();

    assert_eq!(completions[0].label, "TokenAccount");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn completes_local_account_data_generic_argument() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, S>
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let completions = completions(&document, position_after(source, "Account<'info, S")).unwrap();

    assert!(completions.iter().any(|item| item.label == "State"));
    assert!(!completions
        .iter()
        .any(|item| item.label.starts_with("account:")));
}

#[test]
fn completes_workspace_account_data_generic_argument() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, S>
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
#[account]
pub struct State {
    pub value: u64,
}
"#
            .to_string(),
        )],
    );

    let completions = completions_with_workspace(
        &document,
        position_after(source, "Account<'info, S"),
        Some(&workspace_index),
    )
    .unwrap();

    assert!(completions.iter().any(|item| item.label == "State"));
}

fn position_after(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle in source") + needle.len();
    let prefix = &source[..offset];
    let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count()).unwrap();
    let character = prefix
        .lines()
        .next_back()
        .map(|line| u32::try_from(line.chars().count()).unwrap())
        .unwrap_or(0);
    Position { line, character }
}
