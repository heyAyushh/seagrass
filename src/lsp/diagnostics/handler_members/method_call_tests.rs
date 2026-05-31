use {
    super::collect_with_workspace,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
    },
    tower_lsp::lsp_types::NumberOrString,
};

#[test]
fn reports_unknown_handler_method_on_typed_local() {
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

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic
                .message
                .contains("`bundle.verify_bundel()` does not resolve")
        })
        .unwrap_or_else(|| panic!("missing unknown method diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unknown-handler-method")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("candidates"))
            .and_then(|value| value.as_array())
            .map(|values| values
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>()),
        Some(vec!["verify_bundle"])
    );
}

#[test]
fn accepts_known_handler_method_on_typed_local() {
    let document = ParsedDocument::parse_or_empty(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, bundle: PositionBundle) -> Result<()> {
        bundle.verify_bundle();
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

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .contains("`bundle.verify_bundle()` does not resolve")
        }),
        "known handler method should resolve: {diagnostics:#?}"
    );
}
