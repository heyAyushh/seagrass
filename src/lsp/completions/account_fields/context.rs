use {
    crate::{
        account_semantics,
        document::{ParsedDocument, SymbolRange},
        range::{byte_offset_at, line_at},
    },
    tower_lsp::lsp_types::Position,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FieldCompletionContext {
    FieldName { prefix: String },
    FieldType { prefix: String },
    TypeArgument { container: String, prefix: String },
}

pub(super) fn is_in_accounts_struct(document: &ParsedDocument, position: Position) -> bool {
    document
        .tree_sitter()
        .is_some_and(|syntax| syntax.accounts_struct_at_position(document.source(), position))
        || document
            .symbols()
            .accounts_structs
            .values()
            .any(|accounts| contains_line(accounts, position.line))
        || text_accounts_struct_context(document.source(), position)
}

fn contains_line(accounts: &SymbolRange, line: u32) -> bool {
    accounts.range.start.line <= line && line <= accounts.range.end.line
}

fn text_accounts_struct_context(source: &str, position: Position) -> bool {
    let Some(offset) = byte_offset_at(source, position) else {
        return false;
    };
    let prefix = &source[..offset.min(source.len())];
    let Some(derive_idx) = prefix.rfind("#[derive(Accounts") else {
        return false;
    };
    let after_derive = &source[derive_idx..];
    let Some(struct_relative) = after_derive.find("struct") else {
        return false;
    };
    let struct_idx = derive_idx + struct_relative;
    if struct_idx > offset {
        return false;
    }
    let Some(open_relative) = source[struct_idx..].find('{') else {
        return false;
    };
    let open = struct_idx + open_relative;
    if open > offset {
        return false;
    }
    let close = matching_close_brace(source, open).unwrap_or(source.len());
    offset <= close
}

fn matching_close_brace(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut string_quote = None;
    let mut escaped = false;
    for (idx, ch) in source[open..].char_indices() {
        let absolute = open + idx;
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
            '"' => string_quote = Some(ch),
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(absolute);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn field_completion_context(
    source: &str,
    position: Position,
) -> Option<FieldCompletionContext> {
    let line = line_at(source, position.line)?;
    let cursor = byte_offset_for_character(line, usize::try_from(position.character).ok()?)?;
    let prefix = &line[..cursor.min(line.len())];
    let trimmed = prefix.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with("#[") {
        return None;
    }

    if let Some(colon) = field_type_colon(prefix) {
        let after_colon = &prefix[colon + 1..];
        if has_top_level_field_separator(after_colon) {
            return None;
        }
        if let Some(container) = type_argument_container(after_colon) {
            return Some(FieldCompletionContext::TypeArgument {
                container,
                prefix: type_argument_prefix(after_colon).unwrap_or_default(),
            });
        }
        return Some(FieldCompletionContext::FieldType {
            prefix: field_type_prefix(after_colon),
        });
    }

    let visibility_prefix = trimmed.starts_with("pub ")
        || trimmed.starts_with("pub(")
        || trimmed == "pub"
        || trimmed.is_empty();
    visibility_prefix.then(|| FieldCompletionContext::FieldName {
        prefix: field_name_prefix(trimmed),
    })
}

fn field_name_before_colon(source: &str, position: Position) -> Option<String> {
    let line = line_at(source, position.line)?;
    let cursor = byte_offset_for_character(line, usize::try_from(position.character).ok()?)?;
    let prefix = &line[..cursor.min(line.len())];
    let colon = field_type_colon(prefix)?;
    identifier_before(&prefix[..colon])
}

pub(super) fn account_field_name_at_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<String> {
    field_name_before_colon(document.source(), position).or_else(|| {
        document
            .tree_sitter()
            .and_then(|syntax| syntax.account_field_at_position(document.source(), position))
            .and_then(|field| field.name)
    })
}

pub(super) fn account_field_at_position<'a>(
    document: &'a ParsedDocument,
    accounts: &'a SymbolRange,
    position: Position,
) -> Option<&'a SymbolRange> {
    let name = account_field_name_at_position(document, position)?;
    accounts.fields.iter().find(|field| field.name == name)
}

pub(super) fn account_struct_at_position(
    document: &ParsedDocument,
    position: Position,
) -> Option<&SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .find(|accounts| contains_line(accounts, position.line))
}

pub(super) fn semantic_inner_type_for_field(
    document: &ParsedDocument,
    field: &SymbolRange,
) -> Option<&'static str> {
    account_semantics::expected_account_inner_type_for_document_field(field)
        .map(|expected| expected.generic)
        .or_else(|| {
            document
                .symbols()
                .accounts_structs
                .values()
                .find(|accounts| {
                    accounts
                        .fields
                        .iter()
                        .any(|candidate| candidate.name == field.name)
                })
                .and_then(|accounts| {
                    account_semantics::expected_account_inner_types_for_document_accounts_struct(
                        accounts,
                    )
                    .get(&field.name)
                    .map(|expected| expected.generic)
                })
        })
}

pub(super) fn semantic_inner_type_for_account_field_name(
    accounts: &SymbolRange,
    field_name: &str,
) -> Option<&'static str> {
    account_semantics::expected_account_inner_types_for_document_accounts_struct(accounts)
        .get(field_name)
        .map(|expected| expected.generic)
}

pub(super) fn semantic_inner_type_from_source_context(
    source: &str,
    position: Position,
    field_name: &str,
) -> Option<&'static str> {
    let field_constraints = source_accounts_field_constraints(source, position)?;
    account_semantics::expected_account_inner_types_from_field_constraints(field_constraints)
        .get(field_name)
        .map(|expected| expected.generic)
}

fn source_accounts_field_constraints(
    source: &str,
    position: Position,
) -> Option<Vec<(String, Vec<String>)>> {
    let lines = source.lines().collect::<Vec<_>>();
    let cursor_line = usize::try_from(position.line).ok()?;
    let struct_start = (0..=cursor_line).rev().find(|line_idx| {
        lines
            .get(*line_idx)
            .is_some_and(|line| line.contains("struct") && line.contains("<'info"))
            && (0..=*line_idx).rev().take(4).any(|attr_idx| {
                lines
                    .get(attr_idx)
                    .is_some_and(|line| line.contains("#[derive(Accounts"))
            })
    })?;
    let mut fields = Vec::new();
    let mut pending_constraints = Vec::new();
    let mut account_attr = String::new();
    let mut in_account_attr = false;

    for line in lines.iter().skip(struct_start + 1) {
        let trimmed = line.trim();
        if trimmed.starts_with('}') {
            break;
        }
        if trimmed.starts_with("#[account") {
            in_account_attr = true;
            account_attr.clear();
        }
        if in_account_attr {
            account_attr.push_str(trimmed);
            if trimmed.contains(")]") {
                pending_constraints.push(account_attr.clone());
                account_attr.clear();
                in_account_attr = false;
            }
            continue;
        }
        if let Some(name) = field_name_from_line(trimmed) {
            fields.push((name, std::mem::take(&mut pending_constraints)));
        }
    }

    Some(fields)
}

fn field_name_from_line(line: &str) -> Option<String> {
    let without_pub = line
        .strip_prefix("pub ")
        .or_else(|| line.strip_prefix("pub(crate) "))
        .or_else(|| line.strip_prefix("pub(super) "))?;
    let (name, _) = without_pub.split_once(':')?;
    let name = name.trim();
    (!name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_'))
    .then(|| name.to_string())
}

fn field_type_colon(prefix: &str) -> Option<usize> {
    prefix.char_indices().find_map(|(idx, ch)| {
        if ch != ':' {
            return None;
        }
        let previous = prefix[..idx].chars().next_back();
        let next = prefix[idx + ch.len_utf8()..].chars().next();
        (previous != Some(':') && next != Some(':')).then_some(idx)
    })
}

fn type_argument_container(after_colon: &str) -> Option<String> {
    let mut open_angles = Vec::new();
    for (idx, ch) in after_colon.char_indices() {
        match ch {
            '<' => open_angles.push(idx),
            '>' => {
                open_angles.pop();
            }
            _ => {}
        }
    }

    let open = *open_angles.last()?;
    let after_open = &after_colon[open + 1..];
    if !has_top_level_comma(after_open) {
        return None;
    }

    identifier_before(&after_colon[..open])
}

fn type_argument_prefix(after_colon: &str) -> Option<String> {
    let open = after_colon.char_indices().rfind(|(_, ch)| *ch == '<')?.0;
    let after_open = &after_colon[open + 1..];
    let prefix_start = top_level_comma_offsets(after_open)
        .last()
        .map(|comma| comma + 1)
        .unwrap_or(0);
    let raw_prefix = after_open[prefix_start..].trim_start();
    let end = raw_prefix
        .char_indices()
        .find_map(|(idx, ch)| (!crate::syntax::is_ascii_identifier_char(ch)).then_some(idx))
        .unwrap_or(raw_prefix.len());

    Some(raw_prefix[..end].to_string())
}

fn field_name_prefix(trimmed_prefix: &str) -> String {
    let after_visibility = trimmed_prefix
        .strip_prefix("pub ")
        .or_else(|| trimmed_prefix.strip_prefix("pub(crate) "))
        .unwrap_or(trimmed_prefix);
    identifier_before(after_visibility).unwrap_or_default()
}

fn field_type_prefix(after_colon: &str) -> String {
    let trimmed = after_colon.trim_start();
    let end = trimmed
        .char_indices()
        .find_map(|(idx, ch)| (!crate::syntax::is_ascii_identifier_char(ch)).then_some(idx))
        .unwrap_or(trimmed.len());
    trimmed[..end].to_string()
}

fn identifier_before(text: &str) -> Option<String> {
    let trimmed = text.trim_end();
    let end = trimmed.char_indices().rev().find_map(|(idx, ch)| {
        crate::syntax::is_ascii_identifier_char(ch).then_some(idx + ch.len_utf8())
    })?;
    let start = trimmed[..end]
        .char_indices()
        .rev()
        .find_map(|(idx, ch)| {
            (!crate::syntax::is_ascii_identifier_char(ch)).then_some(idx + ch.len_utf8())
        })
        .unwrap_or(0);
    Some(trimmed[start..end].to_string())
}

fn has_top_level_comma(text: &str) -> bool {
    !top_level_comma_offsets(text).is_empty()
}

fn has_top_level_field_separator(text: &str) -> bool {
    top_level_char(text, ',') || top_level_char(text, ';')
}

fn top_level_char(text: &str, target: char) -> bool {
    top_level_char_offsets(text, target).next().is_some()
}

fn top_level_comma_offsets(text: &str) -> Vec<usize> {
    top_level_char_offsets(text, ',').collect()
}

fn top_level_char_offsets(text: &str, target: char) -> impl Iterator<Item = usize> + '_ {
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut angle_depth = 0usize;
    let mut string_quote = None;
    let mut escaped = false;

    text.char_indices().filter_map(move |(idx, ch)| {
        if let Some(quote) = string_quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == quote {
                string_quote = None;
            }
            return None;
        }

        match ch {
            '"' => string_quote = Some(ch),
            '(' => paren_depth += 1,
            ')' => paren_depth = paren_depth.saturating_sub(1),
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            '<' => angle_depth += 1,
            '>' => angle_depth = angle_depth.saturating_sub(1),
            ch if ch == target
                && paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && angle_depth == 0 =>
            {
                return Some(idx);
            }
            _ => {}
        }
        None
    })
}

pub(super) fn matches_prefix(label: &str, prefix: &str) -> bool {
    !prefix.is_empty() && super::super::matches_completion_prefix(label, prefix)
}

pub(super) fn prefix_starts_like_rust_type(prefix: &str) -> bool {
    prefix
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
}

pub(super) fn prefix_is_specific_generic_query(prefix: &str) -> bool {
    prefix_starts_like_rust_type(prefix) && prefix.chars().count() >= 2
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
