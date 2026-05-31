pub(super) fn primary_type_name(type_display: &str) -> Option<&str> {
    let trimmed = type_display.trim();
    let end = trimmed
        .find(|ch: char| !matches!(ch, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9'))
        .unwrap_or(trimmed.len());
    (end > 0).then(|| &trimmed[..end])
}
