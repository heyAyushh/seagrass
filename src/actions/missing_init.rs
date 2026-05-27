use {
    super::common::{
        diagnostic_code, diagnostic_touches_range, signer_candidate, single_document_edit,
    },
    crate::{
        diagnostics::ANCHOR_MISSING_INIT_CONSTRAINT_CODE,
        document::{ParsedDocument, SymbolRange},
    },
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, TextEdit, Url, WorkspaceEdit,
    },
};

pub(super) fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE)
        })
        .filter_map(|diagnostic| {
            missing_init_edit(document, diagnostic).map(|edit| (diagnostic, edit))
        })
        .map(|(diagnostic, edit)| {
            let account = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("account"))
                .and_then(|value| value.as_str())
                .unwrap_or("account");
            CodeAction {
                title: format!("Add Anchor init constraints to `{account}`"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "add-missing-init-constraint",
                    "account": account,
                })),
            }
        })
        .collect()
}
pub(super) fn fix_all_code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let cursor_touches_missing_init = diagnostics.iter().any(|diagnostic| {
        diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE)
            && diagnostic_touches_range(diagnostic, range)
    });
    if !cursor_touches_missing_init {
        return Vec::new();
    }

    let edits = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE)
        })
        .filter_map(|diagnostic| missing_init_edit(document, diagnostic))
        .collect::<Vec<_>>();
    if edits.len() < 2 {
        return Vec::new();
    }

    vec![CodeAction {
        title: "Fix all Anchor missing init constraints in file".to_string(),
        kind: Some(CodeActionKind::SOURCE_FIX_ALL),
        diagnostics: Some(
            diagnostics
                .iter()
                .filter(|diagnostic| {
                    diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE)
                })
                .cloned()
                .collect(),
        ),
        edit: None,
        command: None,
        is_preferred: Some(false),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "fix-all-missing-init-constraints",
            "uri": uri,
        })),
    }]
}
pub(super) fn resolve(
    document: &ParsedDocument,
    uri: Url,
    mut action: CodeAction,
    diagnostics: &[Diagnostic],
) -> CodeAction {
    if action.edit.is_some() {
        return action;
    }

    let Some(anchor_action) = action
        .data
        .as_ref()
        .and_then(|data| data.get("anchorAction"))
        .and_then(|value| value.as_str())
    else {
        return action;
    };

    if anchor_action == "fix-all-missing-init-constraints" {
        action.edit = fix_all_missing_init_edit(document, uri, diagnostics);
    }

    action
}
fn fix_all_missing_init_edit(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Option<WorkspaceEdit> {
    let edits = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic_code(diagnostic) == Some(ANCHOR_MISSING_INIT_CONSTRAINT_CODE)
        })
        .filter_map(|diagnostic| missing_init_edit(document, diagnostic))
        .collect::<Vec<_>>();
    if edits.len() < 2 {
        return None;
    }

    let mut changes = HashMap::new();
    changes.insert(uri, edits);
    Some(WorkspaceEdit {
        changes: Some(changes),
        document_changes: None,
        change_annotations: None,
    })
}
fn missing_init_edit(document: &ParsedDocument, diagnostic: &Diagnostic) -> Option<TextEdit> {
    let data = diagnostic.data.as_ref()?;
    let account = data.get("account").and_then(|value| value.as_str())?;
    let account_type = data
        .get("accountType")
        .and_then(|value| value.as_str())
        .unwrap_or("AccountType");
    let accounts_name = data
        .get("accountsStruct")
        .and_then(|value| value.as_str())?;
    let accounts = document.symbols().accounts_structs.get(accounts_name)?;
    let field = accounts.fields.iter().find(|field| field.name == account)?;
    let line_number = field
        .account_constraints
        .first()
        .map(|constraint| constraint.range.start.line)
        .unwrap_or(field.selection_range.start.line);
    let line = crate::range::line_at(document.source(), line_number)?;
    let indent = line
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let payer = payer_candidate(accounts).unwrap_or("payer");
    Some(TextEdit {
        range: Range {
            start: Position {
                line: line_number,
                character: 0,
            },
            end: Position {
                line: line_number,
                character: 0,
            },
        },
        new_text: format!(
            "{indent}#[account(init, payer = {payer}, space = 8 + {account_type}::INIT_SPACE)]\n"
        ),
    })
}
fn payer_candidate(accounts: &SymbolRange) -> Option<&str> {
    signer_candidate(accounts.fields.iter())
}
