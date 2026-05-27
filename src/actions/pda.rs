use {
    super::common::snippet_text_edit,
    crate::diagnostics::ANCHOR_PDA_SEED_RESOLUTION_CODE,
    std::collections::HashMap,
    tower_lsp::lsp_types::{
        CodeAction, CodeActionKind, Diagnostic, NumberOrString, Position, Range, Url, WorkspaceEdit,
    },
};

/// Generate code actions for PDA diagnostics.
pub fn code_actions(uri: Url, _range: Range, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    let mut actions = Vec::with_capacity(diagnostics.len() * 2);

    for diagnostic in diagnostics {
        if diagnostic_code(diagnostic) != Some(ANCHOR_PDA_SEED_RESOLUTION_CODE) {
            continue;
        }

        if let Some(action) = document_idl_limitation_action(uri.clone(), diagnostic) {
            actions.push(action);
        }

        if let Some(action) = copy_pda_derivation_action(uri.clone(), diagnostic) {
            actions.push(action);
        }
    }

    actions
}

fn document_idl_limitation_action(uri: Url, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let data = diagnostic.data.as_ref()?;
    let account = data.get("account").and_then(|v| v.as_str())?;
    let seed = data.get("seed").and_then(|v| v.as_str())?;

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![snippet_text_edit(
            Range {
                start: Position {
                    line: diagnostic.range.start.line,
                    character: 0,
                },
                end: Position {
                    line: diagnostic.range.start.line,
                    character: 0,
                },
            },
            &format!("    /// NOTE: PDA seed `{seed}` is not serializable to the Anchor IDL.\n    /// Clients must manually replicate this seed derivation.\n"),
        )],
    );

    Some(CodeAction {
        title: format!("Document IDL limitation for `{account}` seed"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(false),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "document-idl-limitation",
            "account": account,
            "seed": seed,
        })),
    })
}

fn build_ts_comment(account: &str, idl_visible: &[String], idl_invisible: &[String]) -> String {
    if idl_invisible.is_empty() {
        format!("// PDA `{account}` derivation (all seeds IDL-visible)")
    } else {
        let visible_part = if idl_visible.is_empty() {
            String::new()
        } else {
            format!("// IDL-visible seeds: {}\n", idl_visible.join(", "))
        };
        let invisible_part = idl_invisible
            .iter()
            .map(|seed| format!("// TODO: replicate `{seed}` in client"))
            .collect::<Vec<_>>()
            .join("\n");
        format!("// PDA `{account}` derivation:\n{visible_part}{invisible_part}")
    }
}

fn copy_pda_derivation_action(uri: Url, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let data = diagnostic.data.as_ref()?;
    let account = data.get("account").and_then(|v| v.as_str())?;

    let idl_visible = data
        .get("idlVisibleSeeds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let idl_invisible = data
        .get("idlInvisibleSeeds")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let ts_comment = build_ts_comment(account, &idl_visible, &idl_invisible);

    let mut changes = HashMap::new();
    changes.insert(
        uri.clone(),
        vec![snippet_text_edit(
            Range {
                start: Position {
                    line: diagnostic.range.end.line + 1,
                    character: 0,
                },
                end: Position {
                    line: diagnostic.range.end.line + 1,
                    character: 0,
                },
            },
            &format!("{ts_comment}\n"),
        )],
    );

    Some(CodeAction {
        title: format!("Copy `{account}` PDA derivation as TypeScript comment"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(WorkspaceEdit {
            changes: Some(changes),
            document_changes: None,
            change_annotations: None,
        }),
        command: None,
        is_preferred: Some(false),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "copy-pda-derivation",
            "account": account,
        })),
    })
}

fn diagnostic_code(diagnostic: &Diagnostic) -> Option<&str> {
    match diagnostic.code.as_ref()? {
        NumberOrString::String(code) => Some(code.as_str()),
        NumberOrString::Number(_) => None,
    }
}
