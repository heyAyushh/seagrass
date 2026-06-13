use {
    super::{completions, position_after},
    crate::{
        document::ParsedDocument,
        lsp::{
            completions::proptest_support::{rust_identifier, rust_type_identifier},
            local_types,
        },
    },
    proptest::prelude::*,
};

#[test]
fn completes_account_loader_methods_before_loaded_data() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position: AccountLoader<'info, Position>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position = &ctx.accounts.position;
    position.lo
}

#[account(zero_copy)]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let values = local_types::visible_typed_values_at_with_workspace(
        &document,
        position_after(source, "position.lo"),
        None,
    );

    assert!(
        values.iter().any(|value| {
            value.name == "position" && value.type_name == "AccountLoader<Position>"
        }),
        "expected AccountLoader local type, got {values:#?}"
    );
    let members = crate::account_members::resolved_struct_chain_completion_members(
        &document,
        None,
        "AccountLoader<Position>",
        &[],
    )
    .expect("AccountLoader member model");
    assert!(
        members
            .members
            .iter()
            .any(|member| member.name == "load()?"),
        "expected AccountLoader load member, got {members:#?}"
    );

    let items = completions(&document, position_after(source, "position.lo"))
        .expect("AccountLoader method completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"load()?"));
    assert!(labels.contains(&"load_mut()?"));
    assert!(labels.contains(&"load_init()?"));
    assert!(!labels.contains(&"position_mint"));
}

#[test]
fn completes_loaded_account_loader_data_members() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position: AccountLoader<'info, Position>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position = ctx.accounts.position.load()?;
    position.position_
}

#[account(zero_copy)]
pub struct Position {
    pub position_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "position.position_"))
        .expect("loaded AccountLoader data completions");

    assert!(items.iter().any(|item| item.label == "position_mint"));
}

#[test]
fn completes_loaded_account_loader_data_when_account_path_shares_prefix() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub y: AccountLoader<'info, Aa>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let s = ctx.accounts.y.load_mut()?;
    s.y
}

#[account(zero_copy)]
pub struct Aa {
    pub ya: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "    s.y"))
        .expect("loaded AccountLoader data completions");

    assert!(
        items.iter().any(|item| item.label == "ya"),
        "expected loaded account data member completion, got {items:#?}"
    );
}

proptest! {
    #[test]
    fn completes_generated_loaded_account_loader_members(
        account_field in rust_identifier(),
        alias in rust_identifier(),
        owner in rust_type_identifier(),
        field in rust_identifier(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(account_field != field);
        prop_assume!(alias != field);
        prop_assume!(field != "load" && field != "load_mut" && field != "load_init");
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: AccountLoader<'info, {owner}>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {alias} = ctx.accounts.{account_field}.load_mut()?;
    {alias}.{prefix}
}}

#[account(zero_copy)]
pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("    {alias}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated loaded AccountLoader member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated loaded AccountLoader field completion, got {items:#?}"
        );
    }

    #[test]
    fn hides_generated_account_loader_data_before_load(
        account_field in rust_identifier(),
        alias in rust_identifier(),
        owner in rust_type_identifier(),
        field in rust_identifier(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(account_field != field);
        prop_assume!(alias != field);
        prop_assume!(field != "load" && field != "load_mut" && field != "load_init");
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: AccountLoader<'info, {owner}>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {alias} = &ctx.accounts.{account_field};
    {alias}.{prefix}
}}

#[account(zero_copy)]
pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("    {alias}.{prefix}");

        let labels = completions(&document, position_after(&source, &completion_line))
            .unwrap_or_default()
            .into_iter()
            .map(|item| item.label)
            .collect::<Vec<_>>();

        prop_assert!(
            !labels.contains(&field),
            "direct AccountLoader access should not expose data fields, got {labels:#?}"
        );
    }
}
