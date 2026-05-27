use super::*;

#[test]
fn reports_has_one_target_missing_from_account_data() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
pub struct State {
    pub owner: Pubkey,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `authority` field"
    ));
    assert_related_information(
        constraint_shape_diagnostic_with_message(&diagnostics, "has no `authority` field"),
        &[
            "`state` is typed as `State`",
            "`State` declares candidate field `owner`",
        ],
    );
}
#[test]
fn accepts_has_one_target_present_on_account_data() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
pub struct State {
    pub authority: Pubkey,
}
"#,
    );

    assert!(!diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_CONSTRAINT_SHAPE_CODE
        ) && diagnostic.message.contains("has no `authority` field")
    }));
}
#[test]
fn reports_has_one_target_missing_from_workspace_account_data() {
    let diagnostics = diagnostics_for_workspace(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}
"#,
        r#"
#[account]
pub struct State {
    pub owner: Pubkey,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "has no `authority` field"
    ));
}
#[test]
fn accepts_has_one_target_from_workspace_account_data() {
    let diagnostics = diagnostics_for_workspace(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}
"#,
        r#"
#[account]
pub struct State {
    pub authority: Pubkey,
}
"#,
    );

    assert!(!diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_CONSTRAINT_SHAPE_CODE
        ) && diagnostic.message.contains("has no `authority` field")
    }));
}
