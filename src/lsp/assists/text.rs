pub(super) fn leading_seed_identifier(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let end = expression
        .char_indices()
        .find_map(|(idx, ch)| (!is_identifier_char(ch)).then_some(idx))
        .unwrap_or(expression.len());
    let identifier = &expression[..end];
    (!identifier.is_empty()
        && identifier
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && identifier.chars().all(is_identifier_char))
    .then_some(identifier)
}

fn is_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

pub(super) fn line_indent(line: &str) -> String {
    line.chars().take_while(|ch| ch.is_whitespace()).collect()
}
