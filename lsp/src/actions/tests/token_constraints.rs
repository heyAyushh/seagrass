use super::*;

#[test]
fn offers_missing_token_mint_constraint_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, token::authority = payer)]
    pub vault: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
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
                    .data
                    .as_ref()
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("token::mint")
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
        .find(|action| action.title.contains("token::mint = mint"))
        .expect("expected missing token mint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("token::mint = mint"));
}
#[test]
fn token_mint_quickfix_uses_typed_mint_not_misleading_field_name() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = signer, token::authority = signer)]
    pub vault: Account<'info, TokenAccount>,
    pub mint: AccountInfo<'info>,
    pub asset: InterfaceAccount<'info, Mint>,
    pub signer: Signer<'info>,
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
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("token::mint")
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
        .find(|action| action.title.contains("token::mint = asset"))
        .expect("expected typed mint quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("token::mint = asset"));
    assert!(!text_edit.new_text.contains("token::mint = mint"));
}
#[test]
fn offers_missing_associated_token_authority_constraint_quickfix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::mint = mint)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
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
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("associated_token::authority")
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
        .find(|action| action.title.contains("associated_token::authority = payer"))
        .expect("expected missing associated token authority quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("associated_token::authority = payer"));
}
#[test]
fn associated_token_authority_quickfix_uses_typed_signer_not_payer_name() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::mint = mint)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    pub payer: AccountInfo<'info>,
    pub wallet: Signer<'info>,
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
                    .and_then(|data| data.get("missing"))
                    .and_then(|value| value.as_str())
                    == Some("associated_token::authority")
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
        .find(|action| {
            action
                .title
                .contains("associated_token::authority = wallet")
        })
        .expect("expected typed signer authority quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit
        .new_text
        .contains("associated_token::authority = wallet"));
    assert!(!text_edit
        .new_text
        .contains("associated_token::authority = payer"));
}
