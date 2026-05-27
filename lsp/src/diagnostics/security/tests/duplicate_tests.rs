use super::*;

#[test]
fn flags_duplicate_account_types_without_distinct_key_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    let duplicate_diagnostics =
        diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
    assert_eq!(duplicate_diagnostics.len(), 2);
    assert!(duplicate_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity == Some(DiagnosticSeverity::WARNING)));
    assert!(duplicate_diagnostics
        .iter()
        .all(|diagnostic| diagnostic.range.start.line == 4 || diagnostic.range.start.line == 6));
}

#[test]
fn accepts_duplicate_account_types_with_constraint() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, constraint = user_a.key() != user_b.key())]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_duplicate_account_types_with_deref_constraint() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, constraint = (*user_a).key() != (*user_b).key())]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_duplicate_account_types_with_conjunctive_constraint() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, constraint = user_a.key() != user_b.key() && guard)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_duplicate_account_types_with_deref_runtime_check() {
    let diagnostics = security_diagnostics(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn update(ctx: Context<Update>) -> ProgramResult {
        if (*ctx.accounts.user_a).key() == (*ctx.accounts.user_b).key() {
            return Err(ProgramError::InvalidArgument);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn rejects_distinct_key_text_inside_unrelated_constraint_expression() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, constraint = memo == "user_a.key() != user_b.key()")]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_eq!(
        diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE).len(),
        2
    );
}

#[test]
fn accepts_duplicate_account_types_with_runtime_check() {
    let diagnostics = security_diagnostics(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn update(ctx: Context<Update>) -> ProgramResult {
        if ctx.accounts.user_a.key() == ctx.accounts.user_b.key() {
            return Err(ProgramError::InvalidArgument);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_readonly_duplicate_account_types() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Read<'info> {
    user_a: Account<'info, User>,
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_duplicate_mutable_account_with_dup_constraint() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    user_a: Account<'info, User>,
    #[account(mut, dup)]
    user_b: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn accepts_pure_init_duplicate_account_type() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + 8)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn flags_init_if_needed_duplicate_account_type() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init_if_needed, payer = payer, space = 8 + 8)]
    user_a: Account<'info, User>,
    #[account(mut)]
    user_b: Account<'info, User>,
    #[account(mut)]
    payer: Signer<'info>,
    system_program: Program<'info, System>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_eq!(
        diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE).len(),
        2
    );
}

#[test]
fn accepts_non_serializing_duplicate_mutable_types() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    user_a: AccountLoader<'info, User>,
    #[account(mut)]
    user_b: AccountLoader<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn flags_duplicate_mutable_account_across_composite_accounts() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    #[account(mut)]
    direct: Account<'info, User>,
    nested: Nested<'info>,
}

#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(mut)]
    inner: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    let duplicate_diagnostics =
        diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
    assert_eq!(duplicate_diagnostics.len(), 2);
    assert!(duplicate_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`direct` in `Outer`")));
    assert!(duplicate_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`nested.inner` in `Nested`")));
}

#[test]
fn accepts_composite_duplicate_mutable_account_with_inner_dup() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    #[account(mut)]
    direct: Account<'info, User>,
    nested: Nested<'info>,
}

#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(mut, dup)]
    inner: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}

#[test]
fn flags_duplicate_mutable_account_across_workspace_composite_accounts() {
    let outer = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    #[account(mut)]
    direct: Account<'info, User>,
    nested: Nested<'info>,
}
"#,
    )
    .unwrap();
    let nested_source = r#"
#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(mut)]
    inner: Account<'info, User>,
}

#[account]
pub struct User {
    data: u64,
}
"#;
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/outer.rs").unwrap(),
                outer.source().to_string(),
            ),
            (
                Url::parse("file:///tmp/nested.rs").unwrap(),
                nested_source.to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&outer, Some(&index));
    let duplicate_diagnostics =
        diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
    assert_eq!(duplicate_diagnostics.len(), 1);
    assert!(duplicate_diagnostics[0]
        .message
        .contains("`nested.inner` in `Nested`"));
    assert_eq!(
        duplicate_diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("peerPath"))
            .and_then(|value| value.as_str()),
        Some("nested.inner")
    );
}

#[test]
fn accepts_workspace_composite_duplicate_with_distinct_constraint() {
    let outer = ParsedDocument::parse(
        r#"
#[derive(Accounts)]
pub struct Outer<'info> {
    #[account(mut, constraint = direct.key() != nested.inner.key())]
    direct: Account<'info, User>,
    nested: Nested<'info>,
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/outer.rs").unwrap(),
                outer.source().to_string(),
            ),
            (
                Url::parse("file:///tmp/nested.rs").unwrap(),
                r#"
#[derive(Accounts)]
pub struct Nested<'info> {
    #[account(mut)]
    inner: Account<'info, User>,
}
"#
                .to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&outer, Some(&index));
    assert_no_code(&diagnostics, ANCHOR_SECURITY_DUPLICATE_ACCOUNT_CODE);
}
