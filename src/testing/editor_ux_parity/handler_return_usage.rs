use {
    super::position_after,
    crate::{completions, diagnostics, document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::{NumberOrString, Url},
};

#[test]
fn editor_ux_completes_members_from_associated_function_return() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let metadata = BundleMetadata::new()?;
    metadata.position_
}
"#,
    );
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/metadata.rs").unwrap(),
            r#"
use anchor_lang::prelude::*;

pub struct BundleMetadata {
    pub position_bundle_mint: Pubkey,
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

    let items = completions::completions_with_workspace(
        &document,
        position_after(document.source(), "metadata.position_"),
        Some(&workspace_index),
    )
    .expect("editor-visible associated function return completions");

    assert!(items
        .iter()
        .any(|item| item.label == "position_bundle_mint"));
}

#[test]
fn editor_ux_flags_unknown_handler_method() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.verify_bundel();
        Ok(())
    }
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

impl PositionBundle {
    pub fn verify_bundle(&self) -> bool {
        true
    }
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == "anchor-missing-account-reference"
            ) && diagnostic
                .message
                .contains("`bundle.verify_bundel()` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing editor-visible method diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unknown-handler-method")
    );
}
