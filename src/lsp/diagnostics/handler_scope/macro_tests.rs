use {super::collect, crate::document::ParsedDocument, proptest::prelude::*};

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

#[test]
fn reports_unresolved_const_like_identifier_inside_require_macro() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    require!(bundle_index < MISSING_LIMIT, ErrorCode::BadBundle);
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
            .contains("`MISSING_LIMIT` does not resolve")),
        "missing unresolved const-like require! identifier diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn keeps_declared_const_like_identifier_resolved_inside_require_macro() {
    let diagnostics = collect(
        &ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;

const MAX_BUNDLE_INDEX: u16 = 64;

pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {
    require!(bundle_index < MAX_BUNDLE_INDEX, ErrorCode::BadBundle);
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
        diagnostics.iter().all(|diagnostic| !diagnostic
            .message
            .contains("`MAX_BUNDLE_INDEX` does not resolve")),
        "declared const-like require! identifier should stay quiet: {diagnostics:#?}"
    );
}

prop_compose! {
    fn generated_const_like_identifier()(tail in "[A-Z0-9_]{1,8}") -> String {
        format!("MISSING_{tail}")
    }
}

proptest! {
    #[test]
    fn reports_generated_const_like_identifier_inside_require_macro(
        missing in generated_const_like_identifier(),
    ) {
        let source = format!(
            r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>, bundle_index: u16) -> Result<()> {{
    require!(bundle_index < {missing}, ErrorCode::BadBundle);
    Ok(())
}}

pub enum ErrorCode {{
    BadBundle,
}}
"#
        );
        let diagnostics = collect(&ParsedDocument::parse(&source).unwrap());

        prop_assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains(&format!("`{missing}` does not resolve"))),
            "missing generated const-like require! identifier diagnostic: {diagnostics:#?}"
        );
    }
}
