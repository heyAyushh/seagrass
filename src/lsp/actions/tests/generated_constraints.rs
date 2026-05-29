use super::*;

#[test]
fn offers_generated_signer_type_quickfix() {
    let source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct Admin<'info> {
    pub authority: UncheckedAccount<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn admin(ctx: Context<Admin>) -> Result<()> {
        let metas = vec![AccountMeta::new_readonly(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-security-signer"))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Signer<'info>"))
        .expect("expected typed signer quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert_eq!(text_edit.new_text, "Signer<'info>");
}
#[test]
fn offers_generated_duplicate_constraint_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, mut)]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && parser_rule_kind(diagnostic) == Some("duplicate")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Remove duplicate Anchor `mut`"))
        .expect("expected duplicate constraint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[account(mut)]"));
    assert!(!updated.contains("mut, mut"));
}
#[test]
fn offers_generated_ordering_constraint_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(payer = user, init, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && parser_rule_kind(diagnostic) == Some("ordering")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Move Anchor `init` before `payer`"))
        .expect("expected ordering constraint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[account(init, payer = user, space = 8 + State::INIT_SPACE)]"));
}
#[test]
fn offers_remove_conflicting_associated_token_seeds_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer, seeds = [b"token"], bump)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("quickfix"))
                    .and_then(|value| value.as_str())
                    == Some("remove-conflicting-constraints")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Remove conflicting"))
        .expect("expected remove conflict quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("associated_token::mint = mint"));
    assert!(updated.contains("associated_token::authority = payer"));
    assert!(!updated.contains("seeds ="));
    assert!(!updated.contains("bump"));
}
#[test]
fn offers_remove_spl_init_space_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 82, mint::decimals = 6, mint::authority = payer)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .data
                    .as_ref()
                    .and_then(|data| data.get("remove"))
                    .and_then(|value| value.as_array())
                    .is_some_and(|remove| remove.iter().any(|value| value == "space"))
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("`space`"))
        .expect("expected remove space quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("mint::decimals = 6"));
    assert!(updated.contains("mint::authority = payer"));
    assert!(!updated.contains("space ="));
}
#[test]
fn offers_remove_zero_mut_conflict_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(mut, zero, rent_exempt = skip)]
    pub state: Account<'info, LargeState>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic.message.contains("combines `zero` with `mut`")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("`mut`"))
        .expect("expected remove mut quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[account(zero, rent_exempt = skip)]"));
    assert!(!updated.contains("mut,"));
}
#[test]
fn offers_replace_invalid_rent_exempt_keyword_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct InitializeLarge<'info> {
    #[account(zero, rent_exempt = skp)]
    pub state: Account<'info, LargeState>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .message
                    .contains("only accepts `skip` or `enforce`")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("`skip`"))
        .expect("expected rent_exempt keyword replacement quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[account(zero, rent_exempt = skip)]"));
}
#[test]
fn offers_remove_invalid_realloc_group_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Grow<'info> {
    #[account(mut, realloc = 8, realloc::payer = payer, realloc::zero = false)]
    pub state: UncheckedAccount<'info>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-constraint-shape")
                && diagnostic
                    .message
                    .contains("only allows `realloc` on `Account`")
        })
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Remove conflicting"))
        .expect("expected invalid realloc removal quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    let updated = apply_text_edit(source, text_edit);
    assert!(updated.contains("#[account(mut)]"));
    assert!(!updated.contains("realloc"));
}
