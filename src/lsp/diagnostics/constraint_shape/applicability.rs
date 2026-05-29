use {
    crate::{
        account_semantics::{self, ResolvedAccountType},
        constraint_catalog::{self, ConstraintFamily},
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        evidence::{ConstraintEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::Diagnostic,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpectedAccountKind {
    Mint,
    TokenAccount,
}

pub(super) fn constraint_diagnostics(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let declared_type = account_semantics::resolve_declared_field_account_type(field.field);
    if declared_type == ResolvedAccountType::Unknown {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let mut reported = Vec::new();
    for spec in constraint_catalog::CONSTRAINTS {
        let key = constraint_catalog::key(spec.label);
        if !constraint.has_flag_or_key(key) || declared_type.allows_constraint_family(spec.family) {
            continue;
        }

        let Some(expected) = expected_account_kind(spec.family) else {
            continue;
        };
        if reported.contains(&expected) {
            continue;
        }
        reported.push(expected);

        diagnostics.push(diagnostic_from_range(
            field
                .field
                .type_range
                .unwrap_or(field.field.selection_range),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{}` uses `{key}` but {} constraints require `{}`.",
                field.field.name,
                family_description(spec.family),
                expected_account_wrapper(field, expected),
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "constraint": key,
                "quickfix": "replace-account-type",
                "expected": expected_account_wrapper(field, expected),
                "family": family_slug(spec.family),
                "reason": "constraint-family-applicability",
            })),
        ));
    }

    diagnostics
}

fn expected_account_kind(family: ConstraintFamily) -> Option<ExpectedAccountKind> {
    match family {
        ConstraintFamily::Mint | ConstraintFamily::MintExtension => Some(ExpectedAccountKind::Mint),
        ConstraintFamily::TokenAccount | ConstraintFamily::AssociatedTokenAccount => {
            Some(ExpectedAccountKind::TokenAccount)
        }
        ConstraintFamily::Core | ConstraintFamily::Pda | ConstraintFamily::Realloc => None,
    }
}

fn expected_account_wrapper(field: &FieldEvidence<'_>, expected: ExpectedAccountKind) -> String {
    let generic = match expected {
        ExpectedAccountKind::Mint => "Mint",
        ExpectedAccountKind::TokenAccount => "TokenAccount",
    };
    match field.type_name() {
        Some("InterfaceAccount") => format!("InterfaceAccount<'info, {generic}>"),
        _ => format!("Account<'info, {generic}>"),
    }
}

fn family_description(family: ConstraintFamily) -> &'static str {
    match family {
        ConstraintFamily::Mint => "mint",
        ConstraintFamily::MintExtension => "Token-2022 mint extension",
        ConstraintFamily::TokenAccount => "token account",
        ConstraintFamily::AssociatedTokenAccount => "associated token account",
        ConstraintFamily::Core | ConstraintFamily::Pda | ConstraintFamily::Realloc => "account",
    }
}

fn family_slug(family: ConstraintFamily) -> &'static str {
    match family {
        ConstraintFamily::Core => "core",
        ConstraintFamily::Pda => "pda",
        ConstraintFamily::TokenAccount => "token-account",
        ConstraintFamily::AssociatedTokenAccount => "associated-token-account",
        ConstraintFamily::Mint => "mint",
        ConstraintFamily::MintExtension => "mint-extension",
        ConstraintFamily::Realloc => "realloc",
    }
}
