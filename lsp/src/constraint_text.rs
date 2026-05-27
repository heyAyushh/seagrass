/// Checks whether `text` contains a `key = …` assignment.
///
/// This is used to detect explicit key-value syntax inside an
/// `#[account(...)]` constraint attribute.
///
/// # Examples
///
/// ```
/// use seagrass::constraint_text::has_key;
///
/// assert!(has_key("init, payer = user, space = 8", "payer"));
/// assert!(!has_key("init, payer = user", "space"));
/// ```
pub fn has_key(text: &str, key: &str) -> bool {
    key_assignment_start(text, key).is_some()
}

/// Checks whether `text` contains a bare `key` flag (no `=`).
///
/// A flag is recognised when the word is followed immediately by a comma
/// or closing parenthesis, and is not part of a larger identifier.
///
/// # Examples
///
/// ```
/// use seagrass::constraint_text::has_flag;
///
/// assert!(has_flag("init, mut, signer", "mut"));
/// assert!(!has_flag("init, mut", "signer"));
/// ```
pub fn has_flag(text: &str, key: &str) -> bool {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        let after = idx + key.len();
        if has_constraint_key_boundary(text, idx)
            && text
                .as_bytes()
                .get(after)
                .is_none_or(|byte| matches!(byte, b',' | b')'))
        {
            return true;
        }
        search_start = idx + 1;
    }
    false
}

/// Returns `true` if `text` contains `key` either as a flag or as a
/// `key = …` assignment.
pub fn has_flag_or_key(text: &str, key: &str) -> bool {
    has_flag(text, key) || has_key(text, key)
}

/// Returns the byte index where `key = ` begins in `text`, if present.
///
/// The search requires a word boundary before `key` and an `=` sign
/// immediately after it (ignoring whitespace).
pub fn key_assignment_start(text: &str, key: &str) -> Option<usize> {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        let after_key = idx + key.len();
        let rest = text.get(after_key..)?;
        if has_constraint_key_boundary(text, idx) && rest.trim_start().starts_with('=') {
            return Some(idx);
        }
        search_start = idx + 1;
    }
    None
}

/// Returns the raw text that appears after `key = ` in `text`.
///
/// **Important:** the returned slice is *not* parsed; it may contain
/// trailing commas, additional constraints, or nested expressions.
///
/// # Examples
///
/// ```
/// use seagrass::constraint_text::value_after_key;
///
/// assert_eq!(
///     value_after_key("init, payer = user, space = 8", "payer"),
///     Some("user, space = 8")
/// );
/// ```
pub fn value_after_key<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let start = key_assignment_start(text, key)? + key.len();
    let rest = text.get(start..)?;
    let after_equals = rest.find('=')? + 1;
    Some(text[start + after_equals..].trim_start())
}

/// Extracts the contents of the `[...]` array that follows `key = `.
///
/// Returns everything between the first `[` and its matching `]`.
///
/// # Examples
///
/// ```
/// use seagrass::constraint_text::bracket_value;
///
/// assert_eq!(
///     bracket_value("seeds = [b\"state\", user.key()]", "seeds"),
///     Some("b\"state\", user.key()")
/// );
/// ```
pub fn bracket_value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let idx = search_start + relative;
        let after_key = idx + key.len();
        let rest = text.get(after_key..)?;
        if has_constraint_key_boundary(text, idx) {
            let after_equals = rest.trim_start().strip_prefix('=')?;
            let after_bracket = after_equals.trim_start().strip_prefix('[')?;
            let end = after_bracket.find(']')?;
            return Some(&after_bracket[..end]);
        }
        search_start = idx + 1;
    }
    None
}

/// Returns `true` when the character just before `idx` is *not* part of
/// an identifier or path segment (i.e. not `:` or an ident char).
fn has_constraint_key_boundary(text: &str, idx: usize) -> bool {
    text[..idx]
        .chars()
        .next_back()
        .map(|ch| !(is_ident_char(ch) || ch == ':'))
        .unwrap_or(true)
}

/// Returns `true` for ASCII alphanumeric characters and `_`.
fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Splits a comma-separated list at the top level, trimming whitespace
/// and discarding empty parts.
///
/// # Examples
///
/// ```
/// use seagrass::constraint_text::split_top_level;
///
/// let parts = split_top_level("a, b, c");
/// assert_eq!(parts, vec!["a", "b", "c"]);
/// ```
pub fn split_top_level(value: &str) -> Vec<&str> {
    let mut parts = Vec::with_capacity(value.matches(',').count() + 1);
    parts.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty()),
    );
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_key_assignment() {
        assert!(has_key("init, payer = user, space = 8", "payer"));
        assert!(!has_key("init, payer = user", "space"));
    }

    #[test]
    fn detects_flag() {
        assert!(has_flag("init, mut, signer", "mut"));
        assert!(!has_flag("init, mut", "signer"));
    }

    #[test]
    fn detects_flag_or_key() {
        assert!(has_flag_or_key("init, payer = user", "payer"));
        assert!(has_flag_or_key("init, mut", "mut"));
        assert!(!has_flag_or_key("init", "signer"));
    }

    #[test]
    fn extracts_value_after_key() {
        assert_eq!(
            value_after_key("init, payer = user, space = 8", "payer"),
            Some("user, space = 8")
        );
    }

    #[test]
    fn extracts_bracket_value() {
        assert_eq!(
            bracket_value("seeds = [b\"state\", user.key()]", "seeds"),
            Some("b\"state\", user.key()")
        );
    }

    #[test]
    fn splits_top_level_comma_list() {
        let parts = split_top_level("a, b, c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }
}
