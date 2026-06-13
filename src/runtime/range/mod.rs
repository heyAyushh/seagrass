use {
    proc_macro2::Span, tower_lsp::lsp_types::Position, tower_lsp::lsp_types::Range,
    tree_sitter::Point,
};

pub fn range_from_span(span: Span) -> Range {
    let start = span.start();
    let end = span.end();

    if start.line == 0 {
        return Range::default();
    }

    Range {
        start: Position {
            line: u32::try_from(start.line.saturating_sub(1)).unwrap_or_default(),
            character: u32::try_from(start.column).unwrap_or_default(),
        },
        end: Position {
            line: u32::try_from(end.line.saturating_sub(1)).unwrap_or_default(),
            character: u32::try_from(end.column).unwrap_or_default(),
        },
    }
}

pub fn is_in_account_attribute(text: &str, position: Position) -> bool {
    if is_in_comment_or_string(text, position) {
        return false;
    }

    let Some(offset) = offset_at(text, position) else {
        return false;
    };

    let prefix = &text[..offset];
    let Some(start) = prefix.rfind("#[account") else {
        return false;
    };

    let after_start = &prefix[start..];
    after_start.contains('(') && !after_start.contains(']')
}

pub fn is_in_comment_or_string(text: &str, position: Position) -> bool {
    let Some(cursor) = byte_offset_at(text, position) else {
        return false;
    };
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut state = LexicalState::Code;

    while index < cursor.min(bytes.len()) {
        match state {
            LexicalState::Code => {
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'/') {
                    state = LexicalState::LineComment;
                    index += 2;
                    continue;
                }
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    state = LexicalState::BlockComment { depth: 1 };
                    index += 2;
                    continue;
                }
                if bytes[index] == b'"' {
                    state = LexicalState::String { escaped: false };
                }
                index += 1;
            }
            LexicalState::LineComment => {
                if bytes[index] == b'\n' {
                    state = LexicalState::Code;
                }
                index += 1;
            }
            LexicalState::BlockComment { depth } => {
                if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
                    state = LexicalState::BlockComment { depth: depth + 1 };
                    index += 2;
                    continue;
                }
                if bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/') {
                    let next_depth = depth.saturating_sub(1);
                    state = if next_depth == 0 {
                        LexicalState::Code
                    } else {
                        LexicalState::BlockComment { depth: next_depth }
                    };
                    index += 2;
                    continue;
                }
                index += 1;
            }
            LexicalState::String { escaped } => {
                state = match (bytes[index], escaped) {
                    (_, true) => LexicalState::String { escaped: false },
                    (b'\\', false) => LexicalState::String { escaped: true },
                    (b'"', false) => LexicalState::Code,
                    _ => LexicalState::String { escaped: false },
                };
                index += 1;
            }
        }
    }

    !matches!(state, LexicalState::Code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LexicalState {
    Code,
    LineComment,
    BlockComment { depth: usize },
    String { escaped: bool },
}

pub fn account_type_after_line(text: &str, line_number: u32) -> Option<String> {
    let start = usize::try_from(line_number).ok()?.saturating_add(1);
    let field_line = text
        .lines()
        .skip(start)
        .find(|line| line.contains("Account<"))?;
    let start = field_line.find("Account<")?;
    let generic = &field_line[start..];
    let end = generic.find('>')?;
    let inner = &generic[..end];
    let ty = inner.rsplit_once(',')?.1.trim();

    (!ty.is_empty()).then_some(ty.to_string())
}

pub fn word_at_position(text: &str, position: Position) -> Option<String> {
    let range = word_range_at_position(text, position)?;
    let line = line_at(text, range.start.line)?;
    let start = usize::try_from(range.start.character).ok()?;
    let end = usize::try_from(range.end.character).ok()?;

    Some(line.chars().skip(start).take(end - start).collect())
}

pub fn matching_word_ranges(text: &str, word: &str) -> Vec<Range> {
    if word.is_empty() {
        return Vec::new();
    }

    text.lines()
        .enumerate()
        .flat_map(|(line_idx, line)| {
            let chars = line.chars().collect::<Vec<_>>();
            find_word_offsets(&chars, word)
                .into_iter()
                .map(move |start| {
                    let end = start + word.chars().count();
                    Range {
                        start: Position {
                            line: u32::try_from(line_idx).unwrap_or_default(),
                            character: u32::try_from(start).unwrap_or_default(),
                        },
                        end: Position {
                            line: u32::try_from(line_idx).unwrap_or_default(),
                            character: u32::try_from(end).unwrap_or_default(),
                        },
                    }
                })
        })
        .collect()
}

pub fn word_range_at_position(text: &str, position: Position) -> Option<Range> {
    let line = line_at(text, position.line)?;
    let character = usize::try_from(position.character).ok()?;
    let chars = line.chars().collect::<Vec<_>>();
    if character > chars.len() {
        return None;
    }

    let cursor = character.min(chars.len().saturating_sub(1));
    let mut start = cursor;
    while start > 0 && crate::syntax::is_ascii_identifier_char(chars[start.saturating_sub(1)]) {
        start -= 1;
    }

    let mut end = cursor;
    while end < chars.len() && crate::syntax::is_ascii_identifier_char(chars[end]) {
        end += 1;
    }

    (start < end).then_some(Range {
        start: Position {
            line: position.line,
            character: u32::try_from(start).unwrap_or_default(),
        },
        end: Position {
            line: position.line,
            character: u32::try_from(end).unwrap_or_default(),
        },
    })
}

pub fn line_at(text: &str, line_number: u32) -> Option<&str> {
    text.lines().nth(usize::try_from(line_number).ok()?)
}

pub fn byte_offset_at(text: &str, position: Position) -> Option<usize> {
    offset_at(text, position)
}

pub fn point_at(text: &str, position: Position) -> Option<Point> {
    let line = line_at(text, position.line)?;
    let character = usize::try_from(position.character).ok()?;
    let column = byte_offset_for_character(line, character)?;
    Some(Point {
        row: usize::try_from(position.line).ok()?,
        column,
    })
}

fn find_word_offsets(chars: &[char], word: &str) -> Vec<usize> {
    let word_chars = word.chars().collect::<Vec<_>>();
    let word_len = word_chars.len();
    if word_len == 0 || chars.len() < word_len {
        return Vec::new();
    }

    (0..=chars.len() - word_len)
        .filter(|start| {
            let end = start + word_len;
            chars[*start..end] == word_chars
                && start
                    .checked_sub(1)
                    .and_then(|idx| chars.get(idx))
                    .is_none_or(|ch| !crate::syntax::is_ascii_identifier_char(*ch))
                && chars
                    .get(end)
                    .is_none_or(|ch| !crate::syntax::is_ascii_identifier_char(*ch))
        })
        .collect()
}

fn offset_at(text: &str, position: Position) -> Option<usize> {
    let target_line = usize::try_from(position.line).ok()?;
    let target_character = usize::try_from(position.character).ok()?;
    let mut offset = 0;

    for (line_idx, line) in text.split_inclusive('\n').enumerate() {
        if line_idx == target_line {
            let line_without_newline = line.strip_suffix('\n').unwrap_or(line);
            return byte_offset_for_character(line_without_newline, target_character)
                .map(|line_offset| offset + line_offset);
        }
        offset += line.len();
    }

    (target_line == text.lines().count())
        .then_some(text.len())
        .filter(|_| target_character == 0)
}

fn byte_offset_for_character(line: &str, character: usize) -> Option<usize> {
    if character == 0 {
        return Some(0);
    }

    line.char_indices()
        .nth(character)
        .map(|(idx, _)| idx)
        .or_else(|| (line.chars().count() == character).then_some(line.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_references_on_word_boundaries() {
        let source = r#"
pub struct State {}
pub struct Stateful {}
pub fn use_state(state: State) -> State {
    state
}
"#;

        let refs = matching_word_ranges(source, "State");

        assert_eq!(refs.len(), 3);
        assert!(refs
            .iter()
            .all(|range| range.end.character - range.start.character == "State".len() as u32));
    }
}
