use {
    super::position_after,
    crate::{completions, document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::Url,
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
