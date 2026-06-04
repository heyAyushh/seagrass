use crate::document::{self, ParsedDocument};
use salsa::Database;
use std::sync::Arc;
use tower_lsp::lsp_types::DocumentSymbol;

#[salsa::db]
pub trait LspDatabase: Database + Send {
    fn document_symbols(&self, source: Arc<str>, version: i32) -> Vec<DocumentSymbol>;
}

#[salsa::tracked]
pub fn document_symbols(
    db: &dyn LspDatabase,
    source: Arc<str>,
    version: i32,
) -> Vec<DocumentSymbol> {
    let _ = (db, version);
    let parsed = ParsedDocument::parse_or_empty(source.to_string());
    document::document_symbols(&parsed)
}

#[salsa::db]
#[derive(Clone, Default)]
pub struct LspSalsaDb {
    storage: salsa::Storage<Self>,
}

impl std::fmt::Debug for LspSalsaDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspSalsaDb").finish_non_exhaustive()
    }
}

#[salsa::db]
impl LspDatabase for LspSalsaDb {
    fn document_symbols(&self, source: Arc<str>, version: i32) -> Vec<DocumentSymbol> {
        document_symbols(self, source, version)
    }
}

#[salsa::db]
impl Database for LspSalsaDb {}

impl LspSalsaDb {}
