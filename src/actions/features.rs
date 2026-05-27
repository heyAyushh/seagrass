//! Feature-flag and keyword-value code actions for Anchor Cargo.toml features.
//! Single seam (`code_actions`) for the thin central router.

use {
    super::common::{diagnostic_code, edit_distance, snippet_text_edit},
    crate::diagnostics::{check_cfg, project_identity},
    std::{collections::HashMap, fs},
    tower_lsp::lsp_types::{CodeAction, CodeActionKind, Diagnostic, Url, WorkspaceEdit},
};

pub fn code_actions(
    _document: &crate::document::ParsedDocument,
    uri: Url,
    _range: tower_lsp::lsp_types::Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = Vec::new();
    actions.extend(replace_keyword_value_actions(uri.clone(), diagnostics));
    actions.extend(add_anchor_debug_feature_actions(diagnostics));
    actions.extend(add_init_if_needed_feature_actions(diagnostics));
    actions.extend(sync_declare_id_actions(uri.clone(), diagnostics));
    actions
}

fn replace_keyword_value_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-constraint-shape"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some("replace-keyword-value")
        })
        .filter_map(|diagnostic| {
            let data = diagnostic.data.as_ref()?;
            let current = data.get("value").and_then(|value| value.as_str())?;
            let replacement = data
                .get("candidates")
                .and_then(|value| value.as_array())?
                .iter()
                .filter_map(|value| value.as_str())
                .min_by_key(|candidate| edit_distance(candidate, current))?;
            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(diagnostic.range, replacement)],
            );

            Some(CodeAction {
                title: format!("Replace `{current}` with `{replacement}`"),
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
                    "anchorAction": "replace-keyword-value",
                    "replacement": replacement,
                })),
            })
        })
        .collect()
}

fn add_anchor_debug_feature_actions(diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-check-cfg"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some(check_cfg::ADD_ANCHOR_DEBUG_FEATURE_QUICKFIX)
        })
        .filter_map(|diagnostic| {
            let manifest_uri = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("manifest"))
                .and_then(|value| value.as_str())
                .and_then(|value| Url::parse(value).ok())?;
            let manifest_path = manifest_uri.to_file_path().ok()?;
            let manifest_text = fs::read_to_string(manifest_path).ok()?;
            let edit = check_cfg::anchor_debug_feature_edit(&manifest_text)?;
            let mut changes = HashMap::new();
            changes.insert(manifest_uri.clone(), vec![edit]);

            Some(CodeAction {
                title: "Add `anchor-debug = []` to Cargo.toml features".to_string(),
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
                    "anchorAction": "add-anchor-debug-feature",
                })),
            })
        })
        .collect()
}

fn add_init_if_needed_feature_actions(diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-check-cfg"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some(check_cfg::ADD_INIT_IF_NEEDED_FEATURE_QUICKFIX)
        })
        .filter_map(|diagnostic| {
            let manifest_uri = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("manifest"))
                .and_then(|value| value.as_str())
                .and_then(|value| Url::parse(value).ok())?;
            let feature = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("feature"))
                .and_then(|value| value.as_str())
                .unwrap_or("init-if-needed");
            let manifest_path = manifest_uri.to_file_path().ok()?;
            let manifest_text = fs::read_to_string(manifest_path).ok()?;
            let edit = check_cfg::anchor_lang_feature_edit(&manifest_text, feature)?;
            let mut changes = HashMap::new();
            changes.insert(manifest_uri.clone(), vec![edit]);

            Some(CodeAction {
                title: format!("Enable Anchor `{feature}` Cargo feature"),
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
                    "anchorAction": "add-anchor-lang-feature",
                    "feature": feature,
                })),
            })
        })
        .collect()
}

fn sync_declare_id_actions(uri: Url, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some("anchor-project-id"))
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("quickfix"))
                .and_then(|value| value.as_str())
                == Some(project_identity::SYNC_DECLARE_ID_QUICKFIX)
        })
        .filter_map(|diagnostic| {
            let expected = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("anchorTomlProgramId"))
                .and_then(|value| value.as_str())?;
            let mut changes = HashMap::new();
            changes.insert(
                uri.clone(),
                vec![snippet_text_edit(diagnostic.range, expected)],
            );

            Some(CodeAction {
                title: "Sync declare_id! with Anchor.toml".to_string(),
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
                    "anchorAction": "sync-declare-id",
                    "programId": expected,
                })),
            })
        })
        .collect()
}
