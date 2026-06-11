use {
    super::*,
    tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString},
};

mod account_lifecycle;
mod instruction_bounds;
mod native_raw;
mod native_validation;
mod pda;
mod security_bundle;
mod unsafe_unwrap;

fn assert_no_attack(diagnostics: &[Diagnostic], attack: &str) {
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!(attack))
        }),
        "unexpected {attack}: {diagnostics:#?}"
    );
}

fn assert_has_attack(diagnostics: &[Diagnostic], attack: &str) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!(attack))
        }),
        "missing {attack}: {diagnostics:#?}"
    );
}
