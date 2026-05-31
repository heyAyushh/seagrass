use {
    super::{completions, position_after, should_offer_completion},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_visible_handler_locals_and_instruction_args() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    let copied_index = bundle_index;
    let selected = pos
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let local_items = completions(&document, position_after(source, "let selected = pos"))
        .expect("expected local value completions");
    assert_eq!(local_items[0].label, "position_bundle");
    assert_eq!(
        local_items[0].insert_text.as_deref(),
        Some("position_bundle")
    );

    let arg_items = completions(&document, position_after(source, "let copied_index = bun"))
        .expect("expected argument completions");
    assert!(arg_items.iter().any(|item| item.label == "bundle_index"));
}

#[test]
fn completes_account_fields_as_handler_values() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
    pub position_bundle: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let rent_receiver = rec
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let rent_receiver = rec"))
        .expect("expected account field value completions");
    let receiver = items
        .iter()
        .find(|item| item.label == "receiver")
        .expect("receiver account field completion");

    assert_eq!(
        receiver.insert_text.as_deref(),
        Some("ctx.accounts.receiver")
    );
}

#[test]
fn handler_value_completion_stays_quiet_on_let_lhs() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let rec
}
"#;

    assert!(!should_offer_completion(
        source,
        position_after(source, "let rec")
    ));
}

#[test]
fn handler_value_completion_stays_quiet_outside_anchor_handlers() {
    let source = r#"
fn helper() {
    let position_bundle = 1;
    let selected = pos
}
"#;

    assert!(!should_offer_completion(
        source,
        position_after(source, "let selected = pos")
    ));
}

#[test]
fn handler_value_completion_ignores_out_of_scope_block_locals() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    if true {
        let scoped_value = 1;
    }
    let selected = sco
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let selected = sco"));

    assert!(
        items
            .unwrap_or_default()
            .iter()
            .all(|item| item.label != "scoped_value"),
        "block-local value should not leak into outer handler completions"
    );
}

#[test]
fn completes_if_let_pattern_binding_inside_then_block() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let maybe_receiver = Some(ctx.accounts.receiver.key());
    if let Some(position_bundle) = maybe_receiver {
        let selected = pos
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let selected = pos"))
        .expect("expected if-let pattern value completions");

    assert!(
        items.iter().any(|item| item.label == "position_bundle"),
        "if-let pattern binding should complete inside then block: {items:#?}"
    );
}

#[test]
fn completes_match_arm_pattern_binding_inside_arm_body() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let maybe_receiver = Some(ctx.accounts.receiver.key());
    match maybe_receiver {
        Some(position_bundle) => {
            let selected = pos
        }
        _ => {}
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let selected = pos"))
        .expect("expected match arm pattern value completions");

    assert!(
        items.iter().any(|item| item.label == "position_bundle"),
        "match arm pattern binding should complete inside arm body: {items:#?}"
    );
}

#[test]
fn completes_closure_pattern_binding_inside_closure_body() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>) -> Result<()> {
    let visit = |position_bundle: Pubkey| {
        let selected = pos;
    };
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let selected = pos"))
        .expect("expected closure pattern value completions");

    assert!(
        items.iter().any(|item| item.label == "position_bundle"),
        "closure pattern binding should complete inside closure body: {items:#?}"
    );
}

#[test]
fn completes_for_loop_pattern_binding_inside_loop_body() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {
    pub receiver: AccountInfo<'info>,
}

pub fn handler(ctx: Context<Close>, bundles: Vec<Pubkey>) -> Result<()> {
    for position_bundle in bundles.iter() {
        let selected = pos;
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let items = completions(&document, position_after(source, "let selected = pos"))
        .expect("expected for-loop pattern value completions");

    assert!(
        items.iter().any(|item| item.label == "position_bundle"),
        "for-loop pattern binding should complete inside loop body: {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_handler_locals_without_hardcoded_names(
        local_tail in rust_identifier(),
        argument_tail in rust_identifier(),
        account_tail in rust_identifier(),
    ) {
        let local = format!("shared_local_{local_tail}");
        let argument = format!("shared_arg_{argument_tail}");
        let account = format!("shared_account_{account_tail}");
        prop_assume!(local != argument && local != account && argument != account);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {{
    pub {account}: AccountInfo<'info>,
}}

pub fn handler(ctx: Context<Close>, {argument}: u64) -> Result<()> {{
    let {local} = {argument};
    let selected = shared_
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let cursor = "let selected = shared_";
        let items = completions(&document, position_after(&source, &cursor))
            .expect("expected generated handler value completions");

        prop_assert!(
            items.iter().any(|item| item.label == local),
            "generated local should complete; items: {items:#?}"
        );
        prop_assert!(
            items.iter().any(|item| item.label == argument),
            "generated argument should complete; items: {items:#?}"
        );
        prop_assert!(
            items.iter().any(|item| item.label == account),
            "generated account field should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_if_let_pattern_bindings_without_hardcoded_names(
        pattern_tail in rust_identifier(),
        account_tail in rust_identifier(),
    ) {
        let pattern = format!("pattern_value_{pattern_tail}");
        let account = format!("account_value_{account_tail}");
        prop_assume!(pattern != account);
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {{
    pub {account}: AccountInfo<'info>,
}}

pub fn handler(ctx: Context<Close>) -> Result<()> {{
    let maybe_value = Some(ctx.accounts.{account}.key());
    if let Some({pattern}) = maybe_value {{
        let selected = pattern_
    }}
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "let selected = pattern_"))
            .expect("expected generated if-let pattern completions");

        prop_assert!(
            items.iter().any(|item| item.label == pattern),
            "generated if-let pattern should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_closure_pattern_bindings_without_hardcoded_names(
        pattern_tail in rust_identifier(),
    ) {
        let pattern = format!("pattern_value_{pattern_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {{
    pub receiver: AccountInfo<'info>,
}}

pub fn handler(ctx: Context<Close>) -> Result<()> {{
    let visit = |{pattern}: Pubkey| {{
        let selected = pattern_;
    }};
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "let selected = pattern_"))
            .expect("expected generated closure pattern completions");

        prop_assert!(
            items.iter().any(|item| item.label == pattern),
            "generated closure pattern should complete; items: {items:#?}"
        );
    }

    #[test]
    fn completes_generated_for_loop_pattern_bindings_without_hardcoded_names(
        pattern_tail in rust_identifier(),
    ) {
        let pattern = format!("pattern_value_{pattern_tail}");
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Close<'info> {{
    pub receiver: AccountInfo<'info>,
}}

pub fn handler(ctx: Context<Close>, bundles: Vec<Pubkey>) -> Result<()> {{
    for {pattern} in bundles.iter() {{
        let selected = pattern_;
    }}
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let items = completions(&document, position_after(&source, "let selected = pattern_"))
            .expect("expected generated for-loop pattern completions");

        prop_assert!(
            items.iter().any(|item| item.label == pattern),
            "generated for-loop pattern should complete; items: {items:#?}"
        );
    }
}
