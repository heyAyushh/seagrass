use {
    crate::document::ParsedDocument,
    tower_lsp::lsp_types::{FoldingRange, FoldingRangeKind},
};

pub fn folding_ranges(document: &ParsedDocument) -> Vec<FoldingRange> {
    let mut ranges = document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .filter_map(|symbol| range(symbol.range, Some(FoldingRangeKind::Region)))
        .chain(
            document
                .symbols()
                .instructions
                .iter()
                .filter_map(|instruction| range(instruction.range, Some(FoldingRangeKind::Region))),
        )
        .chain(
            document
                .symbols()
                .accounts_structs
                .values()
                .flat_map(|accounts| accounts.fields.iter())
                .flat_map(|field| field.account_constraints.iter())
                .filter_map(|constraint| {
                    range_with_collapsed_text(
                        constraint.range,
                        Some(FoldingRangeKind::Region),
                        Some("#[account(...)]".to_string()),
                    )
                }),
        )
        .collect::<Vec<_>>();

    ranges.sort_by_key(|range| {
        (
            range.start_line,
            range.start_character.unwrap_or_default(),
            range.end_line,
            range.end_character.unwrap_or_default(),
        )
    });
    ranges.dedup_by_key(|range| {
        (
            range.start_line,
            range.start_character.unwrap_or_default(),
            range.end_line,
            range.end_character.unwrap_or_default(),
        )
    });
    ranges
}

fn range(
    source: tower_lsp::lsp_types::Range,
    kind: Option<FoldingRangeKind>,
) -> Option<FoldingRange> {
    range_with_collapsed_text(source, kind, None)
}

fn range_with_collapsed_text(
    source: tower_lsp::lsp_types::Range,
    kind: Option<FoldingRangeKind>,
    collapsed_text: Option<String>,
) -> Option<FoldingRange> {
    (source.end.line > source.start.line).then_some(FoldingRange {
        start_line: source.start.line,
        start_character: Some(source.start.character),
        end_line: source.end.line,
        end_character: Some(source.end.character),
        kind,
        collapsed_text,
    })
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            constraint_catalog::{self, ConstraintValueKind},
            document::ParsedDocument,
        },
    };

    #[test]
    fn folds_multiline_account_constraints() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + State::INIT_SPACE
    )]
    pub state: Account<'info, State>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let ranges = folding_ranges(&document);

        assert!(ranges.iter().any(|range| {
            range.start_line == 3
                && range.end_line == 7
                && range.collapsed_text.as_deref() == Some("#[account(...)]")
        }));
        assert!(ranges.iter().any(|range| {
            range.start_line == 1
                && range.end_line > 7
                && range.collapsed_text.is_none()
                && range.kind == Some(FoldingRangeKind::Region)
        }));
    }

    #[test]
    fn folds_every_generated_multiline_account_constraint() {
        for spec in constraint_catalog::CONSTRAINTS {
            let key = constraint_catalog::key(spec.label);
            let source = generated_constraint_source(spec);
            let document = ParsedDocument::parse(&source)
                .unwrap_or_else(|err| panic!("generated source for `{key}` should parse: {err}"));
            let ranges = folding_ranges(&document);

            let account_fold = ranges.iter().find(|range| {
                range.collapsed_text.as_deref() == Some("#[account(...)]")
                    && range_text(&source, range).contains(key)
            });

            assert!(
                account_fold.is_some(),
                "expected a multiline account fold for generated constraint `{key}`; got {ranges:?}"
            );
        }
    }

    fn generated_constraint_source(spec: &constraint_catalog::ConstraintSpec) -> String {
        format!(
            r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account(
        {}
    )]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}}
"#,
            sample_constraint_fragment(spec)
        )
    }

    fn sample_constraint_fragment(spec: &constraint_catalog::ConstraintSpec) -> String {
        let key = constraint_catalog::key(spec.label);
        match spec.value_kind {
            ConstraintValueKind::None => key.to_string(),
            ConstraintValueKind::AnyExpression => format!("{key} = user.key()"),
            ConstraintValueKind::AccountReference | ConstraintValueKind::SignerReference => {
                format!("{key} = user")
            }
            ConstraintValueKind::ProgramReference => format!("{key} = system_program"),
            ConstraintValueKind::InstructionArgument => format!("{key} = amount"),
            ConstraintValueKind::Keyword => format!("{key} = skip"),
            ConstraintValueKind::Boolean => format!("{key} = true"),
            ConstraintValueKind::Space => format!("{key} = 8 + State::INIT_SPACE"),
            ConstraintValueKind::Seeds => format!("{key} = [b\"state\", user.key().as_ref()]"),
        }
    }

    fn range_text(source: &str, range: &FoldingRange) -> String {
        source
            .lines()
            .enumerate()
            .filter(|(line, _)| {
                let line = u32::try_from(*line).unwrap_or(u32::MAX);
                range.start_line <= line && line <= range.end_line
            })
            .map(|(_, line)| line)
            .collect::<Vec<_>>()
            .join("\n")
    }
}
