#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintAssignment<'a> {
    pub key: &'a str,
    pub value: &'a str,
}

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

pub fn values_after_key<'a>(text: &'a str, key: &str) -> Vec<&'a str> {
    let mut values = Vec::new();
    let mut search_start = 0;
    while let Some(relative) = text[search_start..].find(key) {
        let key_start = search_start + relative;
        let after_key = key_start + key.len();
        let Some(rest) = text.get(after_key..) else {
            break;
        };
        if !has_constraint_key_boundary(text, key_start) {
            search_start = key_start + 1;
            continue;
        }
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('=') {
            search_start = after_key;
            continue;
        }
        let leading_ws = rest.len() - trimmed.len();
        let value_start = after_key + leading_ws + 1;
        let Some(after_equals) = text.get(value_start..) else {
            break;
        };
        let value = after_equals.trim_start();
        let value_offset = after_equals.len() - value.len();
        let value_start = value_start + value_offset;
        let value_end = value_start + top_level_value_len(value);
        if let Some(value) = text.get(value_start..value_end).map(str::trim) {
            if !value.is_empty() {
                values.push(value);
            }
        }
        search_start = value_end.max(after_key);
    }
    values
}

pub fn top_level_assignments(text: &str) -> Vec<ConstraintAssignment<'_>> {
    split_top_level(account_constraint_contents(text))
        .into_iter()
        .filter_map(split_top_level_assignment)
        .collect()
}

fn account_constraint_contents(text: &str) -> &str {
    let trimmed = text.trim();
    trimmed
        .strip_prefix("account(")
        .and_then(|inner| inner.strip_suffix(')'))
        .unwrap_or(trimmed)
}

fn split_top_level_assignment(text: &str) -> Option<ConstraintAssignment<'_>> {
    let equals_index = top_level_assignment_equals(text)?;
    let key = text.get(..equals_index)?.trim();
    let value = text.get(equals_index + 1..)?.trim();
    (!key.is_empty() && !value.is_empty()).then_some(ConstraintAssignment { key, value })
}

fn top_level_assignment_equals(text: &str) -> Option<usize> {
    let mut state = TopLevelScanState::default();
    for (idx, ch) in text.char_indices() {
        state.observe(ch);
        if ch == '=' && state.is_top_level() && is_assignment_equals(text, idx) {
            return Some(idx);
        }
    }
    None
}

fn is_assignment_equals(text: &str, idx: usize) -> bool {
    let previous = text[..idx].chars().next_back();
    let next = text[idx + 1..].chars().next();
    !matches!(previous, Some('!' | '<' | '>' | '=')) && !matches!(next, Some('='))
}

fn top_level_value_len(value: &str) -> usize {
    let mut state = TopLevelScanState::default();
    for (idx, ch) in value.char_indices() {
        if state.is_top_level() && matches!(ch, ')' | ',') {
            return idx;
        }
        state.observe(ch);
    }
    value.len()
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
    let mut start = 0usize;
    let mut state = TopLevelScanState::default();
    for (idx, ch) in value.char_indices() {
        state.observe(ch);
        if ch == ',' && state.is_top_level() {
            push_trimmed_part(&mut parts, value, start, idx);
            start = idx + ch.len_utf8();
        }
    }
    push_trimmed_part(&mut parts, value, start, value.len());
    parts
}

fn push_trimmed_part<'a>(parts: &mut Vec<&'a str>, value: &'a str, start: usize, end: usize) {
    if let Some(part) = value.get(start..end).map(str::trim) {
        if !part.is_empty() {
            parts.push(part);
        }
    }
}

#[derive(Debug, Default)]
struct TopLevelScanState {
    depth: usize,
    literal: Option<char>,
    escaped: bool,
}

impl TopLevelScanState {
    fn observe(&mut self, ch: char) {
        if let Some(delimiter) = self.literal {
            self.observe_literal(ch, delimiter);
            return;
        }

        match ch {
            '"' | '\'' => self.literal = Some(ch),
            '(' | '[' | '{' => self.depth += 1,
            ')' | ']' | '}' => self.depth = self.depth.saturating_sub(1),
            _ => {}
        }
    }

    fn observe_literal(&mut self, ch: char, delimiter: char) {
        if self.escaped {
            self.escaped = false;
            return;
        }
        if ch == '\\' {
            self.escaped = true;
            return;
        }
        if ch == delimiter {
            self.literal = None;
        }
    }

    fn is_top_level(&self) -> bool {
        self.depth == 0 && self.literal.is_none()
    }
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
    fn extracts_all_top_level_values_after_key() {
        assert_eq!(
            values_after_key(
                "account(constraint = sd, constraint = other.key() != Pubkey::default())",
                "constraint",
            ),
            vec!["sd", "other.key() != Pubkey::default()"]
        );
    }

    #[test]
    fn extracts_top_level_assignments() {
        assert_eq!(
            top_level_assignments(
                "account(init, space = 8 + State::INIT_SPACE, constraint = a == b, seeds = [b\"a,b\", user.key()])",
            ),
            vec![
                ConstraintAssignment {
                    key: "space",
                    value: "8 + State::INIT_SPACE",
                },
                ConstraintAssignment {
                    key: "constraint",
                    value: "a == b",
                },
                ConstraintAssignment {
                    key: "seeds",
                    value: "[b\"a,b\", user.key()]",
                },
            ]
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
        let parts = split_top_level(r#"a, call(b, c), [d, e], "f,g""#);
        assert_eq!(parts, vec!["a", "call(b, c)", "[d, e]", r#""f,g""#]);
    }
}
