use {
    super::position_after,
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::Url,
};

#[test]
fn completes_workspace_typed_handler_local_members_after_dot() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>, bundle: PositionBundle) -> Result<()> {
    bundle.
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
        position_after(source, "bundle."),
        Some(&index),
    )
    .expect("workspace typed handler member completions after dot");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_bundle_mint"));
    assert!(labels.contains(&"position_bitmap"));
}
