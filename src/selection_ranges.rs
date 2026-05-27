use {
    crate::{
        constraint_ranges::{constraint_key_ranges, contains_position},
        document::{ParsedDocument, SymbolRange},
        range::word_range_at_position,
    },
    tower_lsp::lsp_types::{Position, Range, SelectionRange},
};

pub fn selection_ranges(document: &ParsedDocument, positions: &[Position]) -> Vec<SelectionRange> {
    positions
        .iter()
        .map(|position| selection_range(document, *position))
        .collect()
}

fn selection_range(document: &ParsedDocument, position: Position) -> SelectionRange {
    let word = word_range_at_position(document.source(), position).unwrap_or(Range {
        start: position,
        end: position,
    });
    let field = containing_field(document, position);
    let accounts = containing_accounts(document, position);
    let constraint = field.and_then(|field| containing_constraint(document, field, position));

    let mut ranges = vec![word];
    if let Some((key_range, attribute_range)) = constraint {
        push_unique_range(&mut ranges, key_range);
        push_unique_range(&mut ranges, attribute_range);
    }
    if let Some(field) = field {
        push_unique_range(&mut ranges, field.range);
    }
    if let Some(accounts) = accounts {
        push_unique_range(&mut ranges, accounts.range);
    }
    selection_range_chain(ranges)
}

fn selection_range_chain(ranges: Vec<Range>) -> SelectionRange {
    ranges
        .into_iter()
        .rev()
        .fold(None, |parent, range| {
            Some(Box::new(SelectionRange { range, parent }))
        })
        .map(|range| *range)
        .unwrap_or_else(|| SelectionRange {
            range: Range::default(),
            parent: None,
        })
}

fn containing_field(document: &ParsedDocument, position: Position) -> Option<&SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .chain(document.symbols().account_data_structs.values())
        .flat_map(|symbol| symbol.fields.iter())
        .find(|field| contains(field.range, position))
}

fn containing_accounts(document: &ParsedDocument, position: Position) -> Option<&SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .find(|accounts| contains(accounts.range, position))
}

fn containing_constraint(
    document: &ParsedDocument,
    field: &SymbolRange,
    position: Position,
) -> Option<(Range, Range)> {
    field.account_constraints.iter().find_map(|attribute| {
        if !contains(attribute.range, position) {
            return None;
        }
        let key_range = constraint_key_ranges(document.source(), attribute.range)
            .into_iter()
            .find(|key_range| contains_position(key_range.range, position))
            .map(|key_range| key_range.range)
            .unwrap_or(attribute.range);
        Some((key_range, attribute.range))
    })
}

fn push_unique_range(ranges: &mut Vec<Range>, range: Range) {
    if !ranges.contains(&range) {
        ranges.push(range);
    }
}

fn contains(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{constraint_catalog, range::word_range_at_position},
    };

    #[test]
    fn selection_range_expands_from_constraint_key_to_attribute_field_and_struct() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
"#;
        let document = ParsedDocument::parse(source).unwrap();
        let position = Position {
            line: 3,
            character: 16,
        };
        let selection = selection_range(&document, position);

        assert_eq!(
            selection.range,
            word_range_at_position(source, position).unwrap()
        );
        let attribute = selection.parent.as_ref().expect("attribute parent");
        assert_eq!(attribute.range.start.line, 3);
        assert_eq!(attribute.range.start.character, 4);
        let field = attribute.parent.as_ref().expect("field parent");
        assert_eq!(field.range.start.line, 3);
        assert_eq!(field.range.end.line, 4);
        let accounts = field.parent.as_ref().expect("accounts parent");
        assert_eq!(accounts.range.start.line, 1);
        assert_eq!(accounts.range.end.line, 5);
    }

    #[test]
    fn selection_range_expands_every_generated_constraint_key() {
        for spec in constraint_catalog::CONSTRAINTS {
            let key = constraint_catalog::key(spec.label);
            let source = generated_constraint_source(spec);
            let document = ParsedDocument::parse_or_empty(&source);
            let position = position_inside(&source, key);
            let selection = selection_range(&document, position);
            let ranges = selection_chain_ranges(&selection);
            let range_texts = ranges
                .iter()
                .filter_map(|range| range_text(&source, *range))
                .collect::<Vec<_>>();

            assert!(
                range_texts.iter().any(|text| text == key),
                "selection range for generated constraint `{key}` did not include the full key: {range_texts:?}"
            );
            assert!(
                range_texts
                    .iter()
                    .any(|text| text.trim_start().starts_with("#[account(")),
                "selection range for generated constraint `{key}` did not include the account attribute: {range_texts:?}"
            );
            assert!(
                range_texts.iter().any(|text| text.contains("pub state:")),
                "selection range for generated constraint `{key}` did not include the account field: {range_texts:?}"
            );
        }
    }

    fn generated_constraint_source(spec: &constraint_catalog::ConstraintSpec) -> String {
        format!(
            r#"
#[derive(Accounts)]
pub struct Create<'info> {{
    #[account({})]
    pub state: Account<'info, State>,
}}
"#,
            sample_constraint_fragment(spec)
        )
    }

    fn sample_constraint_fragment(spec: &constraint_catalog::ConstraintSpec) -> String {
        let key = constraint_catalog::key(spec.label);
        match spec.value_kind {
            constraint_catalog::ConstraintValueKind::None => key.to_string(),
            constraint_catalog::ConstraintValueKind::AnyExpression => format!("{key} = expr"),
            constraint_catalog::ConstraintValueKind::AccountReference => {
                format!("{key} = account")
            }
            constraint_catalog::ConstraintValueKind::SignerReference => format!("{key} = signer"),
            constraint_catalog::ConstraintValueKind::ProgramReference => {
                format!("{key} = program")
            }
            constraint_catalog::ConstraintValueKind::InstructionArgument => {
                format!("{key} = arg")
            }
            constraint_catalog::ConstraintValueKind::Keyword => format!("{key} = skip"),
            constraint_catalog::ConstraintValueKind::Boolean => format!("{key} = true"),
            constraint_catalog::ConstraintValueKind::Space => format!("{key} = 8"),
            constraint_catalog::ConstraintValueKind::Seeds => {
                format!("{key} = [b\"state\"]")
            }
        }
    }

    fn position_inside(source: &str, needle: &str) -> Position {
        let offset =
            source.find(needle).expect("needle in source") + needle.len().saturating_sub(1);
        let prefix = &source[..offset];
        let line = u32::try_from(prefix.bytes().filter(|byte| *byte == b'\n').count()).unwrap();
        let character = prefix
            .lines()
            .next_back()
            .map(|line| u32::try_from(line.chars().count()).unwrap())
            .unwrap_or(0);
        Position { line, character }
    }

    fn selection_chain_ranges(selection: &SelectionRange) -> Vec<Range> {
        let mut ranges = vec![selection.range];
        let mut parent = selection.parent.as_deref();
        while let Some(selection) = parent {
            ranges.push(selection.range);
            parent = selection.parent.as_deref();
        }
        ranges
    }

    fn range_text(source: &str, range: Range) -> Option<String> {
        if range.start.line == range.end.line {
            let line = source
                .lines()
                .nth(usize::try_from(range.start.line).ok()?)?;
            return Some(
                line.chars()
                    .skip(usize::try_from(range.start.character).ok()?)
                    .take(usize::try_from(range.end.character - range.start.character).ok()?)
                    .collect(),
            );
        }

        let lines = source.lines().collect::<Vec<_>>();
        let start_line = usize::try_from(range.start.line).ok()?;
        let end_line = usize::try_from(range.end.line).ok()?;
        Some(lines.get(start_line..=end_line)?.join("\n"))
    }
}
