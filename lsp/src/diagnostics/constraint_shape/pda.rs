use {
    super::support::{constraint_related_information, field_related_information},
    crate::{
        constraint_catalog,
        diagnostics::{
            diagnostic_from_range, diagnostic_from_range_with_related,
            registry::AnchorDiagnosticKind,
        },
        document::{ParsedDocument, PdaSeeds},
        evidence::{ConstraintEvidence, FieldEvidence},
    },
    syn::{Expr, Lit},
    tower_lsp::lsp_types::Diagnostic,
};

pub(super) fn constraint_diagnostics(
    document: &ParsedDocument,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(seed_bump_pair_diagnostics(document, field, constraint));
    diagnostics.extend(seeds_program_diagnostic(field, constraint));
    diagnostics.extend(associated_token_seeds_conflict_diagnostic(
        field, constraint,
    ));
    diagnostics.extend(init_with_foreign_seeds_program_diagnostic(
        field, constraint,
    ));
    diagnostics
}

pub(super) fn field_diagnostics(field: &FieldEvidence<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(static_pda_seed_diagnostic(field));
    diagnostics.extend(pda_seed_count_diagnostic(field));
    diagnostics.extend(pda_seed_length_diagnostics(field));
    diagnostics
}
fn init_with_foreign_seeds_program_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_init_like_constraint() || !constraint.has_key("seeds::program") {
        return None;
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `{init_key}` with `seeds::program`; Anchor can only create PDAs signed by the current program.",
            field.field.name,
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "seeds::program",
            "anchorError": "ConstraintSeeds",
            "conflictsWith": init_key,
        })),
    ))
}
fn seed_bump_pair_diagnostics(
    document: &ParsedDocument,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    // Source-driven: seeds <-> bump relationship comes from the generated catalog.
    let has_seeds = constraint.has_key("seeds");
    let has_bump = constraint.has_flag_or_key("bump");

    match (has_seeds, has_bump) {
        (true, false) if constraint_catalog::requires_companion("seeds", "bump") => {
            Some(diagnostic_from_range_with_related(
                constraint.range(),
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` uses PDA seeds without `bump`; add `bump` so Anchor validates the canonical PDA bump.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": "seeds",
                })),
                Some(vec![
                    constraint_related_information(
                        document.source(),
                        constraint,
                        "seeds",
                        "`seeds` makes this account a PDA and requires a companion `bump`.".to_string(),
                    ),
                    field_related_information(
                        field,
                        format!("`{}` is the PDA account field using these seeds.", field.field.name),
                    ),
                ]),
            ))
        }
        (false, true) if constraint_catalog::requires_companion("bump", "seeds") => {
            Some(diagnostic_from_range_with_related(
                constraint.range(),
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` uses `bump` without `seeds`; add `seeds = [...]` or remove the bump constraint.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": "bump",
                })),
                Some(vec![
                    constraint_related_information(
                        document.source(),
                        constraint,
                        "bump",
                        "`bump` only validates a PDA when paired with `seeds = [...]`.".to_string(),
                    ),
                    field_related_information(
                        field,
                        format!("`{}` is the account field carrying the standalone bump.", field.field.name),
                    ),
                ]),
            ))
        }
        _ => None,
    }
}
fn seeds_program_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_key("seeds::program") || constraint.has_key("seeds") {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `seeds::program` but is missing `seeds = [...]`.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "seeds::program",
            "missing": "seeds",
        })),
    ))
}
fn associated_token_seeds_conflict_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    let uses_associated_token = constraint.has_key("associated_token::mint")
        || constraint.has_key("associated_token::authority")
        || constraint.has_key("associated_token::token_program");
    if !uses_associated_token || !constraint.has_key("seeds") {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` combines `associated_token::*` with `seeds`; Anchor associated token accounts are derived by the associated token program, not custom PDA seeds.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "associated_token",
            "conflictsWith": "seeds",
            "quickfix": "remove-conflicting-constraints",
            "remove": ["seeds", "bump", "seeds::program"],
        })),
    ))
}
fn static_pda_seed_diagnostic(field: &FieldEvidence<'_>) -> Option<Diagnostic> {
    let constraint = field
        .constraints()
        .iter()
        .find(|constraint| constraint.has_key("seeds"))?;
    if !constraint.has_flag_or_key("bump") || !field.seeds_are_static_only() {
        return None;
    }

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::SecurityStaticPda,
        format!(
            "`{}` derives a PDA from only static seeds; add an account or instruction seed if this state should be scoped.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "seeds",
        })),
    ))
}
fn pda_seed_count_diagnostic(field: &FieldEvidence<'_>) -> Option<Diagnostic> {
    let pda = field.pda()?;
    let PdaSeeds::List(seeds) = &pda.seeds else {
        return None;
    };
    if seeds.len() <= 16 {
        return None;
    }

    let range = field
        .constraints()
        .iter()
        .find(|constraint| constraint.has_key("seeds"))
        .map(ConstraintEvidence::range)
        .unwrap_or(field.field.selection_range);

    Some(diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` derives a PDA with {} seeds; Solana allows at most 16 seeds.",
            field.field.name,
            seeds.len()
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "seeds",
            "anchorError": "ConstraintSeeds",
            "seedCount": seeds.len(),
            "maxSeeds": 16,
        })),
    ))
}
fn pda_seed_length_diagnostics(field: &FieldEvidence<'_>) -> Vec<Diagnostic> {
    let Some(pda) = field.pda() else {
        return Vec::new();
    };
    let PdaSeeds::List(seeds) = &pda.seeds else {
        return Vec::new();
    };

    let range = field
        .constraints()
        .iter()
        .find(|constraint| constraint.has_key("seeds"))
        .map(ConstraintEvidence::range)
        .unwrap_or(field.field.selection_range);

    seeds
        .iter()
        .filter_map(|seed| {
            let byte_len = static_seed_byte_len(seed)?;
            (byte_len > 32).then(|| {
                diagnostic_from_range(
                    range,
                    AnchorDiagnosticKind::AnchorConstraintShape,
                    format!(
                        "`{}` derives a PDA with seed `{seed}` that is {byte_len} bytes; Solana allows at most 32 bytes per seed.",
                        field.field.name
                    ),
                    Some(serde_json::json!({
                        "account": field.field.name,
                        "constraint": "seeds",
                        "anchorError": "ConstraintSeeds",
                        "seed": seed,
                        "seedLength": byte_len,
                        "maxSeedLength": 32,
                    })),
                )
            })
        })
        .collect()
}
fn static_seed_byte_len(seed: &str) -> Option<usize> {
    let expr = syn::parse_str::<Expr>(seed).ok()?;
    static_seed_byte_len_from_expr(&expr)
}
fn static_seed_byte_len_from_expr(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::ByteStr(bytes) => Some(bytes.value().len()),
            Lit::Str(string) => Some(string.value().len()),
            _ => None,
        },
        Expr::MethodCall(method) if method.method == "as_ref" || method.method == "as_bytes" => {
            static_seed_byte_len_from_expr(&method.receiver)
        }
        Expr::Reference(reference) => static_seed_byte_len_from_expr(&reference.expr),
        Expr::Paren(paren) => static_seed_byte_len_from_expr(&paren.expr),
        _ => None,
    }
}
