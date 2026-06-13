use {
    super::*,
    crate::{
        anchor_types::{self, AnchorFieldCompletionKind},
        diagnostics::registry::{
            ANCHOR_SECURITY_CPI_PROGRAM_CODE, ANCHOR_SECURITY_OWNER_CHECK_CODE,
            ANCHOR_SECURITY_SIGNER_CODE, ANCHOR_SECURITY_SYSVAR_CODE,
            ANCHOR_SECURITY_TYPE_COSPLAY_CODE,
        },
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString, Url},
};

mod core_tests;
mod raw_account_tests;
mod signer_query_tests;

fn security_diagnostics(source: &str) -> Vec<Diagnostic> {
    let document = ParsedDocument::parse(source).unwrap();
    collect(&document)
}

fn assert_has_code<'a>(diagnostics: &'a [Diagnostic], code: &str) -> &'a Diagnostic {
    diagnostics_with_code(diagnostics, code)
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("expected diagnostic code {code}; diagnostics: {diagnostics:?}"))
}

fn assert_no_code(diagnostics: &[Diagnostic], code: &str) {
    assert!(
        diagnostics_with_code(diagnostics, code).is_empty(),
        "did not expect diagnostic code {code}; diagnostics: {diagnostics:?}"
    );
}

fn diagnostics_with_code<'a>(diagnostics: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(diagnostic_code)) if diagnostic_code == code
            )
        })
        .collect()
}
