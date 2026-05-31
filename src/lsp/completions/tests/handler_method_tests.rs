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
fn completes_typed_handler_methods_from_inherent_impl() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.ver
}

pub struct PositionBundle {
    pub asset_mint: Pubkey,
}

impl PositionBundle {
    pub fn verify_bundle(&self) -> bool {
        true
    }

    pub fn static_helper() -> bool {
        true
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    let items = completions(&document, position_after(source, "bundle.ver"))
        .expect("typed handler method completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"verify_bundle()"));
    assert!(!labels.contains(&"static_helper()"));
}

#[test]
fn completes_workspace_typed_handler_methods() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.ver
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

impl PositionBundle {
    pub fn verify_bundle(&self) -> bool {
        true
    }

    pub fn static_helper() -> bool {
        true
    }
}
"#
            .to_string(),
        )],
    );

    let items = crate::lsp::completions::completions_with_workspace(
        &document,
        position_after(source, "bundle.ver"),
        Some(&index),
    )
    .expect("workspace typed handler method completions");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"verify_bundle()"));
    assert!(!labels.contains(&"static_helper()"));
}

proptest! {
    #[test]
    fn completes_generated_typed_handler_methods(
        local in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        method in rust_identifier(),
        associated_fn in rust_identifier(),
    ) {
        prop_assume!(method != associated_fn);
        prop_assume!(local != method && local != associated_fn);
        let prefix = method.chars().next().unwrap_or_default().to_string();
        prop_assume!(!associated_fn.starts_with(&prefix));
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, {local}: {owner}) -> Result<()> {{
    {local}.{prefix}
}}

pub struct {owner} {{
    pub value: Pubkey,
}}

impl {owner} {{
    pub fn {method}(&self) -> bool {{
        true
    }}

    pub fn {associated_fn}() -> bool {{
        true
    }}
}}
"#
        );
        let document = ParsedDocument::parse_or_empty(&source);
        let completion_line = format!("{local}.{prefix}");
        let method_label = format!("{method}()");
        let associated_label = format!("{associated_fn}()");

        let items = completions(&document, position_after(&source, &completion_line))
            .expect("generated typed handler method completions");

        prop_assert!(
            items.iter().any(|item| item.label == method_label),
            "expected generated method completion, got {items:#?}"
        );
        prop_assert!(
            items.iter().all(|item| item.label != associated_label),
            "static associated functions should not appear in member completions, got {items:#?}"
        );
    }
}
