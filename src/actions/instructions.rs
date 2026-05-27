//! Instruction argument actions for Anchor `#[instruction]` attributes on context structs.
//!
//! This deeper module owns all logic for adding, replacing, and removing instruction arguments.
//! It provides a single seam (`code_actions`) for the thin central router in mod.rs.
//!
//! Locality: All instruction-related quickfix logic, edit calculation, type inference from the
//! callable function, and attribute parsing is concentrated here. Deletion test passes — removing
//! this module eliminates all "anchor-missing-instruction-argument" handling without scattering.
//!
//! Follows the architecture deepening: narrow interface (one code_actions call), deep implementation
//! (complex range calculation, duplicate name checking, fallback to function signature).

use {
    super::common::{
        diagnostic_code, diagnostic_quickfix, single_document_edit, single_text_edit, struct_line,
    },
    crate::{
        document::{ParsedDocument, SymbolRange},
        range,
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
    },
};

/// Main seam for the instructions family. Thin router in mod.rs calls this to get all
/// instruction-argument related quickfixes. Filters diagnostics by code and quickfix type.
pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = add_instruction_argument_actions(document, uri.clone(), diagnostics);
    actions.extend(replace_instruction_argument_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(remove_instruction_argument_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions
}

fn add_instruction_argument_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
        })
        .filter(|diagnostic| diagnostic_quickfix(diagnostic) == Some("add-instruction-argument"))
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let argument = data.get("argument").and_then(|value| value.as_str())?;
            let accounts_name = data
                .get("accountsStruct")
                .and_then(|value| value.as_str())?;
            let type_name = data
                .get("argumentType")
                .and_then(|value| value.as_str())
                .or_else(|| instruction_argument_type(document, accounts_name, argument))?;
            let edit = add_instruction_argument_edit(document, accounts_name, argument, type_name)?;
            let mut changes = HashMap::new();
            changes.insert(uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Add `{argument}: {type_name}` to #[instruction]"),
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
                    "anchorAction": "add-instruction-argument",
                    "accountsStruct": accounts_name,
                    "argument": argument,
                    "argumentType": type_name,
                })),
            })
        })
        .collect()
}

fn add_instruction_argument_edit(
    document: &ParsedDocument,
    accounts_name: &str,
    argument: &str,
    type_name: &str,
) -> Option<TextEdit> {
    let accounts = document.symbols().accounts_structs.get(accounts_name)?;
    if accounts.instruction_arguments.iter().any(|existing| {
        existing.name == argument
            || existing.name.trim_start_matches('_') == argument.trim_start_matches('_')
    }) {
        return None;
    }

    if let Some(line_number) = instruction_attribute_line(document, accounts) {
        let line = range::line_at(document.source(), line_number)?;
        let close_idx = line.find(")]")?;
        let before_close = &line[..close_idx];
        let separator =
            if before_close.trim_end().ends_with('(') || before_close.trim_end().ends_with(',') {
                ""
            } else {
                ", "
            };
        return Some(TextEdit {
            range: Range {
                start: Position {
                    line: line_number,
                    character: u32::try_from(close_idx).ok()?,
                },
                end: Position {
                    line: line_number,
                    character: u32::try_from(close_idx).ok()?,
                },
            },
            new_text: format!("{separator}{argument}: {type_name}"),
        });
    }

    let insert_line = struct_line(document.source(), accounts_name)?;
    let line = range::line_at(document.source(), insert_line)?;
    let indent = line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    Some(TextEdit {
        range: Range {
            start: Position {
                line: insert_line,
                character: 0,
            },
            end: Position {
                line: insert_line,
                character: 0,
            },
        },
        new_text: format!("{indent}#[instruction({argument}: {type_name})]\n"),
    })
}

fn instruction_attribute_line(document: &ParsedDocument, accounts: &SymbolRange) -> Option<u32> {
    let start = accounts
        .derive_accounts_range
        .map_or(0, |range| range.start.line);
    let end = accounts.selection_range.start.line;
    (start..=end).find(|line_number| {
        range::line_at(document.source(), *line_number)
            .is_some_and(|line| line.trim_start().starts_with("#[instruction"))
    })
}

fn instruction_argument_type<'a>(
    document: &'a ParsedDocument,
    accounts_name: &str,
    argument: &str,
) -> Option<&'a str> {
    document
        .symbols()
        .callable_functions()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts_name)
        })
        .flat_map(|instruction| instruction.arguments.iter())
        .find(|candidate| {
            candidate.name == argument
                || candidate.name.trim_start_matches('_') == argument.trim_start_matches('_')
        })
        .and_then(|candidate| candidate.type_name.as_deref())
}

fn replace_instruction_argument_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
        })
        .filter(|diagnostic| {
            diagnostic_quickfix(diagnostic) == Some("replace-instruction-argument")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let current = data.get("argument").and_then(|value| value.as_str())?;
            let expected = data.get("expected").and_then(|value| value.as_str())?;
            let expected_type = data.get("expectedType").and_then(|value| value.as_str())?;
            let range = instruction_argument_declaration_range(document, diagnostic)?;
            let replacement = format!("{expected}: {expected_type}");
            let edit = single_text_edit(range, replacement.clone());

            Some(CodeAction {
                title: format!("Replace `{current}` with `{replacement}` in #[instruction]"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "replace-instruction-argument",
                    "argument": current,
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn instruction_argument_declaration_range(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<Range> {
    let source_line = range::line_at(document.source(), diagnostic.range.start.line)?;
    let start = diagnostic.range.start;
    let end = instruction_argument_end_character(source_line, start.character)
        .unwrap_or(diagnostic.range.end.character);
    Some(Range {
        start,
        end: Position {
            line: start.line,
            character: end.max(diagnostic.range.end.character),
        },
    })
}

fn instruction_argument_end_character(line: &str, start_character: u32) -> Option<u32> {
    let start = usize::try_from(start_character).ok()?;
    let tail = line.get(start..)?;
    let end_offset = tail.find(',').or_else(|| tail.find(")]"))?;
    u32::try_from(start + end_offset).ok()
}

fn remove_instruction_argument_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some("anchor-missing-instruction-argument")
        })
        .filter(|diagnostic| diagnostic_quickfix(diagnostic) == Some("remove-instruction-argument"))
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let argument = data.get("argument").and_then(|value| value.as_str())?;
            let edit = remove_instruction_argument_edit(document, diagnostic)?;
            Some(CodeAction {
                title: format!("Remove `{argument}` from #[instruction]"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "remove-instruction-argument",
                    "argument": argument,
                })),
            })
        })
        .collect()
}

fn remove_instruction_argument_edit(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let line_number = diagnostic.range.start.line;
    let line = range::line_at(document.source(), line_number)?;
    let start = usize::try_from(diagnostic.range.start.character).ok()?;
    let open = line.find("#[instruction(")? + "#[instruction(".len();
    let close = line.rfind(")]")?;
    let before = line.get(open..start)?;
    if before.trim().is_empty() {
        let after_arg = instruction_argument_end_byte(line, start)?;
        if line.get(after_arg..close)?.trim().is_empty() {
            return Some(single_text_edit(
                Range {
                    start: Position {
                        line: line_number,
                        character: 0,
                    },
                    end: Position {
                        line: line_number + 1,
                        character: 0,
                    },
                },
                String::new(),
            ));
        }
        let end = skip_comma_and_spaces(line, after_arg);
        return Some(single_text_edit(
            Range {
                start: Position {
                    line: line_number,
                    character: u32::try_from(open).ok()?,
                },
                end: Position {
                    line: line_number,
                    character: u32::try_from(end).ok()?,
                },
            },
            String::new(),
        ));
    }

    let comma = line.get(..start)?.rfind(',')?;
    let end = instruction_argument_end_byte(line, start)?;
    Some(TextEdit {
        range: Range {
            start: Position {
                line: line_number,
                character: u32::try_from(comma).ok()?,
            },
            end: Position {
                line: line_number,
                character: u32::try_from(end).ok()?,
            },
        },
        new_text: String::new(),
    })
}

fn instruction_argument_end_byte(line: &str, start: usize) -> Option<usize> {
    let tail = line.get(start..)?;
    Some(start + tail.find(',').or_else(|| tail.find(")]"))?)
}

fn skip_comma_and_spaces(line: &str, start: usize) -> usize {
    let mut end = start;
    if line.as_bytes().get(end).is_some_and(|byte| *byte == b',') {
        end += 1;
        while line.as_bytes().get(end).is_some_and(|byte| *byte == b' ') {
            end += 1;
        }
    }
    end
}
