use super::*;

#[test]
fn reports_incomplete_realloc_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(realloc = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `realloc::payer"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `realloc::zero"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "not marked `mut`"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `Program<'info, System>`"
    ));
}
#[test]
fn reports_malformed_system_program_for_realloc() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info>,
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
                .contains("because `state` uses `realloc`")
        })
        .expect("expected malformed system_program diagnostic for realloc");
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str()),
        Some("system-program-type")
    );
}
#[test]
fn accepts_complete_realloc_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!has_code(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE));
}
#[test]
fn reports_close_without_mut() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Delete<'info> {
    #[account(close = user)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `close` but is not marked `mut`"
    ));
}
#[test]
fn accepts_close_with_zero_constraint_as_effectively_mut() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Delete<'info> {
    #[account(zero, close = user)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(!has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `close` but is not marked `mut`"
    ));
}
#[test]
fn reports_zero_with_explicit_mut_conflict() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(mut, zero)]
    pub state: Account<'info, LargeState>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "combines `zero` with `mut`"
    ));
}
#[test]
fn reports_init_without_system_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `Program<'info, System>`"
    ));
}
#[test]
fn reports_init_on_system_account() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: SystemAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses `init` on `SystemAccount`"
    ));
}
#[test]
fn reports_init_if_needed_without_system_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `Program<'info, System>`"
    ));
}
#[test]
fn reports_realloc_on_unsupported_account_type() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8, realloc::payer = payer, realloc::zero = false)]
    pub state: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only allows `realloc` on `Account`, `LazyAccount`, or `AccountLoader`"
    ));
}
#[test]
fn reports_close_on_unsupported_account_type() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Close<'info> {
    #[account(mut, close = payer)]
    pub signer: Signer<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only allows `close` on `Account`, `LazyAccount`, or `AccountLoader`"
    ));
}
#[test]
fn accepts_realloc_and_close_on_account_loader() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8, realloc::payer = payer, realloc::zero = false, close = payer)]
    pub state: AccountLoader<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only allows `realloc`"
    ));
    assert!(!has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only allows `close`"
    ));
}
#[test]
fn reports_malformed_system_program_on_system_program_field() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info>,
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
                .contains("must be typed `Program<'info, System>`")
        })
        .expect("expected malformed system_program diagnostic");
    assert_eq!(diagnostic.range.start.line, 6);
    assert!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("quickfix"))
            .and_then(|quickfix| quickfix.as_str())
            == Some("system-program-type")
    );
}
#[test]
fn accepts_complete_init_shape() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(!has_code(&diagnostics, ANCHOR_CONSTRAINT_SHAPE_CODE));
}
#[test]
fn reports_required_init_with_optional_system_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Option<Program<'info, System>>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`system_program` is optional but `state` is required"
    ));
}
#[test]
fn accepts_optional_init_if_needed_with_optional_system_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Option<Account<'info, State>>,
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    pub system_program: Option<Program<'info, System>>,
}
"#,
    );

    assert!(!has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "is optional but `state` is required"
    ));
}
#[test]
fn reports_init_payer_without_mut() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`user` pays for `state` via `payer` but is missing `#[account(mut)]`"
    ));
}
#[test]
fn reports_required_init_with_optional_payer() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Option<Signer<'info>>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`payer` pays for required `state` via `payer` but is optional"
    ));
}
#[test]
fn reports_realloc_payer_without_mut() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`user` pays for `state` via `realloc::payer` but is missing `#[account(mut)]`"
    ));
}
#[test]
fn reports_required_realloc_with_optional_payer_and_system_program() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8 + State::INIT_SPACE, realloc::payer = user, realloc::zero = false)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub user: Option<Signer<'info>>,
    pub system_program: Option<Program<'info, System>>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`user` pays for required `state` via `realloc::payer` but is optional"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "`system_program` is optional but `state` is required"
    ));
}
#[test]
fn reports_init_payer_as_initialized_account() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = state, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub system_program: Program<'info, System>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "uses itself as `payer`"
    ));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("anchorError"))
        .and_then(|error| error.as_str())
        == Some("TryingToInitPayerAsProgramAccount")));
}
#[test]
fn realloc_diagnostics_use_catalog_companions() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(realloc = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `realloc::payer"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "missing `realloc::zero"
    ));
    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "not marked `mut`"
    ));
}
