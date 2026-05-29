use {
    super::{
        code_actions,
        program_fields::{ProgramFieldProvider, SystemProgramFieldProvider},
        proposed_assists, *,
    },
    crate::document::ParsedDocument,
    tower_lsp::lsp_types::{CodeActionKind, Url},
};

fn assist_titles(source: &str) -> Vec<String> {
    let document = ParsedDocument::parse_or_empty(source);
    let uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let range = document
        .symbols()
        .accounts_structs
        .values()
        .next()
        .map(|accounts| accounts.selection_range)
        .unwrap_or_default();
    code_actions(&document, uri, range)
        .into_iter()
        .map(|action| action.title)
        .collect()
}

mod canonical_cpi;
mod program_fields;
mod structural;
mod surfaces;
