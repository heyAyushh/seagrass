use {
    crate::{
        actions, constraint_catalog,
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        evidence::{ConstraintEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::Diagnostic,
};

pub(super) fn diagnostics(
    document: &ParsedDocument,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = catalog_companion_diagnostics(field, constraint);
    diagnostics.extend(keyword_value_diagnostics(document, field, constraint));
    diagnostics
}
fn catalog_companion_diagnostics(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for spec in constraint_catalog::CONSTRAINTS {
        let key = constraint_catalog::key(spec.label);
        if !constraint.has_flag_or_key(key) {
            continue;
        }

        for missing in constraint_catalog::constraint_key_companions(spec) {
            if constraint.has_flag_or_key(missing) || companion_is_handled_elsewhere(key, missing) {
                continue;
            }

            diagnostics.push(diagnostic_from_range(
                constraint.range(),
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` uses `{key}` but is missing `{missing} = ...`.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": key,
                    "requiredBy": key,
                    "missing": missing,
                })),
            ));
        }
    }

    diagnostics
}
fn companion_is_handled_elsewhere(key: &str, missing: &str) -> bool {
    // Transitional: many "big" companions (realloc, close, seeds/bump) are still emitted via
    // custom diagnostic functions that produce richer messages and quickfix data. We keep them
    // in the exclusion list so the generic catalog path does not duplicate them.
    //
    // As more custom functions are converted to consult `constraint_catalog::requires_companion`,
    // entries can be removed from this list, shrinking the hand-maintained surface.
    matches!(
        (key, missing),
        ("init", "payer")
            | ("init", "space")
            | ("init", "system_program")
            | ("init_if_needed", "payer")
            | ("init_if_needed", "space")
            | ("init_if_needed", "system_program")
            | ("realloc", "mut")
            | ("realloc", "realloc::payer")
            | ("realloc", "realloc::zero")
            | ("close", "mut")
            | ("seeds", "bump")
            | ("bump", "seeds")
            | ("seeds::program", "seeds")
    )
}
fn keyword_value_diagnostics(
    document: &ParsedDocument,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for spec in constraint_catalog::CONSTRAINTS {
        if spec.value_kind != constraint_catalog::ConstraintValueKind::Keyword {
            continue;
        }
        if spec.allowed_keyword_values.is_empty() {
            continue;
        }

        let key = constraint_catalog::key(spec.label);
        let Some(value) = constraint.simple_identifier_value(key) else {
            continue;
        };

        if spec.allowed_keyword_values.contains(&value) {
            continue;
        }

        let range = constraint
            .value_range(document.source(), key, value)
            .unwrap_or(constraint.range());

        let candidates = spec.allowed_keyword_values.to_vec();
        let _closest = candidates
            .iter()
            .min_by_key(|candidate| actions::edit_distance(candidate, value))
            .copied()
            .unwrap_or(candidates[0]);

        let candidates_formatted = candidates
            .iter()
            .map(|c| format!("`{c}`"))
            .collect::<Vec<_>>()
            .join(" or ");

        diagnostics.push(diagnostic_from_range(
            range,
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{}` uses `{key} = {value}`; Anchor only accepts {candidates_formatted}.",
                field.field.name,
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "constraint": key,
                "value": value,
                "candidates": candidates,
                "quickfix": "replace-keyword-value",
                "parserRule": {
                    "kind": "parser",
                    "message": format!("{key} must be either {candidates_formatted}"),
                    "sourceMethod": "parse_token",
                },
            })),
        ));
    }

    diagnostics
}
