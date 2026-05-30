use {
    super::{apply_text_edit, code_actions},
    crate::{diagnostics, document::ParsedDocument},
    tower_lsp::lsp_types::{Range, Url},
};

#[test]
fn offers_constraint_expression_member_replacement() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = position.position_mnit == mint.key())]
    pub state: Account<'info, State>,
    pub position: Account<'info, Position>,
    pub mint: Account<'info, Mint>,
}

#[account]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("position_mnit"))
        .expect("expected unknown member diagnostic");

    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        Range {
            start: diagnostic.range.start,
            end: diagnostic.range.start,
        },
        &diagnostics,
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Replace `position_mnit` with `position_mint`")
        .expect("expected member replacement quickfix");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.values().next())
        .and_then(|edits| edits.first())
        .expect("expected replacement edit");

    let updated = apply_text_edit(source, edit);
    assert!(updated.contains("position.position_mint == mint.key()"));
}

#[test]
fn offers_constraint_expression_identifier_replacement() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = authortiy.key() == signer.key())]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("`authortiy` does not resolve"))
        .expect("expected unresolved identifier diagnostic");

    let actions = code_actions(
        &document,
        Url::parse("file:///tmp/lib.rs").unwrap(),
        Range {
            start: diagnostic.range.start,
            end: diagnostic.range.start,
        },
        &diagnostics,
    );
    let action = actions
        .iter()
        .find(|action| action.title == "Replace `authortiy` with `authority`")
        .expect("expected identifier replacement quickfix");
    let edit = action
        .edit
        .as_ref()
        .and_then(|edit| edit.changes.as_ref())
        .and_then(|changes| changes.values().next())
        .and_then(|edits| edits.first())
        .expect("expected replacement edit");

    let updated = apply_text_edit(source, edit);
    assert!(updated.contains("constraint = authority.key() == signer.key()"));
}
