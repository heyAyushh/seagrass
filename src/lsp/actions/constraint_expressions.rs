//! Code actions for semantic Anchor constraint expression diagnostics.

use {
    super::common::{diagnostic_code, edit_distance, single_document_edit, single_text_edit},
    crate::diagnostics::{
        ANCHOR_CONSTRAINT_EXPRESSION_CODE, REPLACE_CONSTRAINT_EXPRESSION_IDENTIFIER_QUICKFIX,
        REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX,
    },
    tower_lsp::lsp_types::{CodeAction, CodeActionKind, Diagnostic, Range, Url},
};

const MAX_MEMBER_REPLACEMENT_DISTANCE: usize = 3;
const MAX_IDENTIFIER_REPLACEMENT_DISTANCE: usize = 3;

pub fn code_actions(uri: Url, _range: Range, diagnostics: &[Diagnostic]) -> Vec<CodeAction> {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_code(diagnostic) == Some(ANCHOR_CONSTRAINT_EXPRESSION_CODE))
        .flat_map(|diagnostic| {
            [
                identifier_replacement_action(uri.clone(), diagnostic),
                member_replacement_action(uri.clone(), diagnostic),
            ]
            .into_iter()
            .flatten()
        })
        .collect()
}

fn identifier_replacement_action(uri: Url, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let data = diagnostic.data.as_ref()?;
    let missing = data.get("identifier").and_then(|value| value.as_str())?;
    let replacement = closest_candidate(data, missing)?;
    let distance = edit_distance(replacement, missing);
    if distance > MAX_IDENTIFIER_REPLACEMENT_DISTANCE {
        return None;
    }

    Some(CodeAction {
        title: format!("Replace `{missing}` with `{replacement}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(single_document_edit(
            uri,
            single_text_edit(diagnostic.range, replacement.to_string()),
        )),
        command: None,
        is_preferred: Some(distance == 1),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "replace-constraint-expression-identifier",
            "quickfix": REPLACE_CONSTRAINT_EXPRESSION_IDENTIFIER_QUICKFIX,
            "replacement": replacement,
        })),
    })
}

fn member_replacement_action(uri: Url, diagnostic: &Diagnostic) -> Option<CodeAction> {
    let data = diagnostic.data.as_ref()?;
    let missing = data.get("field").and_then(|value| value.as_str())?;
    let replacement = closest_candidate(data, missing)?;
    let distance = edit_distance(replacement, missing);
    if distance > MAX_MEMBER_REPLACEMENT_DISTANCE {
        return None;
    }

    Some(CodeAction {
        title: format!("Replace `{missing}` with `{replacement}`"),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic.clone()]),
        edit: Some(single_document_edit(
            uri,
            single_text_edit(diagnostic.range, replacement.to_string()),
        )),
        command: None,
        is_preferred: Some(distance == 1),
        disabled: None,
        data: Some(serde_json::json!({
            "anchorAction": "replace-constraint-expression-member",
            "quickfix": REPLACE_CONSTRAINT_EXPRESSION_MEMBER_QUICKFIX,
            "replacement": replacement,
        })),
    })
}

fn closest_candidate<'a>(data: &'a serde_json::Value, missing: &str) -> Option<&'a str> {
    data.get("candidates")
        .and_then(|value| value.as_array())
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .min_by_key(|candidate| edit_distance(candidate, missing))
}
