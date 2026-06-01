use {
    super::{completions, position_after},
    crate::{
        document::ParsedDocument,
        lsp::completions::{proptest_support::rust_identifier, should_offer_completion},
        workspace::WorkspaceIndex,
    },
    proptest::prelude::*,
    tower_lsp::lsp_types::Url,
};

#[test]
fn completes_explicit_handler_local_struct_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle: PositionBundle = PositionBundle {
        position_bundle_mint: Pubkey::default(),
        position_bitmap: [0; 32],
    };
    bundle.position_bundle_m
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "bundle.position_bundle_m"),
    )
    .expect("typed handler local member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
    assert!(items[0]
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("PositionBundle")));
}

#[test]
fn completes_nested_explicit_handler_local_struct_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.inner.real_"))
        .expect("nested typed handler member completions");

    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_typed_handler_member_after_alias() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let alias = bundle;
    alias.position_bundle_m
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "alias.position_bundle_m"))
        .expect("typed handler alias member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn completes_typed_handler_member_after_field_alias() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let inner = bundle.inner;
    inner.real_
}

pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "inner.real_"))
        .expect("typed handler field alias member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_block_item_const_members_declared_after_cursor() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    LOCAL_BUNDLE.position_bundle_m

    const LOCAL_BUNDLE: PositionBundle = PositionBundle {
        position_bundle_mint: Pubkey::default(),
        position_bitmap: [0; 32],
    };
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "LOCAL_BUNDLE.position_bundle_m"),
    )
    .expect("block item const member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn completes_typed_handler_member_after_context_account_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let position_bundle = &mut ctx.accounts.position_bundle;
    position_bundle.position_
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "position_bundle.position_"),
    )
    .expect("typed context account alias member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn completes_account_data_members_after_accounts_alias_field() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub bundle_account: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let accounts = &mut ctx.accounts;
    accounts.bundle_account.asset_
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
    pub asset_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(
        &document,
        position_after(source, "accounts.bundle_account.asset_"),
    )
    .expect("account data completions through accounts alias field");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"asset_mint"));
    assert!(labels.contains(&"asset_owner"));
}

#[test]
fn completes_account_data_members_after_accounts_alias_account_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub bundle_account: Box<Account<'info, PositionBundle>>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let accounts = &mut ctx.accounts;
    let bundle = &mut accounts.bundle_account;
    bundle.asset_
}

#[account]
pub struct PositionBundle {
    pub asset_mint: Pubkey,
    pub asset_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.asset_"))
        .expect("account data completions through account alias from accounts alias");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"asset_mint"));
    assert!(labels.contains(&"asset_owner"));
}

#[test]
fn completes_context_bump_members_from_pda_fields() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    #[account(seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,
    pub payer: Signer<'info>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    ctx.bumps.st
}

#[account]
pub struct State {
    pub bump: u8,
}

#[account]
pub struct Vault {
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "ctx.bumps.st"))
        .expect("context bump member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"state"));
    assert!(!labels.contains(&"vault"));
    assert!(!labels.contains(&"payer"));
    assert!(items[0]
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("RunBumps")));
}

#[test]
fn completes_context_bump_members_after_alias() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bumps = ctx.bumps;
    bumps.st
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bumps.st"))
        .expect("context bump alias member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"state"));
    assert!(!labels.contains(&"payer"));
}

#[test]
fn completes_context_root_members() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    pub payer: Signer<'info>,
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    ctx.
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "ctx."))
        .expect("context root member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"accounts"));
    assert!(labels.contains(&"bumps"));
    assert!(labels.contains(&"program_id"));
    assert!(labels.contains(&"remaining_accounts"));
}

#[test]
fn completes_typed_handler_member_after_alias_while_dot_is_incomplete() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let alias = bundle;
    alias.
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#
            .to_string(),
        )],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "alias."),
        Some(&index),
    )
    .expect("typed handler alias member completions during incomplete dot access");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn completes_typed_handler_member_after_field_alias_while_dot_is_incomplete() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let inner = bundle.inner;
    inner.
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
pub struct PositionBundle {
    pub inner: InnerBundle,
}

pub struct InnerBundle {
    pub real_mint: Pubkey,
    pub real_owner: Pubkey,
}
"#
            .to_string(),
        )],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "inner."),
        Some(&index),
    )
    .expect("typed handler field alias member completions during incomplete dot access");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"real_mint"));
    assert!(labels.contains(&"real_owner"));
}

#[test]
fn completes_workspace_typed_handler_local_members() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.position_
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/state.rs").unwrap(),
            r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
            .to_string(),
        )],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "bundle.position_"),
        Some(&index),
    )
    .expect("workspace typed handler member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn completion_gate_wakes_for_typed_handler_member_dot() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.
}
"#;

    assert!(should_offer_completion(
        source,
        position_after(source, "bundle.")
    ));
}

#[test]
fn handler_member_completion_stays_quiet_for_untyped_locals() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = unknown();
    bundle.
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle."));

    assert!(
        items.is_none_or(|items| {
            items
                .iter()
                .all(|item| item.label != "position_bundle_mint")
        }),
        "untyped locals should not receive guessed struct members"
    );
}

proptest! {
    #[test]
    fn completes_generated_typed_handler_members(
        local in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(local != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
    {local}.{prefix}
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{local}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated typed handler member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated field completion, got {items:#?}"
        );
    }

    #[test]
    fn completes_generated_block_item_members_declared_after_cursor(
        declaration in prop_oneof![Just("const"), Just("static")],
        local in "[A-Z][A-Z0-9_]{1,10}",
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    {local}.{prefix}

    {declaration} {local}: {owner} = {owner} {{
        {field}: Pubkey::default(),
    }};
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{local}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated block item member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated block item field completion, got {items:#?}"
        );
    }

    #[test]
    fn completes_generated_typed_handler_alias_members(
        local in rust_identifier(),
        alias in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(local != alias);
        prop_assume!(local != field);
        prop_assume!(alias != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
    let {alias} = {local};
    {alias}.{prefix}
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{alias}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated typed handler alias member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated alias field completion, got {items:#?}"
        );
    }

    #[test]
    fn completes_generated_context_account_alias_members(
        account_field in rust_identifier(),
        alias in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(account_field != alias);
        prop_assume!(account_field != field);
        prop_assume!(alias != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        prop_assume!(!account_field.starts_with(&prefix));
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Box<Account<'info, {owner}>>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {alias} = &mut ctx.accounts.{account_field};
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
            .expect("generated context account alias member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated context alias field completion, got {items:#?}"
        );
    }

    #[test]
    fn completes_generated_accounts_alias_field_members(
        account_field in rust_identifier(),
        accounts_alias in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(account_field != accounts_alias);
        prop_assume!(account_field != field);
        prop_assume!(accounts_alias != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        prop_assume!(!account_field.starts_with(&prefix));
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    pub {account_field}: Box<Account<'info, {owner}>>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {accounts_alias} = &mut ctx.accounts;
    {accounts_alias}.{account_field}.{prefix}
}}

#[account]
pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{accounts_alias}.{account_field}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated accounts alias field member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated accounts alias field completion, got {items:#?}"
        );
    }

    #[test]
    fn completes_generated_context_bump_members(
        account_field in rust_identifier(),
        non_pda_field in rust_identifier(),
    ) {
        prop_assume!(account_field != non_pda_field);
        let prefix = account_field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(seeds = [b"generated"], bump)]
    pub {account_field}: Account<'info, GeneratedState>,
    pub {non_pda_field}: Signer<'info>,
}}

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    ctx.bumps.{prefix}
}}

#[account]
pub struct GeneratedState {{
    pub value: u64,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("ctx.bumps.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated context bump completions");

        prop_assert!(
            items.iter().any(|item| item.label == account_field),
            "expected generated bump field completion, got {items:#?}"
        );
        prop_assert!(
            items.iter().all(|item| item.label != non_pda_field),
            "non-PDA account field should not be offered as a bump, got {items:#?}"
        );
    }
}
