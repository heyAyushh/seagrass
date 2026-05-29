use super::*;

#[test]
fn reports_associated_token_mint_without_authority() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::mint = mint)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `associated_token::authority"
    ));
    assert!(!has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `token::authority"
    ));
}
#[test]
fn reports_associated_token_authority_without_mint() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `associated_token::mint"
    ));
}
#[test]
fn reports_associated_token_program_without_mint_and_authority() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::token_program = token_program)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `associated_token::mint"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `associated_token::authority"
    ));
}
#[test]
fn reports_incomplete_mint_init_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `mint::decimals"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `mint::authority"
    ));
}
#[test]
fn reports_incomplete_mint_init_if_needed_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = payer)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `mint::decimals"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `mint::authority"
    ));
}
#[test]
fn accepts_complete_mint_init_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, mint::decimals = token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!has_code(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE));
}
#[test]
fn reports_space_on_spl_mint_init() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 82, mint::decimals = 6, mint::authority = payer)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`space` is not required"
    ));
}
#[test]
fn reports_space_on_associated_token_init() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 165, associated_token::mint = mint, associated_token::authority = payer)]
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

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`space` is not required"
    ));
}
#[test]
fn reports_token_init_without_token_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `token_program` account"
    ));
}
#[test]
fn reports_interface_token_account_with_legacy_mint_wrapper() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "is not `InterfaceAccount<'info, Mint>`"
    ));
}
#[test]
fn accepts_interface_token_account_with_interface_mint_wrapper() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub mint: InterfaceAccount<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!has_code(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE));
}
#[test]
fn reports_required_token_init_with_optional_token_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Option<Program<'info, Token>>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`token_program` is optional but `token` is required"
    ));
}
#[test]
fn reports_associated_token_init_without_associated_token_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `associated_token_program` account"
    ));
}
#[test]
fn reports_required_associated_token_init_with_optional_associated_token_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Option<Program<'info, AssociatedToken>>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`associated_token_program` is optional but `token` is required"
    ));
}
#[test]
fn reports_malformed_token_program_for_token_init() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, System>,
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
                .contains("must be an Anchor token program account")
        })
        .expect("expected malformed token_program diagnostic");
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str()),
        Some("program-field-type")
    );
}
#[test]
fn reports_incomplete_token_account_init_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer)]
    pub token: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `token::mint"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `token::authority"
    ));
}
#[test]
fn reports_incomplete_token_account_init_if_needed_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = payer)]
    pub token: Account<'info, TokenAccount>,
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `token::mint"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `token::authority"
    ));
}
#[test]
fn accepts_associated_token_account_init_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
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

    assert!(!has_code(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE));
}
#[test]
fn reports_token_constraints_on_wrong_type() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UseToken<'info> {
    #[account(token::mint = mint, token::authority = owner)]
    pub token: Account<'info, State>,
    pub mint: Account<'info, Mint>,
    pub owner: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "token account constraints require `Account<'info, TokenAccount>`"
    ));
}
#[test]
fn reports_mint_constraints_on_wrong_type() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct UseMint<'info> {
    #[account(mint::decimals = decimals, mint::authority = owner)]
    pub mint: Account<'info, State>,
    pub owner: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "mint constraints require `Account<'info, Mint>`"
    ));
}
