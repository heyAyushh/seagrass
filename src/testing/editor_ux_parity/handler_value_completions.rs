use crate::{completions, document::ParsedDocument};

#[test]
fn editor_ux_completes_empty_handler_expression_slots() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>, bundle_index: u16) -> Result<Pubkey> {
    let copied_index = bundle_index;
    let receiver_key = ctx.accounts.receiver.key();
    let selected =
    verify_bundle(
    return /*cursor*/
}

fn verify_bundle(_bundle_index: u16) {}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_completion_contains(&document, source, "let selected =", "copied_index");
    assert_completion_contains(&document, source, "verify_bundle(", "receiver_key");
    assert_completion_contains(&document, source, "return ", "receiver_key");
}

#[test]
fn editor_ux_completes_program_module_handler_values() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    use super::*;

    const EXPECTED_LIMIT: u16 = 16;

    pub fn handler(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let selected = EXPECTED_
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_completion_contains(
        &document,
        source,
        "let selected = EXPECTED_",
        "EXPECTED_LIMIT",
    );
}

#[test]
fn editor_ux_completes_block_item_handler_values() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    let selected = LOCAL_

    const LOCAL_LIMIT: u16 = 64;
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_completion_contains(&document, source, "let selected = LOCAL_", "LOCAL_LIMIT");
}

fn assert_completion_contains(
    document: &ParsedDocument,
    source: &str,
    marker: &str,
    expected_label: &str,
) {
    let items = completions::completions(document, super::position_after(source, marker))
        .unwrap_or_else(|| panic!("missing empty handler value completions at `{marker}`"));
    assert!(
        items.iter().any(|item| item.label == expected_label),
        "missing `{expected_label}` completion at `{marker}`; got {:?}",
        items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>()
    );
}
