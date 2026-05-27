use {
    crate::{constraint_catalog, range::line_at},
    tower_lsp::lsp_types::{Position, Range},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintKeyRange {
    pub key: &'static str,
    pub range: Range,
}

pub fn constraint_key_ranges(source: &str, attribute_range: Range) -> Vec<ConstraintKeyRange> {
    let keys = constraint_keys_by_specificity();
    let mut ranges = Vec::new();
    for line_number in attribute_range.start.line..=attribute_range.end.line {
        let Some(line) = line_at(source, line_number) else {
            continue;
        };
        for key in &keys {
            for range in key_ranges_on_line(line, line_number, key) {
                if contains_position(attribute_range, range.start)
                    && contains_position(attribute_range, range.end)
                    && !overlaps_existing(&ranges, range)
                {
                    ranges.push(ConstraintKeyRange { key, range });
                }
            }
        }
    }
    ranges
}

pub fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

pub fn expression_range_in_constraint(
    source: &str,
    constraint_range: Range,
    expression: &str,
) -> Option<Range> {
    if expression.is_empty() {
        return None;
    }

    for line_number in constraint_range.start.line..=constraint_range.end.line {
        let Some(line) = line_at(source, line_number) else {
            continue;
        };
        let mut search_start = 0usize;
        while let Some(relative_start) = line[search_start..].find(expression) {
            let start = search_start + relative_start;
            let end = start + expression.len();
            let start_character = u32::try_from(start).ok()?;
            let end_character = u32::try_from(end).ok()?;

            if line_number == constraint_range.start.line
                && start_character < constraint_range.start.character
            {
                search_start = end;
                continue;
            }
            if line_number == constraint_range.end.line
                && end_character > constraint_range.end.character
            {
                search_start = end;
                continue;
            }

            return Some(Range {
                start: Position {
                    line: line_number,
                    character: start_character,
                },
                end: Position {
                    line: line_number,
                    character: end_character,
                },
            });
        }
    }

    None
}

fn constraint_keys_by_specificity() -> Vec<&'static str> {
    let mut keys = constraint_catalog::CONSTRAINTS
        .iter()
        .map(|spec| constraint_catalog::key(spec.label))
        .collect::<Vec<_>>();
    keys.sort_by_key(|key| std::cmp::Reverse(key.len()));
    keys
}

fn key_ranges_on_line(line: &str, line_number: u32, key: &str) -> Vec<Range> {
    let mut ranges = Vec::new();
    let mut search_start = 0usize;
    while let Some(relative) = line[search_start..].find(key) {
        let start = search_start + relative;
        let end = start + key.len();
        if has_key_boundaries(line, start, end) {
            ranges.push(Range {
                start: Position {
                    line: line_number,
                    character: u32::try_from(start).unwrap_or_default(),
                },
                end: Position {
                    line: line_number,
                    character: u32::try_from(end).unwrap_or_default(),
                },
            });
        }
        search_start = end;
    }
    ranges
}

fn overlaps_existing(existing: &[ConstraintKeyRange], candidate: Range) -> bool {
    existing.iter().any(|existing| {
        existing.range.start.line == candidate.start.line
            && candidate.start.character < existing.range.end.character
            && existing.range.start.character < candidate.end.character
    })
}

fn has_key_boundaries(line: &str, start: usize, end: usize) -> bool {
    line[..start]
        .chars()
        .next_back()
        .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        && line[end..]
            .chars()
            .next()
            .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_key_ranges_prefer_namespaced_keys() {
        let source = "#[account(init, token::mint = mint, mint::decimals = decimals)]";
        let range = Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: u32::try_from(source.len()).unwrap(),
            },
        };

        let keys = constraint_key_ranges(source, range)
            .into_iter()
            .map(|range| range.key)
            .collect::<Vec<_>>();

        assert!(keys.contains(&"init"));
        assert!(keys.contains(&"token::mint"));
        assert!(keys.contains(&"mint::decimals"));
        assert!(!keys.contains(&"mint"));
        assert!(!keys.contains(&"decimals"));
    }

    #[test]
    fn expression_range_in_constraint_finds_value_inside_attribute_bounds() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b"state", max_key(&ctx.accounts.user)], bump)]
    pub state: Account<'info, State>,
}
"#;
        let line = source
            .lines()
            .position(|line| line.contains("#[account("))
            .expect("fixture should contain account attribute") as u32;
        let line_text = line_at(source, line).expect("fixture line should exist");
        let range = Range {
            start: Position { line, character: 4 },
            end: Position {
                line,
                character: u32::try_from(line_text.len()).unwrap(),
            },
        };

        let expression_range =
            expression_range_in_constraint(source, range, "max_key(&ctx.accounts.user)")
                .expect("expected expression range");

        assert_eq!(
            &line_text[expression_range.start.character as usize
                ..expression_range.end.character as usize],
            "max_key(&ctx.accounts.user)"
        );
    }
}
