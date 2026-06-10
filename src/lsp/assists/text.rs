pub(super) fn leading_seed_identifier(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let end = expression
        .char_indices()
        .find_map(|(idx, ch)| (!crate::syntax::is_ascii_identifier_char(ch)).then_some(idx))
        .unwrap_or(expression.len());
    let identifier = &expression[..end];
    crate::syntax::is_ascii_identifier(identifier).then_some(identifier)
}

pub(super) fn line_indent(line: &str) -> String {
    line.chars().take_while(|ch| ch.is_whitespace()).collect()
}
