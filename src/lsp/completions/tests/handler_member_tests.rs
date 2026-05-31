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
}
