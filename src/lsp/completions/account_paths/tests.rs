use {
    super::*,
    crate::{
        document::ParsedDocument, lsp::completions::proptest_support::rust_identifier,
        workspace::WorkspaceIndex,
    },
    proptest::prelude::*,
    tower_lsp::lsp_types::Url,
};

#[test]
fn completes_top_level_ctx_accounts_fields_after_typing() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.c;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "ctx.accounts.c"), None).unwrap();

    assert!(items.iter().any(|item| item.label == "counter"));
    assert!(!items.iter().any(|item| item.label == "authority"));
    let counter = items.iter().find(|item| item.label == "counter").unwrap();
    let Some(CompletionTextEdit::Edit(edit)) = &counter.text_edit else {
        panic!("expected prefix replacement edit");
    };
    assert_eq!(edit.new_text, "counter");
    assert_eq!(edit.range.start.character + 1, edit.range.end.character);
}

#[test]
fn completes_ctx_accounts_fields_after_dot_trigger() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "ctx.accounts."), None).unwrap();

    assert!(items.iter().any(|item| item.label == "counter"));
}

#[test]
fn completes_nested_composite_account_fields() {
    let source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Read>) -> Result<()> {
        let key = ctx.accounts.wrapper.i;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Read<'info> {
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "wrapper.i"), None).unwrap();

    assert_eq!(items[0].label, "inner");
    assert!(items[0]
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("Wrapped")));
}

#[test]
fn completes_account_data_fields_after_account_field() {
    let source = r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        ctx.accounts.counter.c;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}

#[account]
pub struct Counter {
    pub count: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "counter.c"), None).unwrap();

    assert_eq!(items[0].label, "count");
    assert!(items[0]
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("Counter")));
}

#[test]
fn completes_workspace_split_account_fields() {
    let lib = ParsedDocument::parse(
        r#"
#[program]
pub mod demo {
    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let counter = &mut ctx.accounts.c;
        Ok(())
    }
}
"#,
    )
    .unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
"#
            .to_string(),
        )],
    );

    let items = completions(
        &lib,
        position_after(lib.source(), "ctx.accounts.c"),
        Some(&index),
    )
    .unwrap();

    assert_eq!(items[0].label, "counter");
}

#[test]
fn completes_account_data_members_after_local_account_alias_dot() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.

    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/close_bundled_position.rs").unwrap(),
                r#"
#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub owner: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let items = completions(
        &document,
        position_after(source, "position_bundle."),
        Some(&index),
    )
    .unwrap();

    assert_eq!(items[0].label, "owner");
    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn completes_same_file_account_alias_members_when_handler_is_incomplete() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.

    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#
            .to_string(),
        )],
    );

    let items = completions(
        &document,
        position_after(source, "position_bundle."),
        Some(&index),
    )
    .expect("same-file account alias member completions should recover");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
    assert!(items.iter().any(|item| item.label == "position_bitmap"));
}

#[test]
fn completes_same_file_ctx_account_members_when_handler_is_incomplete() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    ctx.accounts.position_bundle.

    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
            .to_string(),
        )],
    );

    let items = completions(
        &document,
        position_after(source, "ctx.accounts.position_bundle."),
        Some(&index),
    )
    .expect("same-file ctx account member completions should recover");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn completes_account_data_members_after_accounts_alias_dot() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<CloseBundledPosition>) -> Result<()> {
    let afn = &mut ctx.accounts;
    let position_bundle = &mut afn.position_bundle;
    position_bundle.position_bundle_m;

    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/close_bundled_position.rs").unwrap(),
                r#"
#[derive(Accounts)]
pub struct CloseBundledPosition<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub owner: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let items = completions(
        &document,
        position_after(source, "position_bundle.position_bundle_m"),
        Some(&index),
    )
    .unwrap();

    assert_eq!(items[0].label, "position_bundle_mint");
}

proptest! {
    #[test]
    fn completes_members_for_generated_account_alias_shapes(
        alias in rust_identifier(),
        account_field in rust_identifier(),
        data_field in rust_identifier(),
        mutable in any::<bool>(),
        reference in any::<bool>(),
    ) {
        prop_assume!(alias != "ctx" && alias != account_field && alias != data_field && account_field != data_field);
        let member_prefix = data_field.chars().next().unwrap_or_default().to_string();
        let mutability = if mutable { "mut " } else { "" };
        let reference = if reference { "&" } else { "" };
        let source = format!(
            "use anchor_lang::prelude::*;\n\npub fn handler(ctx: Context<Run>) -> Result<()> {{\n    let {alias} = {reference}{mutability}ctx.accounts.{account_field};\n    {alias}.{member_prefix}\n    Ok(())\n}}\n"
        );
        let completion_line = format!("    {alias}.{member_prefix}");
        let document = ParsedDocument::parse_or_empty(&source);
        let index = WorkspaceIndex::build(
            &[],
            [
                (
                    Url::parse("file:///tmp/accounts.rs").unwrap(),
                    format!(
                        "#[derive(Accounts)]\npub struct Run<'info> {{\n    pub {account_field}: Account<'info, AccountData>,\n}}\n"
                    ),
                ),
                (
                    Url::parse("file:///tmp/state.rs").unwrap(),
                    format!(
                        "#[account]\npub struct AccountData {{\n    pub {data_field}: Pubkey,\n}}\n"
                    ),
                ),
            ],
        );

        let items = completions(
            &document,
            position_after(&source, &completion_line),
            Some(&index),
        )
        .expect("alias member completions should resolve");

        prop_assert!(items.iter().any(|item| item.label == data_field));
    }
}

fn position_after(source: &str, needle: &str) -> Position {
    let offset = source.find(needle).expect("needle") + needle.len();
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let character = prefix
        .rsplit('\n')
        .next()
        .map(|line| line.chars().count())
        .unwrap_or_default() as u32;
    Position { line, character }
}
