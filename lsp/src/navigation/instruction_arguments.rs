use {
    crate::{
        document::{AccountConstraint, ParsedDocument},
        evidence::ConstraintEvidence,
        range::{matching_word_ranges, word_at_position},
    },
    tower_lsp::lsp_types::{Position, Range},
};

use super::{contains_position, InstructionArgumentTarget};

pub fn instruction_argument_target(
    document: &ParsedDocument,
    position: Position,
) -> Option<InstructionArgumentTarget> {
    for instruction in document.symbols().callable_functions() {
        let Some(context) = instruction.context.as_ref() else {
            continue;
        };
        if let Some(argument) = instruction
            .arguments
            .iter()
            .find(|argument| contains_position(argument.range, position))
        {
            return Some(InstructionArgumentTarget {
                context: context.name.clone(),
                name: argument.name.clone(),
                range: argument.range,
            });
        }
    }

    for accounts in document.symbols().accounts_structs.values() {
        if let Some(argument) = accounts
            .instruction_arguments
            .iter()
            .find(|argument| contains_position(argument.range, position))
        {
            return Some(InstructionArgumentTarget {
                context: accounts.name.clone(),
                name: argument.name.clone(),
                range: argument.range,
            });
        }

        for field in &accounts.fields {
            for constraint in &field.account_constraints {
                if let Some((range, name)) =
                    instruction_argument_range_at_position(document.source(), constraint, position)
                {
                    return Some(InstructionArgumentTarget {
                        context: accounts.name.clone(),
                        name,
                        range,
                    });
                }
            }
        }
    }

    None
}

pub(super) fn instruction_argument_definition_range(
    document: &ParsedDocument,
    position: Position,
) -> Option<Range> {
    let target = instruction_argument_target(document, position)?;

    document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == target.context)
        })
        .flat_map(|instruction| instruction.arguments.iter())
        .find(|argument| instruction_argument_names_match(&argument.name, &target.name))
        .map(|argument| argument.range)
        .or_else(|| {
            document
                .symbols()
                .callable_functions()
                .flat_map(|instruction| instruction.arguments.iter())
                .any(|argument| {
                    argument.range == target.range && contains_position(argument.range, position)
                })
                .then_some(target.range)
        })
}

pub(super) fn instruction_argument_reference_ranges(
    document: &ParsedDocument,
    position: Position,
) -> Option<Vec<Range>> {
    let target = instruction_argument_target(document, position)?;
    let mut ranges = Vec::new();

    for instruction in document.symbols().callable_functions() {
        if instruction
            .context
            .as_ref()
            .is_none_or(|context| context.name != target.context)
        {
            continue;
        }
        ranges.extend(
            instruction
                .arguments
                .iter()
                .filter(|argument| instruction_argument_names_match(&argument.name, &target.name))
                .map(|argument| argument.range),
        );
    }

    if let Some(accounts) = document.symbols().accounts_structs.get(&target.context) {
        ranges.extend(
            accounts
                .instruction_arguments
                .iter()
                .filter(|argument| instruction_argument_names_match(&argument.name, &target.name))
                .map(|argument| argument.range),
        );

        for field in &accounts.fields {
            for constraint in &field.account_constraints {
                ranges.extend(instruction_argument_ranges_in_constraint(
                    document.source(),
                    constraint,
                    &target.name,
                ));
            }
        }
    }

    let ranges = dedupe_ranges(ranges);
    (!ranges.is_empty()).then_some(ranges)
}

fn instruction_argument_range_at_position(
    source: &str,
    constraint: &AccountConstraint,
    position: Position,
) -> Option<(Range, String)> {
    let word = word_at_position(source, position)?;
    let evidence = ConstraintEvidence::new(constraint);
    for reference in evidence.instruction_argument_references() {
        let range = evidence.value_range(source, reference.key, reference.name)?;
        if contains_position(range, position) {
            return Some((range, reference.name.to_string()));
        }
    }

    for range in matching_word_ranges(source, &word) {
        if contains_position(range, position)
            && range_is_within(range, constraint.range)
            && looks_like_seed_argument(source, range)
        {
            return Some((range, word));
        }
    }

    None
}

fn instruction_argument_ranges_in_constraint(
    source: &str,
    constraint: &AccountConstraint,
    argument_name: &str,
) -> Vec<Range> {
    let evidence = ConstraintEvidence::new(constraint);
    let mut ranges = Vec::new();

    for reference in evidence.instruction_argument_references() {
        if instruction_argument_names_match(reference.name, argument_name) {
            if let Some(range) = evidence.value_range(source, reference.key, reference.name) {
                ranges.push(range);
            }
        }
    }

    for alias in instruction_argument_aliases(argument_name) {
        for range in matching_word_ranges(source, &alias) {
            if range_is_within(range, constraint.range) && looks_like_seed_argument(source, range) {
                ranges.push(range);
            }
        }
    }

    dedupe_ranges(ranges)
}

fn instruction_argument_aliases(argument_name: &str) -> Vec<String> {
    let mut aliases = vec![argument_name.to_string()];
    let trimmed = argument_name.trim_start_matches('_');
    if trimmed != argument_name && !trimmed.is_empty() {
        aliases.push(trimmed.to_string());
    }
    aliases
}

fn instruction_argument_names_match(left: &str, right: &str) -> bool {
    left == right || left.trim_start_matches('_') == right.trim_start_matches('_')
}

fn looks_like_seed_argument(source: &str, range: Range) -> bool {
    let Some(line) = source.lines().nth(range.start.line as usize) else {
        return false;
    };
    let Ok(end) = usize::try_from(range.end.character) else {
        return false;
    };
    let suffix = line
        .chars()
        .skip(end)
        .collect::<String>()
        .trim_start()
        .to_string();

    suffix.starts_with(".as_ref")
        || suffix.starts_with(".as_bytes")
        || suffix.starts_with(".to_le_bytes")
}

fn range_is_within(inner: Range, outer: Range) -> bool {
    (inner.start.line > outer.start.line
        || inner.start.line == outer.start.line && inner.start.character >= outer.start.character)
        && (inner.end.line < outer.end.line
            || inner.end.line == outer.end.line && inner.end.character <= outer.end.character)
}

fn dedupe_ranges(ranges: Vec<Range>) -> Vec<Range> {
    let mut deduped = Vec::new();
    for range in ranges {
        if !deduped.contains(&range) {
            deduped.push(range);
        }
    }
    deduped
}
