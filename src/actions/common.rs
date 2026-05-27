//! Shared helpers for building LSP code actions and edits.
//! Extracted from the god module to improve cohesion and DRY (stacc structural rules).
//! Includes diagnostic code/quickfix helpers used across action families for the thin router seam.

use {
    crate::{
        diagnostics::SOURCE,
        document::{ParsedDocument, SymbolRange},
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, TextEdit, Url,
        WorkspaceEdit,
    },
};

const POSITION_CHARACTER_STRIDE: u64 = 1_000_000;

/// Creates a single TextEdit for the given range and replacement text.
pub fn single_text_edit(range: Range, new_text: String) -> TextEdit {
    snippet_text_edit(range, &new_text)
}

pub fn snippet_text_edit(range: Range, template: &str) -> TextEdit {
    TextEdit {
        range,
        new_text: materialize_snippet_template(template),
    }
}

/// Creates a WorkspaceEdit containing a single TextEdit for one document.
pub fn single_document_edit(uri: Url, edit: TextEdit) -> WorkspaceEdit {
    WorkspaceEdit {
        changes: Some([(uri, vec![edit])].into_iter().collect()),
        document_changes: None,
        change_annotations: None,
    }
}

/// Extracts the Anchor diagnostic code if the source matches our SOURCE constant.
/// Used by all action families to filter relevant diagnostics at the seam.
pub fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
    if diagnostic.source.as_deref() != Some(SOURCE) {
        return None;
    }
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => Some(code.as_str()),
        NumberOrString::Number(_) => None,
    }
}

/// Extracts the "quickfix" value from diagnostic.data if present.
/// Allows specific quickfix variants (e.g. "add-instruction-argument") to be routed to the
/// appropriate deeper module without leaking implementation details into the router.
pub fn diagnostic_quickfix(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("quickfix"))
        .and_then(|value| value.as_str())
}

pub fn ranked_diagnostics_for_range(
    diagnostics: &[Diagnostic],
    cursor_range: Range,
) -> Vec<Diagnostic> {
    let should_filter = is_point_range(cursor_range)
        && diagnostics
            .iter()
            .any(|diagnostic| ranges_touch(diagnostic.range, cursor_range));
    let mut ranked = diagnostics
        .iter()
        .filter(|diagnostic| !should_filter || ranges_touch(diagnostic.range, cursor_range))
        .cloned()
        .collect::<Vec<_>>();
    ranked.sort_by_key(|diagnostic| range_distance(diagnostic.range, cursor_range));
    ranked
}

pub fn diagnostic_touches_range(diagnostic: &Diagnostic, cursor_range: Range) -> bool {
    ranges_touch(diagnostic.range, cursor_range)
}

pub fn sort_actions_by_cursor(actions: &mut [CodeAction], cursor_range: Range) {
    actions.sort_by_key(|action| {
        action
            .diagnostics
            .as_ref()
            .and_then(|diagnostics| diagnostics.first())
            .map(|diagnostic| range_distance(diagnostic.range, cursor_range))
            .unwrap_or(u64::MAX)
    });
}

fn ranges_touch(left: Range, right: Range) -> bool {
    position_key(left.start) <= position_key(right.end)
        && position_key(right.start) <= position_key(left.end)
}

fn is_point_range(range: Range) -> bool {
    range.start == range.end
}

fn range_distance(left: Range, right: Range) -> u64 {
    if ranges_touch(left, right) {
        return 0;
    }
    let left_start = position_key(left.start);
    let left_end = position_key(left.end);
    let right_start = position_key(right.start);
    let right_end = position_key(right.end);
    if left_end < right_start {
        right_start.saturating_sub(left_end)
    } else {
        left_start.saturating_sub(right_end)
    }
}

fn position_key(position: Position) -> u64 {
    u64::from(position.line)
        .saturating_mul(POSITION_CHARACTER_STRIDE)
        .saturating_add(u64::from(position.character))
}

fn materialize_snippet_template(template: &str) -> String {
    let mut output = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'$') {
            chars.next();
            output.push('$');
            continue;
        }
        if ch != '$' {
            output.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('{') => {
                chars.next();
                let placeholder = take_until_placeholder_end(&mut chars);
                output.push_str(&materialize_placeholder(&placeholder));
            }
            Some(ch) if ch.is_ascii_digit() => {
                consume_digits(&mut chars);
            }
            _ => output.push('$'),
        }
    }
    output
}

fn take_until_placeholder_end<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    let mut placeholder = String::new();
    while let Some(ch) = chars.next() {
        if ch == '}' {
            break;
        }
        placeholder.push(ch);
    }
    placeholder
}

fn materialize_placeholder(placeholder: &str) -> String {
    let digits_len = placeholder
        .bytes()
        .take_while(|byte| byte.is_ascii_digit())
        .count();
    if digits_len == 0 {
        return format!("${{{placeholder}}}");
    }
    let value = &placeholder[digits_len..];
    if let Some(default) = value.strip_prefix(':') {
        return materialize_snippet_template(default);
    }
    if let Some(choices) = value
        .strip_prefix('|')
        .and_then(|value| value.strip_suffix('|'))
    {
        return choices.split(',').next().unwrap_or("").to_string();
    }
    String::new()
}

fn consume_digits<I>(chars: &mut std::iter::Peekable<I>)
where
    I: Iterator<Item = char>,
{
    while chars.peek().is_some_and(|ch| ch.is_ascii_digit()) {
        chars.next();
    }
}

/// Finds the line number of a struct declaration by name in the source text.
/// Used by multiple action families (accounts, instructions, context types) to locate
/// where to insert attributes like `#[derive(Accounts)]` or `#[instruction(...)]`.
/// Simple linear scan is acceptable for LSP-scale files; concentrated here for locality.
pub fn struct_line(source: &str, name: &str) -> Option<u32> {
    source.lines().enumerate().find_map(|(idx, line)| {
        line.contains("struct")
            .then_some(line)
            .filter(|line| {
                line.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
                    .any(|word| word == name)
            })
            .and_then(|_| u32::try_from(idx).ok())
    })
}

pub fn eof_range(source: &str) -> Range {
    let mut line = 0u32;
    let mut character = 0u32;
    for ch in source.chars() {
        if ch == '\n' {
            line = line.saturating_add(1);
            character = 0;
        } else {
            character = character.saturating_add(ch.len_utf16() as u32);
        }
    }
    Range {
        start: Position { line, character },
        end: Position { line, character },
    }
}

pub fn add_constraint_edit(
    document: &ParsedDocument,
    attribute_range: Range,
    addition: &str,
) -> Option<TextEdit> {
    for line_number in attribute_range.start.line..=attribute_range.end.line {
        let line = crate::range::line_at(document.source(), line_number)?;
        if let Some(close_idx) = line.find(")]") {
            let before_close = &line[..close_idx];
            if before_close.trim().is_empty() {
                let indent = before_close.to_string();
                return Some(snippet_text_edit(
                    Range {
                        start: Position {
                            line: line_number,
                            character: 0,
                        },
                        end: Position {
                            line: line_number,
                            character: 0,
                        },
                    },
                    &format!("{indent}{addition},\n"),
                ));
            }

            let separator = if before_close.trim_end().ends_with('(')
                || before_close.trim_end().ends_with(',')
            {
                ""
            } else {
                ", "
            };
            return Some(snippet_text_edit(
                Range {
                    start: Position {
                        line: line_number,
                        character: u32::try_from(close_idx).ok()?,
                    },
                    end: Position {
                        line: line_number,
                        character: u32::try_from(close_idx).ok()?,
                    },
                },
                &format!("{separator}{addition}"),
            ));
        }
    }

    None
}

pub fn add_constraint_to_field_edit(
    document: &ParsedDocument,
    field: &SymbolRange,
    addition: &str,
) -> Option<TextEdit> {
    if let Some(constraint) = field.account_constraints.first() {
        return add_constraint_edit(document, constraint.range, addition);
    }

    let line = crate::range::line_at(document.source(), field.selection_range.start.line)?;
    let indent = line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    Some(snippet_text_edit(
        Range {
            start: Position {
                line: field.selection_range.start.line,
                character: 0,
            },
            end: Position {
                line: field.selection_range.start.line,
                character: 0,
            },
        },
        &format!("{indent}#[account({addition})]\n"),
    ))
}

pub fn constraint_action(
    uri: &Url,
    diagnostic: &Diagnostic,
    title: String,
    edit: TextEdit,
    is_preferred: bool,
    data: serde_json::Value,
) -> CodeAction {
    let mut changes = HashMap::with_capacity(1);
    changes.insert(uri.clone(), vec![edit]);
    CodeAction {
        title,
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(is_preferred),
        disabled: None,
        data: Some(data),
    }
}

pub fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();

    for (left_idx, left_ch) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right_chars.len() + 1);
        current.push(left_idx + 1);
        for (right_idx, right_ch) in right_chars.iter().enumerate() {
            let substitution = previous[right_idx] + usize::from(left_ch != *right_ch);
            let insertion = current[right_idx] + 1;
            let deletion = previous[right_idx + 1] + 1;
            current.push(substitution.min(insertion).min(deletion));
        }
        previous = current;
    }

    previous[right_chars.len()]
}

pub fn field_indent(document: &ParsedDocument, accounts: &SymbolRange) -> String {
    (accounts.range.start.line..=accounts.range.end.line)
        .filter_map(|line_number| crate::range::line_at(document.source(), line_number))
        .find_map(|line: &str| {
            let trimmed = line.trim_start();
            (trimmed.starts_with("#[account")
                || (trimmed.starts_with("pub ") && trimmed.contains(':')))
            .then(|| line[..line.len() - trimmed.len()].to_string())
        })
        .unwrap_or_else(|| "    ".to_string())
}

pub fn account_struct_closing_line(
    document: &ParsedDocument,
    accounts: &SymbolRange,
) -> Option<u32> {
    (accounts.range.start.line..=accounts.range.end.line)
        .rev()
        .find(|line_number| {
            crate::range::line_at(document.source(), *line_number)
                .is_some_and(|line| line.trim_start().starts_with('}'))
        })
}

// Shared parser and account-shape helpers used by sibling action modules.
fn is_signer(field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some("Signer") || field_has_constraint_key(field, "signer")
}

pub(super) fn signer_candidate<'a>(
    fields: impl Iterator<Item = &'a SymbolRange>,
) -> Option<&'a str> {
    fields
        .filter(|field| is_signer(field))
        .min_by_key(|field| signer_score(field))
        .map(|field| field.name.as_str())
}

fn signer_score(field: &SymbolRange) -> (u8, u32, u32) {
    (
        if field.type_name.as_deref() == Some("Signer") {
            0
        } else {
            1
        },
        field.selection_range.start.line,
        field.selection_range.start.character,
    )
}

fn field_has_constraint_key(field: &SymbolRange, key: &str) -> bool {
    field
        .account_constraints
        .iter()
        .any(|constraint| constraint_has_key(&constraint.text, key))
}

fn constraint_has_key(text: &str, key: &str) -> bool {
    let mut search_end = text.len();
    while let Some(idx) = text[..search_end].rfind(key) {
        if has_constraint_key_boundary(text, idx, key.len()) {
            return true;
        }
        search_end = idx;
    }
    false
}

fn has_constraint_key_boundary(text: &str, idx: usize, key_len: usize) -> bool {
    let previous = text[..idx].chars().next_back();
    let next = text[idx + key_len..].chars().next();
    previous
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .unwrap_or(true)
        && next
            .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
            .unwrap_or(true)
}

pub(super) fn parser_rule_kind(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("parserRule"))
        .and_then(|rule| rule.get("kind"))
        .and_then(|value| value.as_str())
}

pub(super) fn previous_non_ws(chars: &[char], start: usize) -> Option<usize> {
    chars
        .get(..start)?
        .iter()
        .enumerate()
        .rev()
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
}

pub(super) fn next_non_ws(chars: &[char], start: usize) -> Option<usize> {
    chars
        .iter()
        .enumerate()
        .skip(start)
        .find_map(|(idx, ch)| (!ch.is_whitespace()).then_some(idx))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_range() -> Range {
        Range {
            start: Position {
                line: 1,
                character: 2,
            },
            end: Position {
                line: 3,
                character: 4,
            },
        }
    }

    #[test]
    fn snippet_text_edit_materializes_default_placeholder() {
        let edit = snippet_text_edit(
            sample_range(),
            "pub ${1:state}: ${2:Account<'info, State>},",
        );

        assert_eq!(edit.range, sample_range());
        assert_eq!(edit.new_text, "pub state: Account<'info, State>,");
    }

    #[test]
    fn snippet_text_edit_materializes_choice_placeholder() {
        let edit = snippet_text_edit(sample_range(), "#[account(${1|mut,signer,init|})]");

        assert_eq!(edit.new_text, "#[account(mut)]");
    }

    #[test]
    fn snippet_text_edit_unescapes_dollar() {
        let edit = snippet_text_edit(sample_range(), "msg!(\"cost: \\${1:amount}\");");

        assert_eq!(edit.new_text, "msg!(\"cost: ${1:amount}\");");
    }

    #[test]
    fn snippet_text_edit_materializes_multiple_tabstops() {
        let edit = snippet_text_edit(
            sample_range(),
            "pub ${1:authority}: ${2:Signer<'info>},${0}",
        );

        assert_eq!(edit.new_text, "pub authority: Signer<'info>,");
    }

    #[test]
    fn snippet_text_edit_drops_final_cursor_marker() {
        let edit = snippet_text_edit(sample_range(), "Ok(())$0");

        assert_eq!(edit.new_text, "Ok(())");
    }

    #[test]
    fn snippet_text_edit_keeps_plain_text_idempotent() {
        let plain = "#[account(mut, has_one = authority)]";
        let edit = snippet_text_edit(sample_range(), plain);

        assert_eq!(edit.new_text, plain);
    }
}
