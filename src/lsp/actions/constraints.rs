//! Constraint-ordering code actions for Anchor account struct fields.
//! Owns all logic for duplicate, conflicting, missing, and reordered constraints.
//!
//! This deeper module provides a single seam (`code_actions`) for the thin central router.
//!
//! Locality: All constraint management quickfix logic, edit calculation,
//! constraint text manipulation, and ordering diagnostics are concentrated here.

use {
    super::common::{
        add_constraint_to_field_edit, diagnostic_code, next_non_ws, parser_rule_kind,
        previous_non_ws, signer_candidate, snippet_text_edit,
    },
    crate::{
        account_semantics,
        document::{ParsedDocument, SymbolRange},
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
    },
};

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    actions.extend(remove_duplicate_constraint_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(remove_conflicting_constraint_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(reorder_constraint_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(add_missing_constraint_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions
}

// ---------------------------------------------------------------------------
// Duplicate constraint removal
// ---------------------------------------------------------------------------

fn remove_duplicate_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter(|diagnostic| parser_rule_kind(diagnostic) == Some("duplicate"))
        .filter_map(|diagnostic| {
            let constraint = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("constraint"))
                .and_then(|value| value.as_str())
                .unwrap_or("constraint");
            let edit = remove_duplicate_constraint_edit(document, diagnostic)?;
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Remove duplicate Anchor `{constraint}` constraint"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "remove-duplicate-constraint",
                    "constraint": constraint,
                })),
            })
        })
        .collect()
}

fn remove_duplicate_constraint_edit(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let line = crate::range::line_at(document.source(), diagnostic.range.start.line)?;
    if diagnostic.range.start.line != diagnostic.range.end.line {
        return Some(snippet_text_edit(diagnostic.range, ""));
    }

    let chars = line.chars().collect::<Vec<_>>();
    let start = usize::try_from(diagnostic.range.start.character).ok()?;
    let end = usize::try_from(diagnostic.range.end.character)
        .ok()?
        .min(chars.len());
    if start >= end || start > chars.len() {
        return None;
    }

    let mut remove_start = start;
    let mut remove_end = end;

    if let Some(previous) = previous_non_ws(&chars, start).filter(|idx| chars[*idx] == ',') {
        remove_start = previous;
        while remove_end < chars.len() && chars[remove_end].is_whitespace() {
            remove_end += 1;
        }
    } else if let Some(next) = next_non_ws(&chars, end).filter(|idx| chars[*idx] == ',') {
        remove_end = (next + 1).min(chars.len());
        while remove_end < chars.len() && chars[remove_end].is_whitespace() {
            remove_end += 1;
        }
    }

    Some(snippet_text_edit(
        Range {
            start: Position {
                line: diagnostic.range.start.line,
                character: u32::try_from(remove_start).ok()?,
            },
            end: Position {
                line: diagnostic.range.start.line,
                character: u32::try_from(remove_end).ok()?,
            },
        },
        "",
    ))
}

// ---------------------------------------------------------------------------
// Conflicting constraint removal
// ---------------------------------------------------------------------------

fn remove_conflicting_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("remove-conflicting-constraints")
        })
        .filter_map(|diagnostic| {
            let remove = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("remove"))
                .and_then(|value| value.as_array())?
                .iter()
                .filter_map(|value| value.as_str())
                .collect::<Vec<_>>();
            let edit = remove_constraint_keys_edit(document, diagnostic.range.start, &remove)
                .or_else(|| {
                    diagnostic_account_constraint_position(document, diagnostic).and_then(
                        |position| remove_constraint_keys_edit(document, position, &remove),
                    )
                })?;
            let title = format!("Remove conflicting `{}` constraints", remove.join("`, `"));
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title,
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "remove-conflicting-constraints",
                    "remove": remove,
                })),
            })
        })
        .collect()
}

fn diagnostic_account_constraint_position(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<Position> {
    let account = diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("account"))
        .and_then(|value| value.as_str())?;
    document
        .symbols()
        .accounts_structs
        .values()
        .flat_map(|accounts| accounts.fields.iter())
        .find(|field| field.name == account)
        .and_then(|field| field.account_constraints.first())
        .map(|constraint| constraint.range.start)
}

fn remove_constraint_keys_edit(
    document: &ParsedDocument,
    position: Position,
    remove: &[&str],
) -> Option<TextEdit> {
    let attribute = account_attribute_at_position(document.source(), position)?;
    let segments = split_constraint_segments(attribute.inner)
        .into_iter()
        .filter(|segment| {
            !remove
                .iter()
                .any(|key| constraint_phrase_matches(&segment.key, key))
        })
        .collect::<Vec<_>>();

    Some(snippet_text_edit(
        Range {
            start: position_at_byte_offset(document.source(), attribute.inner_start)?,
            end: position_at_byte_offset(document.source(), attribute.inner_end)?,
        },
        &reordered_constraint_inner(attribute.inner, &segments),
    ))
}

// ---------------------------------------------------------------------------
// Constraint reordering
// ---------------------------------------------------------------------------

fn reorder_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter(|diagnostic| parser_rule_kind(diagnostic) == Some("ordering"))
        .filter_map(|diagnostic| {
            let message = parser_rule_message(diagnostic)?;
            let (required, before) = ordering_rule_parts(message)?;
            let edit = reorder_constraint_edit(document, diagnostic, required, before)?;
            let constraint = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("constraint"))
                .and_then(|value| value.as_str())
                .unwrap_or(before);
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Move Anchor `{required}` before `{before}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "reorder-anchor-constraint",
                    "required": required,
                    "constraint": constraint,
                    "before": before,
                })),
            })
        })
        .collect()
}

fn reorder_constraint_edit(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
    required: &str,
    before: &str,
) -> Option<TextEdit> {
    let attribute = account_attribute_at_position(document.source(), diagnostic.range.start)?;
    let mut segments = split_constraint_segments(attribute.inner);
    if segments.len() < 2 {
        return None;
    }

    let required_idx = segments
        .iter()
        .position(|segment| constraint_phrase_matches(&segment.key, required))?;
    let before_idx = segments
        .iter()
        .position(|segment| constraint_phrase_matches(&segment.key, before))
        .or_else(|| {
            let diagnostic_start =
                crate::range::byte_offset_at(document.source(), diagnostic.range.start)?;
            segments.iter().position(|segment| {
                attribute.inner_start + segment.start <= diagnostic_start
                    && diagnostic_start <= attribute.inner_start + segment.end
            })
        })?;

    if required_idx <= before_idx {
        return None;
    }

    let required_segment = segments.remove(required_idx);
    segments.insert(before_idx, required_segment);

    Some(snippet_text_edit(
        Range {
            start: position_at_byte_offset(document.source(), attribute.inner_start)?,
            end: position_at_byte_offset(document.source(), attribute.inner_end)?,
        },
        &reordered_constraint_inner(attribute.inner, &segments),
    ))
}

#[derive(Debug)]
struct AccountAttribute<'a> {
    inner: &'a str,
    inner_start: usize,
    inner_end: usize,
}

#[derive(Debug, Clone)]
struct ConstraintSegment {
    text: String,
    key: String,
    start: usize,
    end: usize,
}

fn account_attribute_at_position(source: &str, position: Position) -> Option<AccountAttribute<'_>> {
    let offset = crate::range::byte_offset_at(source, position)?;
    let search_end = offset.saturating_add("#[account".len()).min(source.len());
    let attr_start = source[..search_end].rfind("#[account")?;
    let open = attr_start + source[attr_start..].find('(')?;
    let close = matching_close_paren(source, open)?;
    if offset < attr_start || offset > close {
        return None;
    }

    Some(AccountAttribute {
        inner: &source[open + 1..close],
        inner_start: open + 1,
        inner_end: close,
    })
}

fn matching_close_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut string_quote = None;
    let mut escaped = false;

    for (idx, ch) in source[open..].char_indices() {
        let absolute_idx = open + idx;
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
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(absolute_idx);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_constraint_segments(inner: &str) -> Vec<ConstraintSegment> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut string_quote = None;
    let mut escaped = false;

    for (idx, ch) in inner.char_indices() {
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
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ',' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                push_constraint_segment(inner, start, idx, &mut segments);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    push_constraint_segment(inner, start, inner.len(), &mut segments);
    segments
}

fn push_constraint_segment(
    inner: &str,
    start: usize,
    end: usize,
    segments: &mut Vec<ConstraintSegment>,
) {
    let Some(raw) = inner.get(start..end) else {
        return;
    };
    let trimmed_start = raw.len() - raw.trim_start().len();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    let absolute_start = start + trimmed_start;
    let absolute_end = absolute_start + trimmed.len();
    segments.push(ConstraintSegment {
        text: trimmed.to_string(),
        key: segment_key(trimmed),
        start: absolute_start,
        end: absolute_end,
    });
}

fn segment_key(segment: &str) -> String {
    let segment = split_top_level(segment, '@')
        .next()
        .unwrap_or(segment)
        .trim();
    let key = split_top_level(segment, '=')
        .next()
        .unwrap_or(segment)
        .trim();
    key.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn split_top_level(text: &str, separator: char) -> impl Iterator<Item = &str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut paren_depth = 0usize;
    let mut bracket_depth = 0usize;
    let mut brace_depth = 0usize;
    let mut string_quote = None;
    let mut escaped = false;

    for (idx, ch) in text.char_indices() {
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
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            '{' => brace_depth += 1,
            '}' => brace_depth = brace_depth.saturating_sub(1),
            ch if ch == separator && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                parts.push(&text[start..idx]);
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(&text[start..]);
    parts.into_iter()
}

fn reordered_constraint_inner(inner: &str, segments: &[ConstraintSegment]) -> String {
    let texts = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>();
    if !inner.contains('\n') {
        let mut line = texts.join(", ");
        if inner.trim_end().ends_with(',') {
            line.push(',');
        }
        return line;
    }

    let indent = first_constraint_indent(inner).unwrap_or("    ");
    let mut body = texts.join(&format!(",\n{indent}"));
    if inner.trim_end().ends_with(',') {
        body.push(',');
    }

    let mut reordered = String::new();
    if inner.starts_with('\n') {
        reordered.push('\n');
    }
    reordered.push_str(indent);
    reordered.push_str(&body);
    if inner.ends_with('\n') {
        reordered.push('\n');
    }
    reordered
}

fn first_constraint_indent(inner: &str) -> Option<&str> {
    inner.lines().find_map(|line| {
        (!line.trim().is_empty()).then(|| {
            let indent_len = line.len() - line.trim_start().len();
            &line[..indent_len]
        })
    })
}

// ---------------------------------------------------------------------------
// Constraint text parsing
// ---------------------------------------------------------------------------

fn parser_rule_message(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("parserRule"))
        .and_then(|rule| rule.get("message"))
        .and_then(|value| value.as_str())
}

fn ordering_rule_parts(message: &str) -> Option<(&str, &str)> {
    message.split_once(" must be provided before ")
}

fn constraint_phrase_matches(key: &str, phrase: &str) -> bool {
    normalized_constraint_phrase(key) == normalized_constraint_phrase(phrase)
}

fn normalized_constraint_phrase(value: &str) -> String {
    value
        .replace("::", " ")
        .replace('_', " ")
        .split_whitespace()
        .filter(|part| *part != "account")
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn position_at_byte_offset(source: &str, offset: usize) -> Option<Position> {
    let offset = offset.min(source.len());
    if !source.is_char_boundary(offset) {
        return None;
    }

    let mut line = 0u32;
    let mut line_start = 0usize;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + 1;
        }
    }

    Some(Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).ok()?,
    })
}

// ---------------------------------------------------------------------------
// Missing constraint insertion
// ---------------------------------------------------------------------------

fn add_missing_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic_code(diagnostic),
                Some("anchor-constraint-shape")
                    | Some("anchor-security-signer")
                    | Some("anchor-security-cpi-program")
                    | Some("anchor-security-unchecked-account")
            )
        })
        .filter_map(|diagnostic| {
            let missing = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("missing"))
                .and_then(|value| value.as_str())?;
            let account = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("account"))
                .and_then(|value| value.as_str())?;
            let field = document
                .symbols()
                .accounts_structs
                .values()
                .flat_map(|accounts| accounts.fields.iter())
                .find(|field| field.name == account)?;
            let addition = missing_constraint_text(document, field, missing)?;
            let edit = add_constraint_to_field_edit(document, field, &addition)?;
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Add `{addition}` to `{account}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(WorkspaceEdit {
                    changes: Some(changes),
                    document_changes: None,
                    change_annotations: None,
                }),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "add-missing-constraint",
                    "account": account,
                    "constraint": missing,
                    "insertText": addition,
                })),
            })
        })
        .collect()
}

fn missing_constraint_text(
    document: &ParsedDocument,
    field: &SymbolRange,
    missing: &str,
) -> Option<String> {
    match missing {
        "mut" => Some("mut".to_string()),
        "dup" => Some("dup".to_string()),
        "signer" => Some("signer".to_string()),
        "executable" => Some("executable".to_string()),
        "seeds" => Some("seeds = [b\"seed\"]".to_string()),
        "realloc::payer" => signer_candidate_for_field(document, field)
            .map(|payer| format!("realloc::payer = {payer}")),
        "realloc::zero" => Some("realloc::zero = false".to_string()),
        "mint::decimals" => instruction_argument_candidate(document, field, "u8")
            .map(|argument| format!("mint::decimals = {argument}"))
            .or_else(|| Some("mint::decimals = 0".to_string())),
        "mint::authority" => signer_candidate_for_field(document, field)
            .map(|authority| format!("mint::authority = {authority}")),
        "token::mint" => account_candidate_for_field(document, field, |accounts, candidate| {
            is_mint_account(accounts, candidate)
        })
        .map(|mint| format!("token::mint = {mint}")),
        "token::authority" => signer_candidate_for_field(document, field)
            .map(|authority| format!("token::authority = {authority}")),
        "associated_token::mint" => {
            account_candidate_for_field(document, field, |accounts, candidate| {
                is_mint_account(accounts, candidate)
            })
            .map(|mint| format!("associated_token::mint = {mint}"))
        }
        "associated_token::authority" => signer_candidate_for_field(document, field)
            .map(|authority| format!("associated_token::authority = {authority}")),
        _ => None,
    }
}

fn signer_candidate_for_field<'a>(
    document: &'a ParsedDocument,
    field: &SymbolRange,
) -> Option<&'a str> {
    accounts_for_field(document, field)
        .and_then(|accounts| signer_candidate(accounts.fields.iter()))
}

fn instruction_argument_candidate<'a>(
    document: &'a ParsedDocument,
    field: &SymbolRange,
    preferred_type: &str,
) -> Option<&'a str> {
    let accounts = accounts_for_field(document, field)?;
    document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .flat_map(|instruction| instruction.arguments.iter())
        .find(|argument| argument.type_name.as_deref() == Some(preferred_type))
        .map(|argument| argument.name.as_str())
}

fn account_candidate_for_field<'a>(
    document: &'a ParsedDocument,
    field: &SymbolRange,
    predicate: impl Fn(&SymbolRange, &SymbolRange) -> bool,
) -> Option<&'a str> {
    accounts_for_field(document, field).and_then(|accounts| {
        accounts
            .fields
            .iter()
            .filter(|candidate| candidate.name != field.name)
            .find(|candidate| predicate(accounts, candidate))
            .map(|candidate| candidate.name.as_str())
    })
}

fn is_mint_account(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Account") | Some("InterfaceAccount")
    ) && account_semantics::field_has_declared_or_expected_account_inner_type(
        accounts, field, "Mint",
    )
}

fn accounts_for_field<'a>(
    document: &'a ParsedDocument,
    field: &SymbolRange,
) -> Option<&'a SymbolRange> {
    document
        .symbols()
        .accounts_structs
        .values()
        .find(|accounts| {
            accounts
                .fields
                .iter()
                .any(|candidate| candidate.name == field.name && candidate.range == field.range)
        })
}
