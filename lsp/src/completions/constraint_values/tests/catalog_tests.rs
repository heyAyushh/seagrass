use super::*;

#[test]
fn completes_values_for_every_generated_catalog_value_kind() {
    for spec in constraint_catalog::CONSTRAINTS.iter().filter(|spec| {
        matches!(
            spec.value_kind,
            ConstraintValueKind::Boolean
                | ConstraintValueKind::Keyword
                | ConstraintValueKind::Space
                | ConstraintValueKind::Seeds
        )
    }) {
        let key = constraint_catalog::key(spec.label);
        let (prefix, expected) = match spec.value_kind {
            ConstraintValueKind::Boolean => ("t", "true"),
            ConstraintValueKind::Keyword => ("s", "skip"),
            ConstraintValueKind::Space => ("8", "8 + State::INIT_SPACE"),
            ConstraintValueKind::Seeds => ("u", "user.key().as_ref()"),
            _ => unreachable!(),
        };
        let (value, needle) = if spec.value_kind == ConstraintValueKind::Seeds {
            (format!("[{prefix}]"), format!("{key} = [{prefix}"))
        } else {
            (prefix.to_string(), format!("{key} = {prefix}"))
        };
        let source = generated_value_completion_source(
            key,
            &value,
            "token_decimals: u8, bump: u8, seed_name: String",
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let completions = completions(&document, position_after(&source, &needle))
            .unwrap_or_else(|| panic!("expected completions for generated constraint `{key}`"));

        assert!(
            completions.iter().any(|item| item.label == expected),
            "generated constraint `{key}` did not suggest `{expected}`; got {:?}",
            completions
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn completes_boolean_constraint_value_from_generated_catalog() {
    let source = r#"
#[derive(Accounts)]
pub struct Resize<'info> {
    #[account(realloc::zero = t)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "realloc::zero = t")).unwrap();

    assert!(completions.iter().any(|item| item.label == "true"));
    assert!(!completions.iter().any(|item| item.label == "false"));
}

#[test]
fn completes_rent_exempt_parser_keywords_from_generated_catalog() {
    let source = r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(zero, rent_exempt = s)]
    pub state: Account<'info, LargeState>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "rent_exempt = s")).unwrap();

    assert!(completions.iter().any(|item| item.label == "skip"));
    assert!(!completions.iter().any(|item| item.label == "enforce"));
    assert!(completions
        .iter()
        .all(|item| item.kind == Some(CompletionItemKind::KEYWORD)));
}

#[test]
fn completes_close_recipient_accounts_from_generated_catalog() {
    let source = r#"
#[derive(Accounts)]
pub struct CloseState<'info> {
    #[account(mut, close = rec)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub receiver: SystemAccount<'info>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "close = rec")).unwrap();

    assert!(completions.iter().any(|item| item.label == "receiver"));
    assert!(!completions.iter().any(|item| item.label == "state"));
}

#[test]
fn ranks_token_mint_value_by_mint_account_role() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = m)]
    pub vault: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub mint_authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "token::mint = m")).unwrap();

    assert_eq!(completions[0].label, "mint");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn filters_token_mint_value_to_mint_accounts() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = m)]
    pub vault: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub mint_authority: Signer<'info>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "token::mint = m")).unwrap();

    assert_eq!(completions[0].label, "mint");
    assert!(!completions.iter().any(|item| item.label == "payer"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "mint_authority"));
}

#[test]
fn token_mint_completion_uses_typed_mint_not_misleading_name() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = a)]
    pub vault: Account<'info, TokenAccount>,
    pub mint: AccountInfo<'info>,
    pub asset: InterfaceAccount<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "token::mint = a")).unwrap();

    assert_eq!(completions[0].label, "asset");
    assert!(completions.iter().any(|item| item.label == "asset"));
    assert!(!completions.iter().any(|item| item.label == "mint"));
}

#[test]
fn token_mint_completion_uses_semantic_mint_role_for_unresolved_inner_type() {
    let source = r#"
#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    pub whirlpool: Box<Account<'info, Whirlpool>>,
    #[account(mut, constraint = token_owner_account_b.mint == whirlpool.token_mint_b)]
    pub token_owner_account_b: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(address = whirlpool.token_mint_b)]
    pub token_mint_b: InterfaceAccount<'info, M>,
    #[account(token::mint = token_)]
    pub vault: InterfaceAccount<'info, TokenAccount>,
    pub payer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "token::mint = token_")).unwrap();

    assert_eq!(completions[0].label, "token_mint_b");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn filters_associated_token_authority_to_authority_accounts() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::authority = p)]
    pub ata: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
    pub random_account: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "associated_token::authority = p"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "payer");
    assert!(!completions.iter().any(|item| item.label == "mint"));
    assert!(!completions
        .iter()
        .any(|item| item.label == "random_account"));
}

#[test]
fn associated_token_authority_completion_uses_typed_signer_not_misleading_name() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::authority = w)]
    pub ata: Account<'info, TokenAccount>,
    pub payer: AccountInfo<'info>,
    pub wallet: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        position_after(source, "associated_token::authority = w"),
    )
    .unwrap();

    assert_eq!(completions[0].label, "wallet");
    assert!(!completions.iter().any(|item| item.label == "payer"));
}

#[test]
fn token_authority_completion_allows_current_pda_token_account() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::authority = v)]
    pub vault: Account<'info, TokenAccount>,
    pub random: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "token::authority = v")).unwrap();

    assert_eq!(completions[0].label, "vault");
    assert!(completions.iter().any(|item| item.label == "vault"));
    assert!(!completions.iter().any(|item| item.label == "random"));
}

#[test]
fn ranks_mint_authority_by_authority_role() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::authority = a)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position_after(source, "mint::authority = a")).unwrap();

    assert_eq!(completions[0].label, "authority");
    assert_eq!(completions[0].preselect, Some(true));
}

#[test]
fn ranks_account_constraint_value_top_three_by_context() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = pa)]
    pub state: Account<'info, State>,
    #[account(token::mint = pa)]
    pub vault: Account<'info, TokenAccount>,
    #[account(has_one = pa)]
    pub profile: Account<'info, Profile>,
    #[account(mint::authority = pa)]
    pub new_mint: Account<'info, Mint>,
    #[account(token::token_program = pa)]
    pub token_vault: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub payment_authority: Signer<'info>,
    pub passive_signer: Signer<'info>,
    pub paper_mint: Account<'info, Mint>,
    pub payment_token_program: Program<'info, Token>,
    pub payment_2022_program: Program<'info, Token2022>,
    pub passive_token_program: Program<'info, TokenInterface>,
    pub patron_unchecked: AccountInfo<'info>,
}

#[account]
pub struct Profile {
    pub payment_authority: Pubkey,
    pub paper_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let payer_items = completions(&document, position_after(source, "payer = pa")).unwrap();
    let mint_items = completions(&document, position_after(source, "token::mint = pa")).unwrap();
    let has_one_items = completions(&document, position_after(source, "has_one = pa")).unwrap();
    let authority_items =
        completions(&document, position_after(source, "mint::authority = pa")).unwrap();
    let token_program_items = completions(
        &document,
        position_after(source, "token::token_program = pa"),
    )
    .unwrap();

    assert_eq!(
        top_labels(&payer_items, 3),
        vec!["payer", "payment_authority", "passive_signer"]
    );
    assert_eq!(top_labels(&mint_items, 3), vec!["paper_mint"]);
    assert_eq!(
        top_labels(&has_one_items, 3),
        vec!["payment_authority", "paper_mint"]
    );
    assert_eq!(
        top_labels(&authority_items, 3),
        vec!["payer", "payment_authority", "passive_signer"]
    );
    assert_eq!(
        top_labels(&token_program_items, 3),
        vec![
            "payment_token_program",
            "payment_2022_program",
            "passive_token_program"
        ]
    );
}

#[test]
fn completes_has_one_from_account_data_fields() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = a)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    pub random: AccountInfo<'info>,
}

#[account]
pub struct State {
    pub authority: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "has_one = a")).unwrap();

    assert_eq!(completions[0].label, "authority");
    assert!(completions.iter().any(|item| item.label == "authority"));
    assert!(!completions.iter().any(|item| item.label == "payer"));
    assert!(!completions.iter().any(|item| item.label == "random"));
}

#[test]
fn completes_has_one_from_workspace_account_data_fields() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = a)]
    pub state: Account<'info, SplitState>,
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
    pub random: AccountInfo<'info>,
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

    assert_eq!(completions[0].label, "authority");
    assert!(completions.iter().any(|item| item.label == "authority"));
    assert!(!completions.iter().any(|item| item.label == "payer"));
    assert!(!completions.iter().any(|item| item.label == "random"));
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
