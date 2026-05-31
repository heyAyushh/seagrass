use crate::{diagnostics, document::ParsedDocument};

#[test]
fn editor_ux_flags_unresolved_anchor_handler_call_identifier() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
        verify_bundel(bundle_index);
        Ok(())
    }
}

fn verify_bundle(bundle_index: u16) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct Close<'info> {
    pub authority: Signer<'info>,
}
"#,
    );

    let diagnostics = diagnostics::collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`verify_bundel` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing unresolved handler call diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unresolved-handler-identifier")
    );
    assert!(diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("candidates"))
        .and_then(|value| value.as_array())
        .is_some_and(|candidates| candidates
            .iter()
            .any(|candidate| candidate.as_str() == Some("verify_bundle"))));
}
