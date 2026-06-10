mod applicability;
mod catalog;
mod has_one;
mod init_lifecycle;
mod pda;
mod program_account;
mod support;
mod token;

use {
    crate::{
        document::ParsedDocument,
        evidence::{AccountSetEvidence, EvidenceGraph, FieldEvidence},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::Diagnostic,
};

const CURRENT_DOCUMENT_PLACEHOLDER_URI: &str = "file:///seagrass/current-document.rs";

#[cfg(test)]
pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    collect_with_workspace(document, None)
}

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    EvidenceGraph::from_document(document)
        .account_sets()
        .iter()
        .flat_map(|accounts| {
            accounts.fields().iter().flat_map(move |field| {
                field_diagnostics(document, workspace_index, accounts, field)
            })
        })
        .collect()
}

fn field_diagnostics(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for constraint in field.constraints() {
        diagnostics.extend(pda::constraint_diagnostics(document, field, constraint));
        diagnostics.extend(init_lifecycle::constraint_diagnostics(
            accounts, field, constraint,
        ));
        diagnostics.extend(catalog::diagnostics(document, field, constraint));
        diagnostics.extend(applicability::constraint_diagnostics(
            document, field, constraint,
        ));
        diagnostics.extend(token::constraint_diagnostics(accounts, field, constraint));
        diagnostics.extend(has_one::diagnostic(
            document,
            workspace_index,
            field,
            constraint,
        ));
    }

    diagnostics.extend(program_account::field_diagnostic(document, accounts, field));
    diagnostics.extend(pda::field_diagnostics(accounts, field));

    diagnostics
}

#[cfg(test)]
mod tests;
