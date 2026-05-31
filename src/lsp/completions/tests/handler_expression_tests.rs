use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_after_if_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = if use_first {
        bundles[0]
    } else {
        bundles.first().unwrap()
    };
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
        .expect("if-expression inferred member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_members_after_match_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle_index: u16, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = match bundle_index {
        0 => bundles[0],
        _ => bundles.first().unwrap(),
    };
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
        .expect("match-expression inferred member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "match-expression inferred member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_after_block_expression_infers_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundles: Vec<PositionBundle>) -> Result<()> {
    let selected = {
        bundles[0]
    };
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
        .expect("block-expression inferred member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "block-expression inferred member should complete: {items:#?}"
    );
}

#[test]
fn completes_members_on_direct_if_expression() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {
    (if use_first { bundles[0] } else { bundles.first().unwrap() }).real_
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
    let items = completions(&document, position_after(source, ").real_"))
        .expect("direct if-expression member completions");

    assert!(
        items.iter().any(|item| item.label == "real_mint"),
        "direct if-expression member should complete: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_if_expression_members(
        field_tail in rust_identifier(),
    ) {
        let field = format!("real_{field_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, use_first: bool, bundles: Vec<PositionBundle>) -> Result<()> {{
    let selected = if use_first {{
        bundles[0]
    }} else {{
        bundles.first().unwrap()
    }};
    selected.real_
}}

pub struct PositionBundle {{
    pub {field}: Pubkey,
}}

#[derive(Accounts)]
pub struct Run<'info> {{
    pub signer: Signer<'info>,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "selected.real_"))
            .expect("generated if-expression inferred member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "generated if-expression inferred member should complete; items: {items:#?}"
        );
    }
}
