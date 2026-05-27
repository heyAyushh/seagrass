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

use {
    crate::document::ParsedDocument,
    tower_lsp::lsp_types::{CodeAction, Diagnostic, Range, Url},
};

pub fn code_actions(
    document: &ParsedDocument,
    uri: Url,
    range: Range,
    diagnostics: &[Diagnostic],
) -> Vec<CodeAction> {
    let mut actions = init_constraints::code_actions(document, uri.clone(), range, diagnostics);
    actions.extend(missing_init::code_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(accounts::code_actions(document, uri.clone(), diagnostics));
    actions.extend(instructions::code_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(security::code_actions(document, uri.clone(), diagnostics));
    actions.extend(features::code_actions(document, uri.clone(), diagnostics));
    actions.extend(constraints::code_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(missing_init::fix_all_code_actions(
        document,
        uri.clone(),
        diagnostics,
    ));
    actions.extend(pda::code_actions(uri, diagnostics));
    actions
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
