use crate::{diagnostics, document::ParsedDocument};

#[test]
fn reports_unknown_member_after_typed_tuple_handler_arg() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, (selected, _index): (PositionBundle, u16)) -> Result<()> {
    selected.real_fake;
    Ok(())
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#,
    ));

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "missing tuple handler-arg member diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_member_after_typed_tuple_local() {
    let diagnostics = diagnostics::collect(&ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let (selected, _index): (PositionBundle, u16) =
        (PositionBundle { real_mint: Pubkey::default() }, 0);
    selected.real_fake;
    Ok(())
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#,
    ));

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`selected.real_fake` does not resolve")),
        "missing tuple local member diagnostic: {diagnostics:#?}"
    );
}
