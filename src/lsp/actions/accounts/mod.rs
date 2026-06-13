//! Account-related code actions for Anchor `#[derive(Accounts)]` structs,
//! `Context<T>` types, missing account fields, mut constraints, system program,
//! and has_one targets.

mod context_structs;
mod field_edits;

use {
    super::common::{
        account_struct_closing_line, diagnostic_code, edit_distance, field_indent,
        single_document_edit, single_text_edit, snippet_text_edit,
    },
    crate::{
        document::{ParsedDocument, SymbolRange},
        range::line_at,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
    },
};

/// Main seam for the accounts family. Thin router in mod.rs calls this to get all
/// account-related quickfixes. Filters diagnostics by code and quickfix type.
pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = context_structs::code_actions(document, uri.clone(), range, diagnostics);
    actions.extend(replace_handler_member_actions(uri.clone(), diagnostics));
    actions.extend(remove_handler_field_call_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(replace_missing_account_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(field_edits::add_missing_account_field_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(replace_has_one_target_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(system_program_type_actions(uri.clone(), diagnostics));
    actions.extend(add_missing_system_program_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(add_mut_constraint_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(replace_handler_identifier_actions(uri.clone(), diagnostics));
    actions
}

fn replace_handler_member_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
        })
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                .is_some_and(|reason| {
                    matches!(
                        reason,
                        "unknown-handler-member"
                            | "unknown-handler-method"
                            | "unknown-struct-literal-field"
                            | "unknown-struct-pattern-field"
                    )
                })
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let reason = data.get("reason").and_then(|value| value.as_str())?;
            let missing = data
                .get("field")
                .or_else(|| data.get("method"))
                .and_then(|value| value.as_str())?;
            let replacement = closest_candidate(data, missing)?;
            let edit = single_text_edit(diagnostic.range, replacement.to_string());
            let anchor_action = if matches!(
                reason,
                "unknown-struct-literal-field" | "unknown-struct-pattern-field"
            ) {
                "replace-struct-record-field"
            } else {
                "replace-handler-member"
            };

            Some(CodeAction {
                title: format!("Replace `{missing}` with `{replacement}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": anchor_action,
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn remove_handler_field_call_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
        })
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("remove-handler-field-call")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let field = data.get("field").and_then(|value| value.as_str())?;
            let edit_range = field_call_suffix_range(document, diagnostic.range)?;
            let edit = single_text_edit(edit_range, String::new());

            Some(CodeAction {
                title: format!("Use `{field}` as a field"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "remove-handler-field-call",
                    "field": field,
                })),
            })
        })
        .collect()
}

fn field_call_suffix_range(document: &ParsedDocument, field_range: Range) -> Option<Range> {
    let line = line_at(document.source(), field_range.end.line)?;
    let suffix_start = usize::try_from(field_range.end.character).ok()?;
    if line.get(suffix_start..suffix_start.checked_add(2)?)? != "()" {
        return None;
    }
    Some(Range {
        start: field_range.end,
        end: Position {
            line: field_range.end.line,
            character: field_range.end.character.saturating_add(2),
        },
    })
}

fn replace_missing_account_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-account-reference")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let missing = data.get("account").and_then(|value| value.as_str())?;
            let accounts_name = data
                .get("accountsStruct")
                .and_then(|value| value.as_str())?;
            let replacement = if data.get("reason").and_then(|value| value.as_str())
                == Some("unknown-ctx-account-field")
            {
                closest_candidate(data, missing).or_else(|| {
                    document
                        .symbols()
                        .accounts_structs
                        .get(accounts_name)
                        .and_then(|accounts| closest_context_account(accounts, missing))
                })?
            } else {
                closest_candidate(data, missing).or_else(|| {
                    document
                        .symbols()
                        .accounts_structs
                        .get(accounts_name)
                        .and_then(|accounts| closest_account(accounts, missing))
                })?
            };
            let edit = single_text_edit(diagnostic.range, replacement.to_string());

            Some(CodeAction {
                title: format!("Replace `{missing}` with `{replacement}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-account-reference",
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn system_program_type_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter(|diagnostic| {
            let quickfix = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str());
            matches!(quickfix, Some("system-program-type" | "program-field-type"))
        })
        .map(|diagnostic| {
            let expected = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("expected"))
                .and_then(|value| value.as_str())
                .unwrap_or("Program<'info, System>");
            let quickfix_type = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfixType"))
                .and_then(|value| value.as_str())
                .unwrap_or(expected);
            let field_name = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("field"))
                .and_then(|value| value.as_str())
                .unwrap_or("system_program");
            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(diagnostic.range, quickfix_type)],
            );

            CodeAction {
                title: format!("Change `{field_name}` type to `{quickfix_type}`"),
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
                    "anchorAction": "program-field-type",
                    "field": field_name,
                    "expected": expected,
                    "quickfixType": quickfix_type,
                })),
            }
        })
        .collect()
}

fn add_missing_system_program_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter_map(|diagnostic| {
            let missing = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("missing"))
                .and_then(|value| value.as_str())?;
            if !matches!(
                missing,
                "system_program" | "token_program" | "associated_token_program"
            ) {
                return None;
            }
            let expected = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("expected"))
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| default_program_field_type(missing));
            let quickfix_type = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfixType"))
                .and_then(|value| value.as_str())
                .unwrap_or(expected);
            let accounts_name = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("accountsStruct"))
                .and_then(|value| value.as_str())?;
            let accounts = document.symbols().accounts_structs.get(accounts_name)?;
            let insert_line = account_struct_closing_line(document, accounts)?;
            let indent = field_indent(document, accounts);
            let edit = snippet_text_edit(
                Range {
                    start: Position {
                        line: insert_line,
                        character: 0,
                    },
                    end: Position {
                        line: insert_line,
                        character: 0,
                    },
                },
                &format!("{indent}pub {missing}: {quickfix_type},\n"),
            );
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Add `{missing}` to `{accounts_name}`"),
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
                    "anchorAction": "add-program-field",
                    "accountsStruct": accounts_name,
                    "field": missing,
                    "expected": expected,
                    "quickfixType": quickfix_type,
                })),
            })
        })
        .collect()
}

fn default_program_field_type(field_name: &str) -> &'static str {
    match field_name {
        "token_program" => "Program<'info, Token>",
        "associated_token_program" => "Program<'info, AssociatedToken>",
        _ => "Program<'info, System>",
    }
}

fn replace_handler_identifier_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-account-usage"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("reason"))
                .and_then(|value| value.as_str())
                == Some("unresolved-handler-identifier")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let missing = data.get("identifier").and_then(|value| value.as_str())?;
            let replacement = closest_candidate(data, missing)?;
            let edit = single_text_edit(diagnostic.range, replacement.to_string());

            Some(CodeAction {
                title: format!("Replace `{missing}` with `{replacement}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-handler-identifier",
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn replace_has_one_target_actions(
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
                == Some("replace-has-one-target")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let missing = data
                .get("missingDataField")
                .and_then(|value| value.as_str())?;
            let replacement = closest_data_field(document, data, missing)?;

            let edit = single_text_edit(diagnostic.range, replacement.to_string());

            Some(CodeAction {
                title: format!("Replace `{missing}` with `{replacement}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(false),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-account-reference",
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn closest_data_field<'a>(
    document: &'a ParsedDocument,
    data: &'a serde_json::Value,
    missing: &str,
) -> Option<&'a str> {
    let candidates = data
        .get("candidates")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .collect::<Vec<_>>();
    if !candidates.is_empty() {
        return candidates
            .into_iter()
            .min_by_key(|candidate| edit_distance(candidate, missing));
    }

    let account_type = data.get("accountType").and_then(|value| value.as_str())?;
    document
        .symbols()
        .account_data_structs
        .get(account_type)?
        .fields
        .iter()
        .min_by_key(|field| edit_distance(&field.name, missing))
        .map(|field| field.name.as_str())
}

fn add_mut_constraint_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic_code(diagnostic),
                Some("anchor-account-usage") | Some("anchor-constraint-shape")
            )
        })
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("add-mut-constraint")
        })
        .filter_map(|diagnostic| {
            let account = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("account"))
                .and_then(|value| value.as_str())?;
            let field = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("accountsStruct"))
                .and_then(|value| value.as_str())
                .and_then(|accounts_name| document.symbols().accounts_structs.get(accounts_name))
                .and_then(|accounts| accounts.fields.iter().find(|field| field.name == account))
                .or_else(|| {
                    document
                        .symbols()
                        .accounts_structs
                        .values()
                        .flat_map(|accounts| accounts.fields.iter())
                        .find(|field| field.name == account)
                })?;
            let edit = add_mut_edit(document, field)?;
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Add #[account(mut)] to `{account}`"),
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
                    "anchorAction": "add-mut-constraint",
                    "account": account,
                })),
            })
        })
        .collect()
}

fn add_mut_edit(document: &ParsedDocument, field: &SymbolRange) -> Option<TextEdit> {
    if let Some(constraint) = field.account_constraints.first() {
        let line = crate::range::line_at(document.source(), constraint.range.start.line)?;
        let insert_at = line.find("#[account(")? + "#[account(".len();
        return Some(snippet_text_edit(
            Range {
                start: Position {
                    line: constraint.range.start.line,
                    character: u32::try_from(insert_at).ok()?,
                },
                end: Position {
                    line: constraint.range.start.line,
                    character: u32::try_from(insert_at).ok()?,
                },
            },
            "mut, ",
        ));
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
        &format!("{indent}#[account(mut)]\n"),
    ))
}

fn closest_account<'a>(accounts: &'a SymbolRange, missing: &str) -> Option<&'a str> {
    accounts
        .fields
        .iter()
        .min_by_key(|field| edit_distance(&field.name, missing))
        .map(|field| field.name.as_str())
}

fn closest_context_account<'a>(accounts: &'a SymbolRange, missing: &str) -> Option<&'a str> {
    accounts
        .fields
        .iter()
        .min_by_key(|field| {
            (
                context_account_replacement_rank(field),
                edit_distance(&field.name, missing),
            )
        })
        .map(|field| field.name.as_str())
}

fn closest_candidate<'a>(data: &'a serde_json::Value, missing: &str) -> Option<&'a str> {
    data.get("candidates")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .min_by_key(|candidate| edit_distance(candidate, missing))
}

fn context_account_replacement_rank(field: &SymbolRange) -> u8 {
    match field.type_name.as_deref() {
        Some("Account") | Some("AccountLoader") | Some("InterfaceAccount") => 0,
        Some("UncheckedAccount") | Some("AccountInfo") => 1,
        Some("Signer") => 2,
        Some("Program") | Some("Interface") | Some("Sysvar") => 3,
        _ => 1,
    }
}
