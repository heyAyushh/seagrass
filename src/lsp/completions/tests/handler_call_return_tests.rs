use {
    super::{completions, position_after},
    crate::{
        document::ParsedDocument, lsp::completions::proptest_support::rust_identifier,
        workspace::WorkspaceIndex,
    },
    proptest::prelude::*,
    tower_lsp::lsp_types::Url,
};

#[test]
fn completes_members_from_helper_return_value() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = load_bundle();
    bundle.position_
}

fn load_bundle() -> PositionBundle {
    PositionBundle {
        position_bundle_mint: Pubkey::default(),
        position_bitmap: [0; 32],
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.position_"))
        .expect("helper return member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

#[test]
fn completes_members_from_question_mark_result_helper() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = load_bundle()?;
    bundle.position_
}

fn load_bundle() -> Result<PositionBundle> {
    Ok(PositionBundle {
        position_bundle_mint: Pubkey::default(),
    })
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.position_"))
        .expect("question-mark helper return member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn does_not_unwrap_result_helper_without_question_mark() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = load_bundle();
    bundle.position_
}

fn load_bundle() -> Result<PositionBundle> {
    Ok(PositionBundle {
        position_bundle_mint: Pubkey::default(),
    })
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.position_"));

    assert!(
        items.is_none_or(|items| items
            .iter()
            .all(|item| item.label != "position_bundle_mint")),
        "Result<T> should not expose T members without `?`"
    );
}

#[test]
fn resolves_qualified_helper_return_without_same_name_guessing() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = helpers::load_bundle()?;
    bundle.position_
}

mod helpers {
    use super::*;

    pub fn load_bundle() -> Result<PositionBundle> {
        unreachable!()
    }
}

mod fixtures {
    use super::*;

    pub fn load_bundle() -> Result<OtherBundle> {
        unreachable!()
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub struct OtherBundle {
    pub position_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.position_"))
        .expect("qualified helper return member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(!labels.contains(&"position_owner"));
}

#[test]
fn completes_members_from_workspace_helper_return() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let bundle = load_bundle()?;
    bundle.position_
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/helpers.rs").unwrap(),
                r#"
use anchor_lang::prelude::*;

pub fn load_bundle() -> Result<PositionBundle> {
    unreachable!()
}
"#
                .to_string(),
            ),
            (
                Url::parse("file:///tmp/state.rs").unwrap(),
                r#"
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#
                .to_string(),
            ),
        ],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "bundle.position_"),
        Some(&index),
    )
    .expect("workspace helper return member completions");

    assert_eq!(items[0].label, "position_bundle_mint");
}

#[test]
fn completes_members_from_associated_function_return() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let metadata = BundleMetadata::new()?;
    metadata.asset_
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}

impl BundleMetadata {
    pub fn new() -> Result<Self> {
        unreachable!()
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "metadata.asset_"))
        .expect("associated function return member completions");

    assert_eq!(items[0].label, "asset_mint");
}

#[test]
fn completes_members_from_workspace_associated_function_return() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let metadata = BundleMetadata::new()?;
    metadata.asset_
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/metadata.rs").unwrap(),
            r#"
use anchor_lang::prelude::*;

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}

impl BundleMetadata {
    pub fn new() -> Result<Self> {
        unreachable!()
    }
}
"#
            .to_string(),
        )],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "metadata.asset_"),
        Some(&index),
    )
    .expect("workspace associated function return member completions");

    assert_eq!(items[0].label, "asset_mint");
}

proptest! {
    #[test]
    fn completes_generated_helper_return_members(
        local in rust_identifier(),
        helper in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(local != helper);
        prop_assume!(local != field);
        prop_assume!(helper != field);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {{
    let {local} = {helper}()?;
    {local}.{prefix}
}}

fn {helper}() -> Result<{owner}> {{
    unreachable!()
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{local}.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated helper return member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated helper-return field completion, got {items:#?}"
        );
    }
}
