use super::*;

#[test]
fn offers_semantic_missing_init_quickfix() {
    let source = r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add Anchor init constraints"))
        .expect("expected semantic missing-init quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("payer = user"));
    assert!(text_edit.new_text.contains("State::INIT_SPACE"));
}
#[test]
fn missing_init_quickfix_uses_first_typed_signer_not_payer_name() {
    let source = r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub state: Account<'info, State>,
    pub wallet: Signer<'info>,
    pub payer: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE))
        .unwrap();
    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        diagnostic.range,
        &diagnostics,
    );

    let action = actions
        .iter()
        .find(|action| action.title.contains("Add Anchor init constraints"))
        .expect("expected missing init quickfix");
    let edit = action.edit.as_ref().unwrap();
    let changes = edit.changes.as_ref().unwrap();
    let text_edit = changes.values().next().unwrap().first().unwrap();
    assert!(text_edit.new_text.contains("payer = wallet"));
    assert!(!text_edit.new_text.contains("payer = payer"));
}
#[test]
fn resolves_fix_all_missing_init_constraints_lazily() {
    let source = r#"
#[program]
mod demo {
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub state: Account<'info, State>,
    pub vault: Account<'info, Vault>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let diagnostics = crate::diagnostics::collect(&document);
    let actions = code_actions(&document, uri.clone(), diagnostics[0].range, &diagnostics);

    let action = actions
        .into_iter()
        .find(|action| action.title == "Fix all Anchor missing init constraints in file")
        .expect("expected source fix all action");
    assert!(action.edit.is_none());

    let resolved = resolve(&document, uri, action, &diagnostics);
    let edits = resolved
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.values().next())
        .expect("resolved fix-all edits");

    assert_eq!(edits.len(), 2);
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("State::INIT_SPACE")));
    assert!(edits
        .iter()
        .any(|edit| edit.new_text.contains("Vault::INIT_SPACE")));
}
