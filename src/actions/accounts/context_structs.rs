use {
    super::super::common::{
        diagnostic_code, diagnostic_quickfix, eof_range, single_document_edit, single_text_edit,
        snippet_text_edit, struct_line,
    },
    super::field_edits::{accounts_struct_stub, inferred_account_fields},
    crate::document::ParsedDocument,
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, Position, Range, Url, WorkspaceEdit,
    },
};

pub(super) fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    _range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    actions.extend(derive_accounts_actions(document, uri.clone(), diagnostics));
    actions.extend(create_accounts_struct_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(fill_context_type_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions
}

fn derive_accounts_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-context-accounts"))
        .filter_map(|diagnostic| {
            let context_type = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("contextType"))
                .and_then(|value| value.as_str())?;
            let line = struct_line(document.source(), context_type)?;
            let edit = single_text_edit(
                Range {
                    start: Position { line, character: 0 },
                    end: Position { line, character: 0 },
                },
                "#[derive(Accounts)]\n".to_string(),
            );

            Some(CodeAction {
                title: format!("Add #[derive(Accounts)] to {context_type}"),
                kind: Some(CodeActionKind::QUICKFIX),
                diagnostics: Some(vec![diagnostic.clone()]),
                edit: Some(single_document_edit(uri.clone(), edit)),
                command: None,
                is_preferred: Some(true),
                disabled: None,
                data: Some(serde_json::json!({
                    "anchorAction": "derive-accounts",
                    "contextType": context_type,
                })),
            })
        })
        .collect()
}
fn create_accounts_struct_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-context-accounts"))
        .filter(|diagnostic| diagnostic_quickfix(diagnostic) == Some("create-accounts-struct"))
        .filter_map(|diagnostic| {
            let context_type = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("contextType"))
                .and_then(|value| value.as_str())?;
            if struct_line(document.source(), context_type).is_some() {
                return None;
            }

            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(
                    eof_range(document.source()),
                    &accounts_struct_stub(
                        document.source(),
                        context_type,
                        &inferred_account_fields(document, context_type),
                    ),
                )],
            );

            Some(CodeAction {
                title: format!("Create #[derive(Accounts)] struct {context_type}"),
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
                    "anchorAction": "create-accounts-struct",
                    "contextType": context_type,
                })),
            })
        })
        .collect()
}
fn fill_context_type_actions(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-context-accounts"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("fill-context-type")
        })
        .filter_map(|diagnostic| {
            let context_type = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("contextType"))
                .and_then(|value| value.as_str())?;
            let function = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("function"))
                .and_then(|value| value.as_str())
                .unwrap_or("handler");

            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(diagnostic.range, context_type)],
            );

            let mut actions = vec![CodeAction {
                title: format!("Use `Context<{context_type}>` for `{function}`"),
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
                    "anchorAction": "fill-context-type",
                    "contextType": context_type,
                    "function": function,
                })),
            }];

            if should_offer_context_stub(document, diagnostic, context_type) {
                let mut changes = HashMap::new();
                changes.insert(
                    uri.clone(),
                    vec![
                        snippet_text_edit(diagnostic.range, context_type),
                        snippet_text_edit(
                            eof_range(document.source()),
                            &accounts_struct_stub(
                                document.source(),
                                context_type,
                                &inferred_account_fields(document, context_type),
                            ),
                        ),
                    ],
                );
                actions.push(CodeAction {
                    title: format!("Use `Context<{context_type}>` and create Accounts struct"),
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
                        "anchorAction": "fill-context-type-and-create-accounts",
                        "contextType": context_type,
                        "function": function,
                    })),
                });
            }

            Some(actions)
        })
        .flatten()
        .collect()
}
fn should_offer_context_stub(
    document: &ParsedDocument,
    diagnostic: &Diagnostic,
    context_type: &str,
) -> bool {
    if document
        .symbols()
        .accounts_structs
        .contains_key(context_type)
        || struct_line(document.source(), context_type).is_some()
    {
        return false;
    }
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("contextTypeExists"))
        .and_then(|value| value.as_bool())
        == Some(false)
}
