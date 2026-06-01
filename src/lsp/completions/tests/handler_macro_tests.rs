use {
    super::{completions, position_after},
    crate::document::ParsedDocument,
};

#[test]
fn completes_handler_members_inside_require_macro() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = &ctx.accounts.position_bundle;
    require!(
        bundle.position_,
        ErrorCode::BadBundle
    );
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub enum ErrorCode {
    BadBundle,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.position_"))
        .expect("require! member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn completes_handler_values_inside_require_macro() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
    pub authority: Signer<'info>,
}

pub fn handler(ctx: Context<Run>, bundle_index: u16) -> Result<()> {
    let copied_index = bundle_index;
    require!(
        copied_,
        ErrorCode::BadBundle
    );
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub enum ErrorCode {
    BadBundle,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "        copied_"))
        .expect("require! value completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"copied_index"));
}

#[test]
fn completes_empty_handler_value_slot_inside_require_macro() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn handler(ctx: Context<Run>, bundle_index: u16) -> Result<()> {
    let copied_index = bundle_index;
    require!(

    );
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "require!(\n"))
        .expect("empty require! value completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"copied_index"));
    assert!(labels.contains(&"position_bundle"));
}

#[test]
fn does_not_complete_empty_value_slot_inside_format_macro() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn handler(ctx: Context<Run>, bundle_index: u16) -> Result<()> {
    let copied_index = bundle_index;
    msg!(

    );
    Ok(())
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(
        completions(&document, position_after(source, "msg!(\n")).is_none(),
        "format-style macros should not get empty handler value completions"
    );
}
