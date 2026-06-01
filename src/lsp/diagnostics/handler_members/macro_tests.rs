use {super::collect_with_workspace, crate::document::ParsedDocument};

#[test]
fn reports_unknown_member_inside_require_macro() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>) -> Result<()> {
        let bundle = &ctx.accounts.position_bundle;
        require!(bundle.fake == Pubkey::default(), ErrorCode::BadBundle);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub enum ErrorCode {
    BadBundle,
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.fake` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing require! member diagnostic: {diagnostics:#?}"));

    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no field `fake`"));
}
