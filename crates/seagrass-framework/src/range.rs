use {
    proc_macro2::Span,
    tower_lsp::lsp_types::{Position, Range},
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

pub fn byte_offset_at(text: &str, position: Position) -> Option<usize> {
    let target_line = usize::try_from(position.line).ok()?;
    let target_character = usize::try_from(position.character).ok()?;
    let mut offset = 0usize;

    for (line_index, line) in text.split_inclusive('\n').enumerate() {
        if line_index == target_line {
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
        .map(|(offset, _)| offset)
        .or_else(|| (line.chars().count() == character).then_some(line.len()))
}
