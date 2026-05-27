use {
    crate::{
        document::AccountConstraint, evidence::ConstraintEvidence, range::matching_word_ranges,
    },
    tower_lsp::lsp_types::{Location, Range},
};

pub(super) fn instruction_argument_ranges_in_constraint(
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

pub(super) fn instruction_argument_names_match(left: &str, right: &str) -> bool {
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

pub(super) fn push_unique_location(locations: &mut Vec<Location>, location: Location) {
    if !locations
        .iter()
        .any(|existing| existing.uri == location.uri && existing.range == location.range)
    {
        locations.push(location);
    }
}
