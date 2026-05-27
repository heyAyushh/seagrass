mod accounts;
mod common;
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
    actions.extend(pda::code_actions(uri, neutral, diagnostics));
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
    let mut actions =
        init_constraints::code_actions(document, uri.clone(), range, diagnostics);
    actions.extend(missing_init::fix_all_code_actions(
        document,
        uri,
        range,
        diagnostics,
    ));
    actions
}

/// Sort and (when the cursor is a point inside a diagnostic) filter an action
/// vector by cursor proximity. The result mirrors what [`code_actions`] returns
/// for the same cursor.
pub fn rank_and_filter_for_cursor(
    mut actions: Vec<CodeAction>,
    cursor: Range,
) -> Vec<CodeAction> {
    common::sort_actions_by_cursor(&mut actions, cursor);
    actions
}

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let ranked_diagnostics = common::ranked_diagnostics_for_range(diagnostics, range);
    let relevant_diagnostics = ranked_diagnostics.as_slice();
    let mut actions = code_actions_unfiltered(document, uri.clone(), relevant_diagnostics);
    actions.extend(cursor_dependent_code_actions(
        document,
        uri,
        range,
        relevant_diagnostics,
    ));
    rank_and_filter_for_cursor(actions, range)
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
#[path = "tests.rs"]
mod tests;
