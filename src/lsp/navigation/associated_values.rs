use {
    crate::{
        document::{is_generated_init_space_value, AssociatedValueKind, ParsedDocument},
        range::{byte_offset_at, matching_word_ranges, word_range_at_position},
    },
    tower_lsp::lsp_types::{Position, Range, SymbolKind},
};

const PATH_SEPARATOR: &str = "::";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssociatedPath {
    owner_type: String,
    value_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AssociatedValueRenameTarget {
    pub owner_type: String,
    pub value_name: String,
    pub range: Range,
}

pub(super) fn definition_range(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    target_at_position(document, word, position).map(|target| target.definition_range)
}

pub(super) fn definition_target_kinds(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Vec<SymbolKind>> {
    target_at_position(document, word, position).map(|target| vec![target.kind])
}

pub(super) fn reference_ranges(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Vec<Range>> {
    let target = target_at_position(document, word, position)?;
    let mut ranges = Vec::new();
    push_unique_range(&mut ranges, target.definition_range);

    for range in matching_word_ranges(document.source(), &target.value_name) {
        if associated_path_at_position(document.source(), &target.value_name, range.start)
            .is_some_and(|path| {
                path.owner_type == target.owner_type && path.value_name == target.value_name
            })
        {
            push_unique_range(&mut ranges, range);
        }
    }

    (!ranges.is_empty()).then_some(ranges)
}

pub(super) fn rename_target(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<AssociatedValueRenameTarget> {
    let target = target_at_position(document, word, position)?;
    (!target.is_generated).then_some(AssociatedValueRenameTarget {
        owner_type: target.owner_type,
        value_name: target.value_name,
        range: word_range_at_position(document.source(), position)?,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssociatedValueTarget {
    owner_type: String,
    value_name: String,
    definition_range: Range,
    kind: SymbolKind,
    is_generated: bool,
}

fn target_at_position(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<AssociatedValueTarget> {
    target_from_associated_path(document, word, position)
        .or_else(|| target_from_declaration(document, word, position))
}

fn target_from_associated_path(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<AssociatedValueTarget> {
    let path = associated_path_at_position(document.source(), word, position)?;
    resolved_target(document, &path.owner_type, &path.value_name)
}

fn target_from_declaration(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<AssociatedValueTarget> {
    document
        .symbols()
        .associated_value_items
        .iter()
        .find_map(|(owner_type, items)| {
            items.iter().find_map(|item| {
                (item.name == word && contains_position(item.range, position)).then(|| {
                    AssociatedValueTarget {
                        owner_type: owner_type.clone(),
                        value_name: item.name.clone(),
                        definition_range: item.range,
                        kind: associated_kind_to_symbol(item.kind),
                        is_generated: false,
                    }
                })
            })
        })
}

fn resolved_target(
    document: &ParsedDocument,
    owner_type: &str,
    value_name: &str,
) -> Option<AssociatedValueTarget> {
    if let Some(item) = document
        .symbols()
        .associated_value_items
        .get(owner_type)
        .and_then(|items| items.iter().find(|item| item.name == value_name))
    {
        return Some(AssociatedValueTarget {
            owner_type: owner_type.to_string(),
            value_name: item.name.clone(),
            definition_range: item.range,
            kind: associated_kind_to_symbol(item.kind),
            is_generated: false,
        });
    }

    generated_init_space_owner_range(document, owner_type, value_name).map(|definition_range| {
        AssociatedValueTarget {
            owner_type: owner_type.to_string(),
            value_name: value_name.to_string(),
            definition_range,
            kind: SymbolKind::CONSTANT,
            is_generated: true,
        }
    })
}

fn associated_kind_to_symbol(kind: AssociatedValueKind) -> SymbolKind {
    match kind {
        AssociatedValueKind::Constant => SymbolKind::CONSTANT,
        AssociatedValueKind::Function => SymbolKind::FUNCTION,
        AssociatedValueKind::Method => SymbolKind::METHOD,
    }
}

fn generated_init_space_owner_range(
    document: &ParsedDocument,
    owner_type: &str,
    value_name: &str,
) -> Option<Range> {
    (is_generated_init_space_value(value_name)
        && document
            .symbols()
            .derived_init_space_types
            .contains(owner_type))
    .then(|| {
        document
            .symbols()
            .all_structs
            .get(owner_type)
            .map(|symbol| symbol.selection_range)
    })
    .flatten()
}

fn associated_path_at_position(
    source: &str,
    word: &str,
    position: Position,
) -> Option<AssociatedPath> {
    let word_range = word_range_at_position(source, position)?;
    let word_offset = byte_offset_at(source, word_range.start)?;
    let current_word = source.get(word_offset..byte_offset_at(source, word_range.end)?)?;
    if current_word != word {
        return None;
    }

    let prefix = source.get(..word_offset)?;
    let owner_prefix = prefix.strip_suffix(PATH_SEPARATOR)?;
    let owner_type = last_path_segment(owner_prefix)?;
    Some(AssociatedPath {
        owner_type: owner_type.to_string(),
        value_name: word.to_string(),
    })
}

fn last_path_segment(path_prefix: &str) -> Option<&str> {
    let bytes = path_prefix.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_identifier_byte(bytes[start - 1]) {
        start -= 1;
    }

    (start < end).then(|| path_prefix.get(start..end)).flatten()
}

fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn push_unique_range(ranges: &mut Vec<Range>, range: Range) {
    if !ranges.contains(&range) {
        ranges.push(range);
    }
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || position.line == range.start.line && position.character >= range.start.character)
        && (position.line < range.end.line
            || position.line == range.end.line && position.character <= range.end.character)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_owner_type_from_qualified_associated_path() {
        let source = "space = crate::state::State::SPACE";
        let path = associated_path_at_position(
            source,
            "SPACE",
            Position {
                line: 0,
                character: 30,
            },
        )
        .unwrap();

        assert_eq!(
            path,
            AssociatedPath {
                owner_type: "State".to_string(),
                value_name: "SPACE".to_string()
            }
        );
    }
}
