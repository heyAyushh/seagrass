use crate::constraint_catalog;
use {
    super::*,
    crate::diagnostics::registry::{
        ANCHOR_CONSTRAINT_SHAPE_CODE, ANCHOR_SECURITY_STATIC_PDA_CODE,
        ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE,
    },
    crate::workspace::WorkspaceIndex,
    tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url},
};

mod catalog_tests;
mod has_one_tests;
mod init_lifecycle_tests;
mod pda_tests;
mod program_account_tests;
mod token_tests;

fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    collect(&document)
}
fn diagnostics_for_workspace(source: &str, account_data_source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [(
            Url::parse("file:///tmp/account-data.rs").unwrap(),
            account_data_source.to_string(),
        )],
    );
    collect_with_workspace(&document, Some(&index))
}
fn has_code(diagnostics: &[Diagnostic], code: &str) -> bool {
    diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(diagnostic_code)) if diagnostic_code == code
        )
    })
}
fn has_code_and_message(diagnostics: &[Diagnostic], code: &str, message: &str) -> bool {
    diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(diagnostic_code)) if diagnostic_code == code
        ) && diagnostic.message.contains(message)
    })
}
fn constraint_shape_diagnostic_with_message<'a>(
    diagnostics: &'a [Diagnostic],
    message: &str,
) -> &'a Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(diagnostic_code))
                    if diagnostic_code == ANCHOR_CONSTRAINT_SHAPE_CODE
            ) && diagnostic.message.contains(message)
        })
        .unwrap_or_else(|| panic!("expected constraint shape diagnostic containing {message:?}"))
}
fn assert_related_information(diagnostic: &Diagnostic, expected_messages: &[&str]) {
    let related_information = diagnostic
        .related_information
        .as_ref()
        .unwrap_or_else(|| panic!("expected related information for {diagnostic:?}"));
    for expected_message in expected_messages {
        assert!(
                related_information.iter().any(|info| {
                    info.message.contains(expected_message)
                        && info.location.uri.as_str() == CURRENT_DOCUMENT_PLACEHOLDER_URI
                }),
                "expected related information containing {expected_message:?}; got {related_information:?}"
            );
    }
}
