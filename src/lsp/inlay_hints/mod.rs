use {
    crate::{
        constraint_ranges::expression_range_in_constraint,
        document::ParsedDocument,
        evidence::{EvidenceGraph, SeedExpressionKind},
    },
    tower_lsp::lsp_types::{InlayHint, InlayHintKind, InlayHintLabel, Position, Range},
};

/// Generates Anchor-specific inlay hints for a parsed document within the given range.
///
/// Analyzes `#[derive(Accounts)]` structs and their `#[account(...)]` constraints
/// to produce visual indicators in the editor, such as:
/// - Warnings when PDA seed expressions cannot be serialized to the Anchor IDL
/// - Implied `mut` annotations for accounts using `init`, `realloc`, `close`, or `zero`
/// - Missing required constraints (`payer`, `space`) for `init` accounts
///
/// # Examples
///
/// ```
/// use tower_lsp::lsp_types::{Position, Range};
/// # use crate::document::ParsedDocument;
///
/// # let source = r#"
/// # #[derive(Accounts)]
/// # pub struct Create<'info> {
/// #     #[account(init, payer = user, space = 8)]
/// #     pub state: Account<'info, State>,
/// #     pub user: Signer<'info>,
/// # }
/// # "#;
/// # let document = ParsedDocument::parse(source).unwrap();
/// let range = Range {
///     start: Position { line: 0, character: 0 },
///     end: Position { line: 100, character: 0 },
/// };
/// let hints = inlay_hints(&document, range);
/// assert!(!hints.is_empty());
/// ```
pub fn inlay_hints(document: &ParsedDocument, range: Range) -> Vec<InlayHint> {
    let graph = EvidenceGraph::from_document(document);
    let estimated = graph
        .account_sets()
        .iter()
        .map(|a| a.fields().len())
        .sum::<usize>()
        * 4;
    let mut hints = Vec::with_capacity(estimated);

    for accounts in graph.account_sets() {
        for field in accounts.fields() {
            for constraint in field.constraints() {
                if !constraint.has_key("seeds") {
                    continue;
                }
                hints.extend(collect_seed_hints(
                    document, accounts, field, constraint, range,
                ));
                hints.extend(collect_pda_invisible_hint(
                    document, accounts, field, constraint, range,
                ));
            }
            hints.extend(collect_implied_mut_hint(field, range));
            hints.extend(collect_missing_companion_hints(field, range));
        }
    }

    hints
}

fn collect_seed_hints(
    document: &ParsedDocument,
    accounts: &crate::evidence::AccountSetEvidence<'_>,
    field: &crate::evidence::FieldEvidence<'_>,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    range: Range,
) -> Vec<InlayHint> {
    let mut hints = Vec::new();
    let seeds = field.seed_expressions(accounts, constraint);

    for seed in &seeds {
        if seed.kind == SeedExpressionKind::Expression {
            if let Some(seed_range) = expression_range_in_constraint(
                document.source(),
                constraint.range(),
                &seed.expression,
            ) {
                if ranges_overlap(seed_range, range) {
                    hints.push(InlayHint {
                        position: Position {
                            line: seed_range.end.line,
                            character: seed_range.end.character,
                        },
                        label: InlayHintLabel::String(" ⚠️ IDL dropped".to_string()),
                        kind: Some(InlayHintKind::PARAMETER),
                        tooltip: Some(
                            tower_lsp::lsp_types::MarkupContent {
                                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                                value: "This seed expression contains a function call or complex expression that Anchor's IDL generator cannot serialize. The PDA will not be derivable from the IDL alone.".to_string(),
                            }
                            .into(),
                        ),
                        text_edits: None,
                        data: None,
                        padding_left: Some(true),
                        padding_right: None,
                    });
                }
            }
        }
    }

    hints
}

fn collect_pda_invisible_hint(
    document: &ParsedDocument,
    accounts: &crate::evidence::AccountSetEvidence<'_>,
    field: &crate::evidence::FieldEvidence<'_>,
    constraint: &crate::evidence::ConstraintEvidence<'_>,
    range: Range,
) -> Vec<InlayHint> {
    let seeds = field.seed_expressions(accounts, constraint);
    let has_idl_dropped_seeds = seeds
        .iter()
        .any(|s| s.kind == SeedExpressionKind::Expression);

    if !has_idl_dropped_seeds {
        return Vec::new();
    }

    if let Some(line_range) = constraint_line_range(document.source(), constraint.range()) {
        if ranges_overlap(line_range, range) {
            return vec![InlayHint {
                position: Position {
                    line: line_range.start.line,
                    character: line_range.start.character,
                },
                label: InlayHintLabel::String("⚠️ PDA invisible in IDL ".to_string()),
                kind: Some(InlayHintKind::TYPE),
                tooltip: Some(
                    tower_lsp::lsp_types::MarkupContent {
                        kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                        value: "This PDA uses seed expressions that cannot be represented in the Anchor IDL. Client-side derivation will require manual replication of the seed logic.".to_string(),
                    }
                    .into(),
                ),
                text_edits: None,
                data: None,
                padding_left: None,
                padding_right: Some(true),
            }];
        }
    }

    Vec::new()
}

fn collect_implied_mut_hint(
    field: &crate::evidence::FieldEvidence<'_>,
    range: Range,
) -> Vec<InlayHint> {
    if !field
        .constraints()
        .iter()
        .any(|constraint| constraint.implies_mutability())
    {
        return Vec::new();
    }

    if field.has_any_constraint(&["mut", "zero"]) {
        return Vec::new();
    }

    let Some(type_range) = field.field.type_range else {
        return Vec::new();
    };

    if !ranges_overlap(type_range, range) {
        return Vec::new();
    }

    vec![InlayHint {
        position: Position {
            line: type_range.end.line,
            character: type_range.end.character,
        },
        label: InlayHintLabel::String(" (mut)".to_string()),
        kind: Some(InlayHintKind::TYPE),
        tooltip: Some(
            tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: format!(
                    "`{}` is effectively mutable because it uses `init`, `realloc`, `close`, or `zero`.",
                    field.field.name
                ),
            }
            .into(),
        ),
        text_edits: None,
        data: None,
        padding_left: Some(true),
        padding_right: None,
    }]
}

fn collect_missing_companion_hints(
    field: &crate::evidence::FieldEvidence<'_>,
    range: Range,
) -> Vec<InlayHint> {
    if !field.has_init_constraint() {
        return Vec::new();
    }

    let mut hints = Vec::new();
    let required = ["payer", "space"];

    for companion in &required {
        if !field.has_any_constraint(&[*companion]) {
            let field_range = field.field.selection_range;
            if ranges_overlap(field_range, range) {
                hints.push(InlayHint {
                    position: Position {
                        line: field_range.end.line,
                        character: field_range.end.character,
                    },
                    label: InlayHintLabel::String(format!(" missing: {companion}")),
                    kind: Some(InlayHintKind::PARAMETER),
                    tooltip: Some(
                        tower_lsp::lsp_types::MarkupContent {
                            kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                            value: format!("`init` requires `{companion} = ...` to be specified."),
                        }
                        .into(),
                    ),
                    text_edits: None,
                    data: None,
                    padding_left: Some(true),
                    padding_right: None,
                });
            }
        }
    }

    hints
}

/// Returns a zero-width range at the start of the `#[account(` line.
///
/// Used to position inlay hints at the beginning of an account constraint
/// attribute line.
fn constraint_line_range(source: &str, constraint_range: Range) -> Option<Range> {
    let line = source.lines().nth(constraint_range.start.line as usize)?;

    let start_char = line.find("#[account(")? as u32 + "#[account(".len() as u32;

    Some(Range {
        start: Position {
            line: constraint_range.start.line,
            character: start_char,
        },
        end: Position {
            line: constraint_range.start.line,
            character: start_char,
        },
    })
}

/// Checks whether two LSP ranges overlap.
///
/// Two ranges overlap if the start of each is less than or equal to the
/// end of the other.
fn ranges_overlap(left: Range, right: Range) -> bool {
    position_le(left.start, right.end) && position_le(right.start, left.end)
}

/// Compares two LSP positions, returning true if `left` is less than or equal to `right`.
///
/// Ordering is lexicographic by line first, then by character.
fn position_le(left: Position, right: Position) -> bool {
    left.line < right.line || (left.line == right.line && left.character <= right.character)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::document::ParsedDocument};

    #[test]
    fn shows_idl_dropped_hint_for_complex_seed_expression() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", crate::ID.as_ref(), max_key(&ctx.accounts.user)], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hints = inlay_hints(
            &document,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 100,
                    character: 0,
                },
            },
        );

        assert!(
            hints
                .iter()
                .any(|h| label_contains(&h.label, "IDL dropped")),
            "expected IDL dropped hint for complex seed expression"
        );
    }

    #[test]
    fn shows_pda_invisible_hint_for_expression_seeds() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", some_helper()], bump)]
    pub state: Account<'info, State>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hints = inlay_hints(
            &document,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 100,
                    character: 0,
                },
            },
        );

        assert!(
            hints
                .iter()
                .any(|h| label_contains(&h.label, "invisible in IDL")),
            "expected PDA invisible hint"
        );
    }

    #[test]
    fn shows_implied_mut_hint_for_init_field() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hints = inlay_hints(
            &document,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 100,
                    character: 0,
                },
            },
        );

        assert!(
            hints.iter().any(|h| label_contains(&h.label, "(mut)")),
            "expected implied mut hint for init field"
        );
    }

    #[test]
    fn no_mut_hint_when_explicitly_marked_mut() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mut, init, payer = user, space = 8)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hints = inlay_hints(
            &document,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 100,
                    character: 0,
                },
            },
        );

        assert!(
            !hints.iter().any(|h| label_contains(&h.label, "(mut)")),
            "should not show implied mut when already marked mut"
        );
    }

    #[test]
    fn no_idl_dropped_hint_for_simple_seeds() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", user.key().as_ref()], bump)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let hints = inlay_hints(
            &document,
            Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 100,
                    character: 0,
                },
            },
        );

        assert!(
            !hints
                .iter()
                .any(|h| label_contains(&h.label, "IDL dropped")),
            "should not show IDL dropped for simple seeds"
        );
    }

    fn label_contains(label: &InlayHintLabel, needle: &str) -> bool {
        match label {
            InlayHintLabel::String(s) => s.contains(needle),
            InlayHintLabel::LabelParts(parts) => parts.iter().any(|p| p.value.contains(needle)),
        }
    }
}
