use {
    super::collect_with_workspace,
    crate::{
        diagnostics::registry::ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE, document::ParsedDocument,
    },
    tower_lsp::lsp_types::NumberOrString,
};

#[test]
fn reports_unknown_handler_struct_literal_field() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = PositionBundle {
        position_bundel_mint: Pubkey::default(),
    };
    Ok(())
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("position_bundel_mint"))
        .unwrap_or_else(|| panic!("missing struct literal field diagnostic: {diagnostics:#?}"));

    assert_eq!(
        diagnostic.code.as_ref(),
        Some(&NumberOrString::String(
            ANCHOR_MISSING_ACCOUNT_REFERENCE_CODE.to_string()
        ))
    );
    assert!(diagnostic
        .message
        .contains("`PositionBundle` has no struct literal field"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("reason"))
            .and_then(|value| value.as_str()),
        Some("unknown-struct-literal-field")
    );
    assert_eq!(
        diagnostic
            .code_description
            .as_ref()
            .map(|description| description.href.as_str()),
        Some("https://doc.rust-lang.org/reference/expressions/struct-expr.html")
    );
}

#[test]
fn accepts_known_handler_struct_literal_fields_and_shorthand() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}

pub fn handler(ctx: Context<Run>, position_bitmap: [u8; 32]) -> Result<()> {
    let _bundle = PositionBundle {
        position_bundle_mint: Pubkey::default(),
        position_bitmap,
    };
    Ok(())
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.is_empty(),
        "known struct literal fields should not diagnose: {diagnostics:#?}"
    );
}

#[test]
fn ignores_unknown_external_struct_literal_type() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

pub fn handler(ctx: Context<Run>) -> Result<()> {
    let _bundle = ExternalBundle {
        position_bundel_mint: Pubkey::default(),
    };
    Ok(())
}
"#,
    )
    .unwrap();

    let diagnostics = collect_with_workspace(&document, None);

    assert!(
        diagnostics.is_empty(),
        "unknown external struct literal types stay outside shallow resolver: {diagnostics:#?}"
    );
}
