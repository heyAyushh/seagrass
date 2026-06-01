use crate::{completions, diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_resolves_typed_tuple_handler_members() {
    let completion_source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, (selected, _index): (PositionBundle, u16)) -> Result<()> {
    selected.real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(completion_source);
    let items = completions::completions(
        &document,
        super::position_after(completion_source, "selected.real_"),
    )
    .expect("editor-visible tuple pattern member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "tuple handler pattern should complete known members: {items:#?}"
    );

    let diagnostic_source = completion_source.replace("selected.real_", "selected.real_fake;");
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(&diagnostic_source));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "tuple handler pattern should flag unknown members: {diagnostics:#?}"
    );
}
