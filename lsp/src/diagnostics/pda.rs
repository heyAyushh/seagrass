use {
    crate::{
        constraint_ranges::expression_range_in_constraint,
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        evidence::{AccountSetEvidence, EvidenceGraph, SeedExpressionEvidence, SeedExpressionKind},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Url},
};

const CURRENT_DOCUMENT_PLACEHOLDER_URI: &str = "file:///seagrass/current-document.rs";
const SEEDS_CONSTRAINT_KEY: &str = "seeds";

/// Gathers PDA seed resolution diagnostics for the given document.
///
/// Iterates over all account validation structs and their fields, checking
/// `seeds` constraints for expressions that cannot be IDL-serialized.
pub fn collect(document: &crate::document::ParsedDocument) -> Vec<Diagnostic> {
    EvidenceGraph::from_document(document)
        .account_sets()
        .iter()
        .flat_map(|accounts| {
            accounts
                .fields()
                .iter()
                .flat_map(move |field| field_diagnostics(document.source(), accounts, field))
        })
        .collect()
}

fn field_diagnostics(
    source: &str,
    accounts: &AccountSetEvidence<'_>,
    field: &crate::evidence::FieldEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for constraint in field.constraints() {
        if !constraint.has_key("seeds") {
            continue;
        }

        let seeds = field.seed_expressions(accounts, constraint);
        let mut constraint_diagnostics = Vec::with_capacity(seeds.len() * 2);

        push_seed_diagnostics(
            &mut constraint_diagnostics,
            source,
            field,
            constraint,
            &seeds,
        );
        push_mixed_seeds_diagnostic(
            &mut constraint_diagnostics,
            source,
            field,
            constraint,
            &seeds,
        );
        diagnostics.extend(constraint_diagnostics);
    }

    diagnostics
}

fn push_seed_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    source: &str,
    field: &crate::evidence::FieldEvidence<'_>,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    seeds: &[SeedExpressionEvidence],
) {
    for seed in seeds {
        if seed.kind == SeedExpressionKind::Expression {
            diagnostics.push(diagnostic_from_range_with_related(
                constraint.range(),
                AnchorDiagnosticKind::PdaSeedResolution,
                format!(
                    "`{}` PDA seed `{}` will be dropped from the IDL. Anchor's IDL generator only supports static bytes, account keys, and instruction arguments. Function calls and complex expressions are not serializable.",
                    field.field.name,
                    seed.expression
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": SEEDS_CONSTRAINT_KEY,
                    "seed": seed.expression,
                    "seedKind": "expression",
                    "idlVisible": false,
                })),
                seed_related_information(source, constraint, seed).map(|info| vec![info]),
            ));
        }
    }
}

fn push_mixed_seeds_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    source: &str,
    field: &crate::evidence::FieldEvidence<'_>,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    seeds: &[SeedExpressionEvidence],
) {
    let has_idl_dropped = seeds
        .iter()
        .any(|s| s.kind == SeedExpressionKind::Expression);
    let has_representable = seeds
        .iter()
        .any(|s| s.kind != SeedExpressionKind::Expression);

    if has_idl_dropped && has_representable {
        diagnostics.push(diagnostic_from_range_with_related(
            constraint.range(),
            AnchorDiagnosticKind::PdaSeedResolution,
            format!(
                "`{}` derives a PDA with mixed IDL-visible and IDL-invisible seeds. The IDL will only contain the representable seeds; clients will need manual derivation logic for the complex expressions.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "constraint": SEEDS_CONSTRAINT_KEY,
                "idlVisibleSeeds": seeds.iter().filter(|s| s.kind != SeedExpressionKind::Expression).map(|s| &s.expression).collect::<Vec<_>>(),
                "idlInvisibleSeeds": seeds.iter().filter(|s| s.kind == SeedExpressionKind::Expression).map(|s| &s.expression).collect::<Vec<_>>(),
            })),
            related_seed_information(source, constraint, seeds),
        ));
    }
}

fn related_seed_information(
    source: &str,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    seeds: &[SeedExpressionEvidence],
) -> Option<Vec<DiagnosticRelatedInformation>> {
    let related_information = seeds
        .iter()
        .filter_map(|seed| seed_related_information(source, constraint, seed))
        .collect::<Vec<_>>();

    (!related_information.is_empty()).then_some(related_information)
}

fn seed_related_information(
    source: &str,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    seed: &SeedExpressionEvidence,
) -> Option<DiagnosticRelatedInformation> {
    let range = expression_range_in_constraint(source, constraint.range(), &seed.expression)?;
    let message = match seed.kind {
        SeedExpressionKind::Expression => {
            format!("IDL-invisible seed `{}` is dropped here.", seed.expression)
        }
        SeedExpressionKind::StaticBytes
        | SeedExpressionKind::AccountKey
        | SeedExpressionKind::InstructionArgument => {
            format!(
                "IDL-visible seed `{}` remains in the generated IDL.",
                seed.expression
            )
        }
    };

    Some(DiagnosticRelatedInformation {
        location: Location {
            uri: current_document_uri(),
            range,
        },
        message,
    })
}

fn current_document_uri() -> Url {
    // Fixed placeholder URI is rebound to the real document URI by the diagnostic engine.
    Url::parse(CURRENT_DOCUMENT_PLACEHOLDER_URI)
        .expect("current document placeholder URI must stay a valid file URL")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::document::ParsedDocument,
        tower_lsp::lsp_types::{NumberOrString, Range},
    };

    #[test]
    fn reports_idl_dropped_seed_for_complex_expression() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", some_helper()], bump)]
    pub state: Account<'info, State>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(
            diagnostics.iter().any(|d| {
                d.code.as_ref().is_some_and(|c| matches!(c, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"))
                    && d.message.contains("some_helper()")
                    && d.message.contains("dropped from the IDL")
            }),
            "expected diagnostic for complex seed expression"
        );
    }

    #[test]
    fn reports_mixed_seeds_for_partially_representable_pda() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref(), some_helper()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(
            diagnostics.iter().any(|d| {
                d.code.as_ref().is_some_and(|c| matches!(c, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"))
                    && d.message.contains("mixed IDL-visible and IDL-invisible seeds")
            }),
            "expected mixed seeds diagnostic"
        );
    }

    #[test]
    fn no_diagnostic_for_fully_representable_seeds() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        assert!(
            diagnostics.iter().all(|d| {
                !d.code.as_ref().is_some_and(|c| matches!(c, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"))
            }),
            "expected no pda-seed-resolution diagnostic for fully representable seeds"
        );
    }

    #[test]
    fn diagnostic_includes_seed_data() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", max_key(&ctx.accounts.user)], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        let diagnostic = diagnostics
            .iter()
            .find(|d| {
                d.code.as_ref().is_some_and(|c| matches!(c, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"))
            })
            .expect("expected pda diagnostic");

        let data = diagnostic.data.as_ref().expect("expected data");
        assert_eq!(data["account"], "state");
        assert_eq!(data["constraint"], "seeds");
        assert_eq!(data["seedKind"], "expression");
        assert_eq!(data["idlVisible"], false);
    }

    #[test]
    fn idl_dropped_seed_diagnostic_links_exact_expression() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", max_key(&ctx.accounts.user)], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        let diagnostic = pda_diagnostic_with_message(&diagnostics, "dropped from the IDL");
        let related_information = diagnostic
            .related_information
            .as_ref()
            .expect("expected related information for complex seed expression");

        assert!(
            related_information.iter().any(|info| {
                info.message.contains("IDL-invisible seed")
                    && range_text(source, info.location.range) == "max_key(&ctx.accounts.user)"
            }),
            "expected related information to point at the exact complex seed expression; got {related_information:?}"
        );
    }

    #[test]
    fn mixed_seed_diagnostic_links_visible_and_invisible_seed_spans() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref(), some_helper()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let diagnostics = collect(&document);

        let diagnostic =
            pda_diagnostic_with_message(&diagnostics, "mixed IDL-visible and IDL-invisible seeds");
        let related_information = diagnostic
            .related_information
            .as_ref()
            .expect("expected related information for mixed PDA seeds");

        assert!(
            related_information.iter().any(|info| {
                info.message.contains("IDL-visible seed")
                    && range_text(source, info.location.range) == "user.key().as_ref()"
            }),
            "expected related information for the account-key seed; got {related_information:?}"
        );
        assert!(
            related_information.iter().any(|info| {
                info.message.contains("IDL-invisible seed")
                    && range_text(source, info.location.range) == "some_helper()"
            }),
            "expected related information for the dropped expression seed; got {related_information:?}"
        );
    }

    fn pda_diagnostic_with_message<'a>(
        diagnostics: &'a [Diagnostic],
        expected_message: &str,
    ) -> &'a Diagnostic {
        diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.message.contains(expected_message)
                    && diagnostic.code.as_ref().is_some_and(
                        |code| matches!(code, NumberOrString::String(code) if code == "anchor-pda-seed-resolution"),
                    )
            })
            .expect("expected pda diagnostic")
    }

    fn range_text(source: &str, range: Range) -> &str {
        let line = source
            .lines()
            .nth(range.start.line as usize)
            .expect("range line should exist in test fixture");
        &line[range.start.character as usize..range.end.character as usize]
    }
}
