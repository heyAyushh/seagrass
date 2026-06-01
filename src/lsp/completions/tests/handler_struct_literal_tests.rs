use {
    super::{completions, position_after},
    crate::{
        document::ParsedDocument,
        lsp::completions::{
            completion_signature, should_offer_completion, CompletionSignatureKind,
        },
    },
};

#[test]
fn completes_handler_struct_literal_fields_from_resolved_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = PositionBundle { position_ };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "PositionBundle { position_"),
    )
    .expect("struct literal field completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert_eq!(items[0].label, "position_bitmap");
    assert!(labels.contains(&"position_bundle_mint"));
}

#[test]
fn completes_handler_struct_literal_fields_after_anchor_lifetimes() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub fn handler(ctx: Context<Run>, signer: Signer<'_>) -> Result<()> {
    let _ = signer;
    let _bundle = crate::state::PositionBundle { position_ };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "PositionBundle { position_"),
    )
    .expect("struct literal field completions after lifetimes");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn wakes_handler_struct_literal_fields_in_empty_field_slot() {
    let source = r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = PositionBundle {  };
    Ok(())
}
"#;
    let position = position_after(source, "PositionBundle { ");
    let signature = completion_signature(source, position).expect("completion signature");
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position).expect("empty struct field completions");

    assert_eq!(signature.kind, CompletionSignatureKind::HandlerStructField);
    assert_eq!(items[0].label, "position_bundle_mint");
    assert!(should_offer_completion(source, position));
}

#[test]
fn skips_already_initialized_handler_struct_literal_fields() {
    let source = r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = PositionBundle {
        position_bundle_mint: Pubkey::default(),
            position_
    };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "\n            position_"))
        .expect("struct literal field completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bitmap"));
    assert!(!labels.contains(&"position_bundle_mint"));
}

#[test]
fn stays_quiet_for_unknown_handler_struct_literal_type() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = ExternalBundle { pos };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(completions(&document, position_after(source, "ExternalBundle { pos")).is_none());
}
