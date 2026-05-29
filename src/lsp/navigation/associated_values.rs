use {
    crate::{
        document::{is_generated_init_space_value, AssociatedValueKind, ParsedDocument},
        range::{byte_offset_at, word_range_at_position},
    },
    tower_lsp::lsp_types::{Position, Range, SymbolKind},
};

const PATH_SEPARATOR: &str = "::";

#[derive(Debug, Clone, PartialEq, Eq)]
struct AssociatedPath {
    owner_type: String,
    value_name: String,
}

pub(super) fn definition_range(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Range> {
    let path = associated_path_at_position(document.source(), word, position)?;
    associated_value_range(document, &path.owner_type, &path.value_name)
}

pub(super) fn definition_target_kinds(
    document: &ParsedDocument,
    word: &str,
    position: Position,
) -> Option<Vec<SymbolKind>> {
    let path = associated_path_at_position(document.source(), word, position)?;
    associated_value_kind(document, &path.owner_type, &path.value_name).map(|kind| vec![kind])
}

fn associated_value_range(
    document: &ParsedDocument,
    owner_type: &str,
    value_name: &str,
) -> Option<Range> {
    document
        .symbols()
        .associated_value_items
        .get(owner_type)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.name == value_name)
                .map(|item| item.range)
        })
        .or_else(|| generated_init_space_owner_range(document, owner_type, value_name))
}

fn associated_value_kind(
    document: &ParsedDocument,
    owner_type: &str,
    value_name: &str,
) -> Option<SymbolKind> {
    document
        .symbols()
        .associated_value_items
        .get(owner_type)
        .and_then(|items| {
            items.iter().find_map(|item| {
                (item.name == value_name).then_some(match item.kind {
                    AssociatedValueKind::Constant => SymbolKind::CONSTANT,
                    AssociatedValueKind::Function => SymbolKind::FUNCTION,
                })
            })
        })
        .or_else(|| {
            generated_init_space_owner_range(document, owner_type, value_name)
                .map(|_| SymbolKind::CONSTANT)
        })
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
