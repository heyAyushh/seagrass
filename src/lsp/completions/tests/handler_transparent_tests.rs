use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_account_data_members_after_as_ref_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position_bundle = ctx.accounts.position_bundle.as_ref();
    position_bundle.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "position_bundle.position_"),
    )
    .expect("as_ref account-data member completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn completes_account_data_members_after_deref_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position_bundle = &*ctx.accounts.position_bundle;
    position_bundle.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "position_bundle.position_"),
    )
    .expect("deref account-data member completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

proptest! {
    #[test]
    fn completes_generated_as_ref_account_members(
        account_field in rust_identifier(),
        alias in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(alias != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Account<'info, {owner}>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {alias} = ctx.accounts.{account_field}.as_ref();
    {alias}.{prefix}
}}

#[account]
pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{alias}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated as_ref account-data member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated as_ref account field completion, got {items:#?}"
        );
    }
}
