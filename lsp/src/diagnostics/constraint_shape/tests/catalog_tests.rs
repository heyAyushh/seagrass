use super::*;

#[test]
fn reports_invalid_rent_exempt_keyword() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(zero, rent_exempt = maybe)]
    pub state: Account<'info, LargeState>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only accepts `skip` or `enforce`"
    ));
}
#[test]
fn generic_keyword_diagnostic_catches_invalid_rent_exempt() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(zero, rent_exempt = maybe)]
    pub state: Account<'info, LargeState>,
}
"#,
    );

    assert!(has_code_and_message(
        &diagnostics,
        ANCHOR_CONSTRAINT_SHAPE_CODE,
        "only accepts `skip` or `enforce`"
    ));
}
#[test]
fn generated_init_like_keys_contains_init_and_init_if_needed() {
    assert!(constraint_catalog::INIT_LIKE_KEYS.contains(&"init"));
    assert!(constraint_catalog::INIT_LIKE_KEYS.contains(&"init_if_needed"));
}
#[test]
fn generated_mutability_keys_contains_expected_constraints() {
    assert!(constraint_catalog::MUTABILITY_IMPLYING_KEYS.contains(&"mut"));
    assert!(constraint_catalog::MUTABILITY_IMPLYING_KEYS.contains(&"init"));
    assert!(constraint_catalog::MUTABILITY_IMPLYING_KEYS.contains(&"realloc"));
    assert!(constraint_catalog::MUTABILITY_IMPLYING_KEYS.contains(&"close"));
    assert!(constraint_catalog::MUTABILITY_IMPLYING_KEYS.contains(&"zero"));
}
