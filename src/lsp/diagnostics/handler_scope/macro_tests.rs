use {super::collect, crate::document::ParsedDocument};

#[test]
fn reports_unresolved_identifier_inside_require_macro() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    let copied_index = bundle_index;
    require!(copied_index > missing_index, ErrorCode::BadBundle);
    Ok(())
}

pub enum ErrorCode {
    BadBundle,
}
"#,
        )
        .unwrap(),
    );

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`missing_index` does not resolve")),
        "missing unresolved require! identifier diagnostic: {diagnostics:#?}"
    );
    assert!(
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("`copied_index` does not resolve")),
        "resolved require! identifiers should stay quiet: {diagnostics:#?}"
    );
}
