use {
    crate::{
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        document::{ParsedDocument, SymbolRange},
        evidence::{AccountSetEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::Diagnostic,
};

pub(super) fn field_diagnostic(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Option<Diagnostic> {
    if !field.is_unchecked_account()
        || !account_used_as_program(document, accounts.accounts, field.field)
        || field.has_any_constraint(&["address", "owner", "executable", "constraint"])
    {
        return None;
    }

    Some(diagnostic_from_range(
        field.field.type_range.unwrap_or(field.field.selection_range),
        AnchorDiagnosticKind::SecurityUncheckedAccount,
        format!(
            "`{}` is an unchecked program account; use `Program<'info, _>` or constrain its executable/address/owner.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "expected": "Program<'info, _>",
            "missing": "executable",
            "quickfix": "add-missing-constraint",
        })),
    ))
}
fn account_used_as_program(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> bool {
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .any(|instruction| {
            instruction
                .cpi_program_usages
                .iter()
                .any(|usage| usage.name == field.name)
        })
}
