use crate::{completions, document::ParsedDocument};

#[test]
fn editor_ux_completes_member_on_boxed_account_alias_after_dot() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    assert_member_completion_contains(&document, "position_bundle.", "position_bundle_mint");
    assert_member_completion_contains(&document, "position_bundle.", "position_bitmap");
}

#[test]
fn editor_ux_completes_direct_ctx_account_member_after_dot() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    ctx.accounts.position_bundle.
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );

    assert_member_completion_contains(
        &document,
        "ctx.accounts.position_bundle.",
        "position_bundle_mint",
    );
    assert_member_completion_contains(
        &document,
        "ctx.accounts.position_bundle.",
        "position_bitmap",
    );
}

fn assert_member_completion_contains(
    document: &ParsedDocument,
    marker: &str,
    expected_label: &str,
) {
    let items =
        completions::completions(document, super::position_after(document.source(), marker))
            .unwrap_or_else(|| panic!("missing member completions at `{marker}`"));
    assert!(
        items.iter().any(|item| item.label == expected_label),
        "missing `{expected_label}` completion at `{marker}`; got {:?}",
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
}
