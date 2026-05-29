use {
    super::{document_symbols, ParsedDocument},
    crate::range::{byte_offset_at, is_in_comment_or_string},
    tower_lsp::lsp_types::{Position, Range},
};

const ACCOUNT_ATTRIBUTE_OPEN: &str = "#[account(";
const ACCOUNT_ATTRIBUTE_CALL: &str = "account(";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAttributeCursor {
    pub range: Range,
    pub field_name: Option<String>,
    pub slot: AccountAttributeSlot,
    pub prefix: String,
    pub constraint_key: Option<String>,
    pub in_seed_array: bool,
}

impl AccountAttributeCursor {
    pub fn from_source(source: &str, position: Position) -> Option<Self> {
        if is_in_comment_or_string(source, position) {
            return None;
        }

        let cursor = byte_offset_at(source, position)?;
        let prefix = &source[..cursor];
        let attribute_start = prefix.rfind(ACCOUNT_ATTRIBUTE_OPEN)?;
        Self::from_attribute_start(source, attribute_start, position)
    }

    pub(crate) fn from_attribute_range(
        source: &str,
        range: Range,
        position: Position,
    ) -> Option<Self> {
        if is_in_comment_or_string(source, position) {
            return None;
        }

        let attribute_start = byte_offset_at(source, range.start)?;
        Self::from_attribute_start(source, attribute_start, position)
    }

    fn from_attribute_start(
        source: &str,
        attribute_start: usize,
        position: Position,
    ) -> Option<Self> {
        let cursor = byte_offset_at(source, position)?;
        if cursor < attribute_start {
            return None;
        }
        let attribute_prefix = &source[attribute_start..cursor];
        let start = account_attribute_inner_start(attribute_prefix, attribute_start)?;
        let inside = &source[start..cursor];
        if account_attribute_closed_before_cursor(inside) {
            return None;
        }

        let constraint_key = assigned_constraint_key(inside);
        let in_seed_array = constraint_key.as_deref() == Some("seeds")
            && inside
                .rfind('=')
                .is_some_and(|assignment| has_unclosed_bracket(&inside[assignment + 1..]));
        let slot = account_attribute_slot(inside)?;
        let prefix = account_attribute_slot_prefix(inside, slot)?;

        Some(Self {
            range: Range {
                start: position_at_byte_offset(source, attribute_start),
                end: position,
            },
            field_name: None,
            slot,
            prefix: prefix.to_string(),
            constraint_key,
            in_seed_array,
        })
    }
}

fn account_attribute_inner_start(attribute_prefix: &str, attribute_start: usize) -> Option<usize> {
    attribute_prefix
        .find(ACCOUNT_ATTRIBUTE_OPEN)
        .map(|open| attribute_start + open + ACCOUNT_ATTRIBUTE_OPEN.len())
        .or_else(|| {
            attribute_prefix
                .find(ACCOUNT_ATTRIBUTE_CALL)
                .map(|open| attribute_start + open + ACCOUNT_ATTRIBUTE_CALL.len())
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAttributeSlot {
    Key,
    Value,
}

pub(super) fn account_attribute_field_name(
    document: &ParsedDocument,
    attribute_line: u32,
) -> Option<String> {
    document
        .symbols()
        .accounts_structs
        .values()
        .filter(|accounts| contains_line(accounts.range, attribute_line))
        .flat_map(|accounts| accounts.fields.iter())
        .filter(|field| field.selection_range.start.line > attribute_line)
        .min_by_key(|field| field.selection_range.start.line)
        .map(|field| field.name.clone())
        .or_else(|| account_attribute_field_name_from_symbols(document, attribute_line))
        .or_else(|| account_attribute_field_name_from_text(document.source(), attribute_line))
}

fn account_attribute_field_name_from_symbols(
    document: &ParsedDocument,
    attribute_line: u32,
) -> Option<String> {
    document_symbols(document)
        .into_iter()
        .filter(|symbol| symbol.detail.as_deref() == Some("#[derive(Accounts)]"))
        .filter(|symbol| contains_line(symbol.range, attribute_line))
        .flat_map(|symbol| symbol.children.unwrap_or_default())
        .filter(|field| field.selection_range.start.line > attribute_line)
        .min_by_key(|field| field.selection_range.start.line)
        .map(|field| field.name)
}

fn account_attribute_field_name_from_text(source: &str, attribute_line: u32) -> Option<String> {
    source
        .lines()
        .enumerate()
        .skip(usize::try_from(attribute_line).ok()?.saturating_add(1))
        .find_map(|(_, line)| {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("//") {
                return None;
            }
            let declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            let (name, _) = declaration.split_once(':')?;
            let name = name.trim();
            (!name.is_empty()
                && name
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
            .then(|| name.to_string())
        })
}

fn contains_line(range: Range, line: u32) -> bool {
    range.start.line <= line && line <= range.end.line
}

fn account_attribute_slot(inner: &str) -> Option<AccountAttributeSlot> {
    let Some((_, delimiter)) = inner
        .char_indices()
        .rev()
        .find(|(_, ch)| matches!(ch, ',' | '(' | '[' | '='))
    else {
        return Some(AccountAttributeSlot::Key);
    };
    let value_after_assignment = inner
        .rfind('=')
        .is_some_and(|assignment| has_unclosed_bracket(&inner[assignment + 1..]));
    if matches!(delimiter, '=' | '[') || delimiter == ',' && value_after_assignment {
        Some(AccountAttributeSlot::Value)
    } else {
        Some(AccountAttributeSlot::Key)
    }
}

fn account_attribute_slot_prefix(inner: &str, slot: AccountAttributeSlot) -> Option<&str> {
    let tail_start = inner
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| {
            let is_delimiter = match slot {
                AccountAttributeSlot::Key => matches!(ch, ',' | '('),
                AccountAttributeSlot::Value => matches!(ch, ',' | '(' | '[' | '='),
            };
            is_delimiter.then_some(idx + ch.len_utf8())
        })
        .unwrap_or(0);
    let tail = inner[tail_start..].trim_start();
    let end = tail
        .char_indices()
        .find_map(|(idx, ch)| {
            (!(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')).then_some(idx)
        })
        .unwrap_or(tail.len());
    Some(&tail[..end])
}

fn assigned_constraint_key(inner: &str) -> Option<String> {
    let mut search_end = inner.len();
    while let Some(assignment) = inner[..search_end].rfind('=') {
        let before_assignment = inner[..assignment].trim_end();
        let key_start = before_assignment
            .char_indices()
            .rev()
            .find_map(|(idx, ch)| matches!(ch, ',' | '(' | '[').then_some(idx + ch.len_utf8()))
            .unwrap_or(0);
        let key = before_assignment[key_start..].trim();
        if is_constraint_key_text(key) {
            return Some(key.to_string());
        }
        search_end = assignment;
    }
    None
}

fn account_attribute_closed_before_cursor(inner: &str) -> bool {
    let mut paren_depth = 1usize;
    let mut string_quote = None;
    let mut escaped = false;

    for ch in inner.chars() {
        if let Some(quote) = string_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                string_quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => string_quote = Some(ch),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            ']' if paren_depth == 0 => return true,
            _ => {}
        }
    }

    false
}

fn is_constraint_key_text(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
}

fn has_unclosed_bracket(text: &str) -> bool {
    let mut depth = 0usize;
    for ch in text.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    depth > 0
}

fn position_at_byte_offset(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let mut line = 0u32;
    let mut line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }
    Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).unwrap_or_default(),
    }
}
