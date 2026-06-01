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
