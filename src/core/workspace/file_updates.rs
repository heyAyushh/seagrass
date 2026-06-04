use {
    super::{
        document_indexed_references, document_indexed_symbols, files, indexed_accounts_structs,
        indexed_functions, WorkspaceDocumentUpdate,
    },
    crate::{document::ParsedDocument, file_text},
    tower_lsp::lsp_types::Url,
};

pub(super) fn update_for_workspace_file(
    roots: &[Url],
    uri: Url,
) -> Option<WorkspaceDocumentUpdate> {
    let path = uri.to_file_path().ok()?;
    let root = roots
        .iter()
        .filter_map(|root_uri| root_uri.to_file_path().ok())
        .find(|root_path| path.starts_with(root_path))?;
    if !files::is_anchor_source_file(&root, &path) {
        return None;
    }
    let source = file_text::read_limited_text(&path).ok().flatten()?;
    let document = ParsedDocument::parse_or_empty(source);
    let accounts_structs = indexed_accounts_structs(&document, &uri, false);
    Some(WorkspaceDocumentUpdate {
        uri,
        is_open: false,
        symbols: document_indexed_symbols(&document),
        references: document_indexed_references(&document),
        functions: indexed_functions(&document),
        accounts_structs,
    })
}
