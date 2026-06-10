use {
    crate::{
        constraint_catalog,
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        evidence::{AccountSetEvidence, ConstraintEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::{Diagnostic, Position, Range},
};

pub(super) fn constraint_diagnostics(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(zero_mut_conflict_diagnostic(field, constraint));
    diagnostics.extend(init_system_account_diagnostic(field, constraint));
    diagnostics.extend(realloc_type_diagnostic(field, constraint));
    diagnostics.extend(close_type_diagnostic(field, constraint));
    diagnostics.extend(realloc_diagnostics(accounts, field, constraint));
    diagnostics.extend(close_diagnostics(field, constraint));
    diagnostics.extend(init_system_program_diagnostic(accounts, field, constraint));
    diagnostics.extend(init_payer_self_diagnostic(field, constraint));
    diagnostics.extend(payer_mutability_diagnostics(accounts, field, constraint));
    diagnostics
}
fn init_payer_self_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_init_like_constraint() {
        return None;
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    let payer_is_initialized_field = constraint
        .account_references()
        .into_iter()
        .any(|reference| reference.key == "payer" && reference.name == field.field.name);
    if !payer_is_initialized_field {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses itself as `payer`; Anchor cannot initialize the payer account as the program account being created.",
            field.field.name,
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": init_key,
            "anchorError": "TryingToInitPayerAsProgramAccount",
        })),
    ))
}
fn init_system_account_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_init_like_constraint() || field.type_name() != Some("SystemAccount") {
        return None;
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    Some(diagnostic_from_range(
        field
            .field
            .type_range
            .unwrap_or(field.field.selection_range),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `{init_key}` on `SystemAccount`; Anchor `SystemAccount` represents an existing system-owned account and cannot be initialized.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": init_key,
            "quickfix": "remove-conflicting-constraints",
            "remove": [init_key, "payer", "space"],
        })),
    ))
}
fn realloc_type_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_key("realloc") || supports_data_resize_or_close(field) {
        return None;
    }

    Some(diagnostic_from_range(
        field
            .field
            .type_range
            .unwrap_or(field.field.selection_range),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `realloc` on `{}`; Anchor only allows `realloc` on `Account`, `LazyAccount`, or `AccountLoader` fields.",
            field.field.name,
            field.type_name().unwrap_or("unknown")
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "realloc",
            "quickfix": "remove-conflicting-constraints",
            "remove": ["realloc", "realloc::payer", "realloc::zero"],
        })),
    ))
}
fn close_type_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_key("close") || supports_data_resize_or_close(field) {
        return None;
    }

    Some(diagnostic_from_range(
        field
            .field
            .type_range
            .unwrap_or(field.field.selection_range),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `close` on `{}`; Anchor only allows `close` on `Account`, `LazyAccount`, or `AccountLoader` fields.",
            field.field.name,
            field.type_name().unwrap_or("unknown")
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "close",
            "quickfix": "remove-conflicting-constraints",
            "remove": ["close"],
        })),
    ))
}
fn zero_mut_conflict_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_flag_or_key("zero") || !constraint.has_flag_or_key("mut") {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` combines `zero` with `mut`; Anchor `zero` already implies mutability.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "zero",
            "conflictsWith": "mut",
            "quickfix": "remove-conflicting-constraints",
            "remove": ["mut"],
            "parserRule": {
                "kind": "conflict",
                "message": "mut cannot be provided with zeroed",
                "sourceMethod": "build",
            },
        })),
    ))
}
fn supports_data_resize_or_close(field: &FieldEvidence<'_>) -> bool {
    matches!(
        field.type_name(),
        Some("Account" | "LazyAccount" | "AccountLoader")
    )
}
fn payer_mutability_diagnostics(
    accounts: &AccountSetEvidence<'_>,
    initialized_or_realloced_field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    constraint
        .account_references()
        .into_iter()
        .filter(|reference| {
            matches!(reference.key, "payer" | "realloc::payer")
                && reference.name != initialized_or_realloced_field.field.name
        })
        .filter_map(|reference| {
            if reference_uses_member_path(constraint, reference.key, reference.name) {
                return None;
            }
            let payer = accounts
                .fields()
                .iter()
                .find(|field| field.field.name == reference.name)?;
            if !initialized_or_realloced_field.is_optional() && payer.is_optional() {
                return Some(diagnostic_from_range(
                    payer.field.selection_range,
                    AnchorDiagnosticKind::AnchorConstraintShape,
                    format!(
                        "`{}` pays for required `{}` via `{}` but is optional; Anchor requires the payer account to be required.",
                        payer.field.name, initialized_or_realloced_field.field.name, reference.key
                    ),
                    Some(serde_json::json!({
                        "account": payer.field.name,
                        "constraint": reference.key,
                        "requiredByAccount": initialized_or_realloced_field.field.name,
                    })),
                ));
            }
            if payer.has_any_constraint(&["mut", "zero"]) {
                return None;
            }

            Some(diagnostic_from_range(
                payer.field.selection_range,
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` pays for `{}` via `{}` but is missing `#[account(mut)]`; Anchor moves lamports through payer accounts.",
                    payer.field.name, initialized_or_realloced_field.field.name, reference.key
                ),
                Some(serde_json::json!({
                    "account": payer.field.name,
                    "constraint": reference.key,
                    "missing": "mut",
                    "quickfix": "add-mut-constraint",
                    "usedByAccount": initialized_or_realloced_field.field.name,
                })),
            ))
        })
        .collect()
}

fn reference_uses_member_path(constraint: &ConstraintEvidence<'_>, key: &str, name: &str) -> bool {
    constraint.values_after_key(key).into_iter().any(|value| {
        let Some(rest) = value.trim_start().strip_prefix(name) else {
            return false;
        };
        rest.trim_start().starts_with('.')
    })
}

fn realloc_diagnostics(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    if !constraint.has_key("realloc") {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();

    let spec = match constraint_catalog::by_key("realloc") {
        Some(spec) => spec,
        None => return diagnostics,
    };

    for companion in spec.required_companions {
        if constraint.has_flag_or_key(companion) {
            continue;
        }
        let msg = if *companion == "mut" {
            format!(
                "`{}` uses `realloc` but is not marked `mut`; realloc changes account data.",
                field.field.name
            )
        } else {
            format!(
                "`{}` uses `realloc` but is missing `{companion} = ...`.",
                field.field.name
            )
        };
        diagnostics.push(diagnostic_from_range(
            constraint.range(),
            AnchorDiagnosticKind::AnchorConstraintShape,
            msg,
            Some(serde_json::json!({
                "account": field.field.name,
                "constraint": "realloc",
                "missing": companion,
            })),
        ));
    }
    if let Some(diagnostic) = required_system_program_diagnostic(
        accounts,
        field,
        constraint,
        "realloc",
        !field.is_optional(),
    ) {
        diagnostics.push(diagnostic);
    }

    diagnostics
}
fn close_diagnostics(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    // Source-driven companion check for "close" + "mut".
    if !constraint.has_key("close")
        || constraint.has_flag_or_key("mut")
        || constraint.has_flag_or_key("zero")
    {
        return None;
    }
    // Still consult the catalog so that if the parser ever changes the declared companion for `close`,
    // the diagnostic will automatically reflect it (even though we currently keep the custom wording).
    if !constraint_catalog::requires_companion("close", "mut") {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `close` but is not marked `mut`; closing drains lamports and resets data.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "close",
            "missing": "mut",
        })),
    ))
}
fn init_system_program_diagnostic(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_init_like_constraint() {
        return None;
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    required_system_program_diagnostic(accounts, field, constraint, init_key, !field.is_optional())
}
fn required_system_program_diagnostic(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
    required_by: &str,
    requires_required_field: bool,
) -> Option<Diagnostic> {
    if let Some(system_program) = accounts.system_program_field() {
        if system_program.type_name() == Some("Program")
            && system_program.has_generic_type("System")
        {
            if !requires_required_field || !system_program.is_optional() {
                return None;
            }

            return Some(diagnostic_from_range(
                system_program.field.selection_range,
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`system_program` is optional but `{}` is required; Anchor requires system program accounts for required `{required_by}` constraints to be required.",
                    field.field.name,
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "accountsStruct": accounts.accounts.name,
                    "constraint": required_by,
                    "field": "system_program",
                })),
            ));
        }

        return Some(diagnostic_from_range(
            system_program_range(system_program),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`system_program` must be typed `Program<'info, System>` because `{}` uses `{required_by}`.",
                field.field.name,
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "accountsStruct": accounts.accounts.name,
                "constraint": required_by,
                "quickfix": "system-program-type",
                "expected": "Program<'info, System>",
            })),
        ));
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `{required_by}` but `{}` has no `Program<'info, System>` field named `system_program`.",
            field.field.name, accounts.accounts.name,
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "accountsStruct": accounts.accounts.name,
            "constraint": required_by,
            "missing": "system_program",
        })),
    ))
}
fn system_program_range(field: &FieldEvidence<'_>) -> Range {
    let Some(type_range) = field.field.type_range else {
        return field.field.selection_range;
    };
    let generic_end = field
        .field
        .generic_type_ranges
        .last()
        .map(|range| Position {
            line: range.range.end.line,
            character: range.range.end.character.saturating_add(1),
        })
        .unwrap_or(type_range.end);

    Range {
        start: type_range.start,
        end: generic_end,
    }
}
