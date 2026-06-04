mod accounts;
pub(crate) mod common;
mod constraint_expressions;
mod constraints;
mod features;
mod init_constraints;
mod instructions;
mod missing_init;
mod pda;
mod security;

pub use common::edit_distance;
pub(crate) use common::snippet_template_for_edit;

#[cfg(test)]
pub(crate) use common::snippet_text_edit;

use {
    crate::document::ParsedDocument,
    tower_lsp::lsp_types::{CodeAction, Diagnostic, Range, Url},
};

/// Cursor-independent code actions for the document.
///
/// Returns every action that any cursor placement could surface from sub-modules
/// whose output does not depend on the cursor (`accounts`, `instructions`, `security`,
/// `features`, `constraints`, the per-account `missing_init` builder, and `pda`).
///
/// Safe to cache per-URI per-publish-epoch. The cursor-dependent sub-modules
/// (`init_constraints` and `missing_init::fix_all_code_actions`) must be assembled
/// separately at read time via [`cursor_dependent_code_actions`].
pub fn code_actions_unfiltered(
    document: &ParsedDocument,
    uri: Url,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let neutral = Range::default();
    let mut actions = missing_init::code_actions(document, uri.clone(), neutral, diagnostics);
    actions.extend(accounts::code_actions(
        document,
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(instructions::code_actions(
        document,
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(security::code_actions(
        document,
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(features::code_actions(
        document,
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(constraints::code_actions(
        document,
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(constraint_expressions::code_actions(
        uri.clone(),
        neutral,
        diagnostics,
    ));
    actions.extend(pda::code_actions(document, uri, neutral, diagnostics));
    actions
}

/// Code actions whose construction depends on the cursor position.
///
/// `init_constraints` keys its single action off the line under the cursor;
/// `missing_init::fix_all_code_actions` only fires when the cursor touches a
/// missing-init diagnostic. These must be recomputed per request because the
/// cache holds only cursor-independent output.
pub fn cursor_dependent_code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = init_constraints::code_actions(document, uri.clone(), range, diagnostics);
    actions.extend(missing_init::fix_all_code_actions(
        document,
        uri,
        range,
        diagnostics,
    ));
    actions
}

/// Canonical composition: cursor-independent build + cursor-dependent build,
/// then ranked and filtered for the cursor.
///
/// The LSP handler unrolls this composition so it can cache the unfiltered
/// step; test callers keep this wrapper around for whole-action flow assertions.
#[cfg(test)]
pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = code_actions_unfiltered(document, uri.clone(), diagnostics);
    actions.extend(cursor_dependent_code_actions(
        document,
        uri,
        range,
        diagnostics,
    ));
    rank_and_filter_for_cursor(actions, range)
}

/// Sort and (when the cursor is a point inside a diagnostic) filter an action
/// vector by cursor proximity.
///
/// When the cursor is a point that touches at least one action's attached
/// diagnostic, actions whose diagnostic does not touch the cursor are dropped.
/// Otherwise all actions are retained and only ranked. This preserves the
/// tight-lightbulb UX of the prior wrapper without filtering input diagnostics
/// before action construction.
pub fn rank_and_filter_for_cursor(mut actions: Vec<CodeAction>, cursor: Range) -> Vec<CodeAction> {
    actions.sort_by_key(|action| action_distance_key(action, cursor));
    if !common::is_point_range(cursor) {
        return actions;
    }
    let touches_cursor = |action: &CodeAction| -> bool {
        action
            .diagnostics
            .as_ref()
            .and_then(|attached| attached.first())
            .is_some_and(|diagnostic| common::diagnostic_touches_range(diagnostic, cursor))
    };
    if !actions.iter().any(touches_cursor) {
        return actions;
    }
    actions.retain(|action| {
        // Keep actions without an attached diagnostic (e.g. document-wide fix-all)
        // and actions whose diagnostic touches the cursor.
        action
            .diagnostics
            .as_ref()
            .and_then(|attached| attached.first())
            .is_none_or(|diagnostic| common::diagnostic_touches_range(diagnostic, cursor))
    });
    actions
}

fn action_distance_key(action: &CodeAction, cursor: Range) -> (u64, u8) {
    if let Some(diagnostic) = action
        .diagnostics
        .as_ref()
        .and_then(|attached| attached.first())
    {
        return (common::range_distance(diagnostic.range, cursor), 0);
    }

    proactive_assist_range(action)
        .map(|range| (common::range_distance(range, cursor), 1))
        .unwrap_or((u64::MAX, 2))
}

fn proactive_assist_range(action: &CodeAction) -> Option<Range> {
    action
        .data
        .as_ref()
        .and_then(|data| data.get("seagrassAssistRange"))
        .and_then(|range| serde_json::from_value(range.clone()).ok())
}

pub fn resolve(
    document: &ParsedDocument,
    uri: Url,
    action: CodeAction,
    diagnostics: &[Diagnostic],
) -> CodeAction {
    missing_init::resolve(document, uri, action, diagnostics)
}

#[cfg(test)]
mod tests;
