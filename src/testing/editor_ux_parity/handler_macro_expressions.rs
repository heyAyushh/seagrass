use crate::{diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_flags_unknown_handler_values_inside_require_macro() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        let position_bundle = &ctx.accounts.position_bundle;
        require!(
            position_bundle.fake == missing_mint,
            ErrorCode::BadBundle
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub position_bundle: Account<'info, PositionBundle>,
}

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}

pub enum ErrorCode {
    BadBundle,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`missing_mint` does not resolve")),
        "editor diagnostics should flag unresolved require! values: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`position_bundle.fake` does not resolve")),
        "editor diagnostics should flag unknown require! members: {diagnostics:#?}"
    );
}
