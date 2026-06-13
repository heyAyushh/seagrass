use super::*;

#[test]
fn reports_seeds_without_bump() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(seeds = [user.key().as_ref()])]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "without `bump`"
    ));
    assert_related_information(
        constraint_shape_diagnostic_with_message(&diagnostics, "without `bump`"),
        &[
            "`seeds` makes this account a PDA",
            "`state` is the PDA account field",
        ],
    );
}
#[test]
fn reports_bump_without_seeds() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(bump)]
    pub state: Account<'info, State>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "without `seeds`"
    ));
    assert_related_information(
        constraint_shape_diagnostic_with_message(&diagnostics, "without `seeds`"),
        &[
            "`bump` only validates a PDA",
            "`state` is the account field carrying the standalone bump",
        ],
    );
}
#[test]
fn reports_seeds_program_without_seeds() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds::program = token_metadata_program.key())]
    pub metadata: AccountInfo<'info>,
    pub token_metadata_program: Program<'info, Metadata>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `seeds::program` but is missing `seeds"
    ));
}
#[test]
fn reports_associated_token_with_seeds_conflict() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        associated_token::mint = mint,
        associated_token::authority = payer,
        seeds = [b"token", payer.key().as_ref()],
        bump,
    )]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_CONSTRAINT_SHAPE_CODE
            ) && diagnostic
                .message
                .contains("combines `associated_token::*` with `seeds`")
        })
        .expect("expected associated token/seeds conflict diagnostic");
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str()),
        Some("remove-conflicting-constraints")
    );
}
#[test]
fn reports_init_with_seeds_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE, seeds = [b"state"], bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub other_program: Program<'info, System>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `init` with `seeds::program`"
    ));
}
#[test]
fn reports_init_if_needed_with_seeds_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + State::INIT_SPACE, seeds = [b"state"], bump, seeds::program = other_program.key())]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub other_program: Program<'info, System>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `init_if_needed` with `seeds::program`"
    ));
}
#[test]
fn reports_static_only_pda_seeds() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Vault<'info> {
    #[account(seeds = [b"vault"], bump)]
    pub vault: Account<'info, VaultState>,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_SECURITY_STATIC_PDA_CODE
            )
        })
        .expect("static PDA diagnostic");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::HINT));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str()),
        None
    );
}
#[test]
fn reports_static_only_pda_seeds_with_scoped_seed_candidate() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Vault<'info> {
    #[account(seeds = [b"vault"], bump)]
    pub vault: Account<'info, VaultState>,
    pub authority: Signer<'info>,
}
"#,
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == ANCHOR_SECURITY_STATIC_PDA_CODE
            )
        })
        .expect("static PDA diagnostic");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::HINT));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str()),
        Some("add-scoped-pda-seed")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("scopedSeed"))
            .and_then(|seed| seed.as_str()),
        Some("authority.key().as_ref()")
    );
}
#[test]
fn reports_too_many_pda_seeds_from_parser_projection() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(seeds = [user.key().as_ref(), b"01", b"02", b"03", b"04", b"05", b"06", b"07", b"08", b"09", b"10", b"11", b"12", b"13", b"14", b"15", b"16"], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_CONSTRAINT_SHAPE_CODE
        ) && diagnostic.message.contains("at most 16 seeds")
            && diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("seedCount"))
                .and_then(|value| value.as_u64())
                == Some(17)
    }));
}
#[test]
fn reports_pda_seed_literal_over_32_bytes_from_parser_projection() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(seeds = [b"123456789012345678901234567890123", user.key().as_ref()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_CONSTRAINT_SHAPE_CODE
        ) && diagnostic.message.contains("at most 32 bytes per seed")
            && diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("seedLength"))
                .and_then(|value| value.as_u64())
                == Some(33)
    }));
}
#[test]
fn accepts_pda_seed_literal_at_32_bytes() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UsePda<'info> {
    #[account(seeds = [b"12345678901234567890123456789012", user.key().as_ref()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(!diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.message.contains("at most 32 bytes per seed") }));
}
#[test]
fn accepts_account_scoped_pda_seeds() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Vault<'info> {
    #[account(seeds = [b"vault", user.key().as_ref()], bump)]
    pub vault: Account<'info, VaultState>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(!has_code(&diagnostics, ANCHOR_SECURITY_STATIC_PDA_CODE));
}
