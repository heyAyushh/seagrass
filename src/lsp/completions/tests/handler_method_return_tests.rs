use {
    super::{completions, position_after},
    crate::{document::ParsedDocument, lsp::completions::proptest_support::rust_identifier},
    proptest::prelude::*,
};

#[test]
fn completes_members_from_method_return_value() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let metadata = bundle.metadata();
    metadata.asset_
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> BundleMetadata {
        BundleMetadata {
            asset_mint: Pubkey::default(),
            asset_owner: Pubkey::default(),
        }
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
    pub asset_owner: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "metadata.asset_"))
        .expect("method return member completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"asset_mint"));
    assert!(labels.contains(&"asset_owner"));
}

#[test]
fn completes_members_from_question_mark_result_method() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let metadata = bundle.metadata()?;
    metadata.asset_
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "metadata.asset_"))
        .expect("question-mark method return member completions");

    assert_eq!(items[0].label, "asset_mint");
}

#[test]
fn does_not_unwrap_result_method_without_question_mark() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    let metadata = bundle.metadata();
    metadata.asset_
}

pub struct PositionBundle {
    pub value: Pubkey,
}

impl PositionBundle {
    pub fn metadata(&self) -> Result<BundleMetadata> {
        unreachable!()
    }
}

pub struct BundleMetadata {
    pub asset_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "metadata.asset_"));

    assert!(
        items.is_none_or(|items| items.iter().all(|item| item.label != "asset_mint")),
        "Result<T> method returns should not expose T members without `?`"
    );
}

proptest! {
    #[test]
    fn completes_generated_method_return_members(
        local in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        method in rust_identifier(),
        returned in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        prop_assume!(local != method);
        prop_assume!(local != field);
        prop_assume!(method != field);
        prop_assume!(owner != returned);
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
    let returned = {local}.{method}()?;
    returned.{prefix}
}}

pub struct {owner} {{
    pub value: Pubkey,
}}

impl {owner} {{
    pub fn {method}(&self) -> Result<{returned}> {{
        unreachable!()
    }}
}}

pub struct {returned} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("returned.{prefix}");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated method return member completions");

        prop_assert!(
            items.iter().any(|item| item.label == field),
            "expected generated method-return field completion, got {items:#?}"
        );
    }
}
