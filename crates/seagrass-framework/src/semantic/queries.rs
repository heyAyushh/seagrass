use {
    super::{
        AccountField, AccountType, CheckKind, ConstraintValue, PopulatedFields, SemanticModel,
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString},
};

const SECURITY_SIGNER_CODE: &str = "anchor-security-signer";
const DIAGNOSTIC_SOURCE: &str = "seagrass";

pub fn missing_signer_diagnostics(model: &SemanticModel) -> Vec<Diagnostic> {
    model
        .accounts_structs
        .iter()
        .flat_map(|accounts| {
            let Some(runtime_checks) = signer_checks_for_context(model, &accounts.inner.name)
            else {
                return Vec::new();
            };
            accounts
                .inner
                .fields
                .iter()
                .filter(|field| signer_authorization_required(field, &accounts.inner.fields))
                .filter(|field| !field_has_signer_constraint(field))
                .filter(|field| !runtime_checks.contains(field.name.as_str()))
                .map(signer_diagnostic)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn signer_checks_for_context<'a>(
    model: &'a SemanticModel,
    accounts_name: &str,
) -> Option<HashSet<&'a str>> {
    let instructions = model
        .instructions
        .iter()
        .filter(|instruction| instruction.inner.context_type.as_deref() == Some(accounts_name))
        .collect::<Vec<_>>();
    if instructions.is_empty()
        || instructions.iter().any(|instruction| {
            !instruction
                .populated_fields
                .has(PopulatedFields::SIGNER_CHECKS)
        })
    {
        return None;
    }
    Some(
        instructions
            .into_iter()
            .flat_map(|instruction| &instruction.inner.signer_checks)
            .filter(|check| check.kind == CheckKind::Signer)
            .filter_map(|check| check.subject_ref.as_deref())
            .collect(),
    )
}

fn signer_authorization_required(field: &AccountField, fields: &[AccountField]) -> bool {
    if !matches!(
        field.account_type,
        AccountType::RawAccountInfo | AccountType::UncheckedAccount
    ) {
        return false;
    }
    if fields.iter().any(field_requires_signer) {
        return field_requires_signer(field);
    }
    true
}

fn field_requires_signer(field: &AccountField) -> bool {
    field
        .constraints
        .iter()
        .any(|constraint| constraint.key == "requires_signer")
}

fn field_has_signer_constraint(field: &AccountField) -> bool {
    field.constraints.iter().any(|constraint| {
        constraint.key == "signer"
            || matches!(
                &constraint.value,
                ConstraintValue::AccountRef(value) | ConstraintValue::Expression(value)
                    if value.contains(".is_signer") || value == "is_signer"
            )
    })
}

fn signer_diagnostic(field: &AccountField) -> Diagnostic {
    Diagnostic {
        range: field.source_range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(SECURITY_SIGNER_CODE.to_string())),
        source: Some(DIAGNOSTIC_SOURCE.to_string()),
        message: format!(
            "`{}` is used as an unchecked signer but no signer constraint or runtime signer check is visible.",
            field.name
        ),
        data: Some(serde_json::json!({
            "account": field.name,
            "reason": "semantic-signer-query",
        })),
        ..Diagnostic::default()
    }
}
