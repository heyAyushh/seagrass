use {
    crate::{diagnostics, document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::Url,
};

#[test]
fn editor_ux_flags_unknown_member_on_boxed_account_alias() {
    let document = ParsedDocument::parse_or_empty(
        r#"
#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>) -> Result<()> {
        let position_bundle = &mut ctx.accounts.position_bundle;
        position_bundle.s.s;
        Ok(())
    }
}
"#,
    );
    let workspace_index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/close_accounts.rs").unwrap(),
            r#"
#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Box<Account<'info, PositionBundle>>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#
            .to_string(),
        )],
    );

    let diagnostics = diagnostics::collect_with_workspace(&document, Some(&workspace_index));
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`position_bundle.s` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing boxed account alias diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `s`"));
}
