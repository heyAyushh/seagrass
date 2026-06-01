use {
    super::{completions, position_after},
    crate::document::ParsedDocument,
};

#[test]
fn completes_members_after_typed_tuple_handler_arg() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, (selected, _index): (PositionBundle, u16)) -> Result<()> {
    selected.real_
}

pub struct PositionBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("typed tuple handler-arg member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_typed_tuple_local() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let (selected, _index): (PositionBundle, u16) =
        (PositionBundle { real_mint: Pubkey::default() }, 0);
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
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "selected.real_"))
        .expect("typed tuple local member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "typed tuple local should complete known members: {items:#?}"
    );
}
