use crate::anchor_analysis::AnchorAnalysis;
use crate::diagnostics;
use crate::document::{self, ParsedDocument};
use salsa::Database;
use std::sync::Arc;
use tower_lsp::lsp_types::{Diagnostic, DocumentSymbol};

#[salsa::db]
#[allow(dead_code)]
pub trait LspDatabase: Database + Send {
    fn document_symbols(&self, source: Arc<str>, version: i32) -> Vec<DocumentSymbol>;
    fn constraint_diagnostics(&self, source: Arc<str>, version: i32) -> Vec<Diagnostic>;
    fn anchor_analysis(&self, source: Arc<str>, version: i32) -> Arc<AnchorAnalysis>;
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

#[salsa::tracked]
pub fn constraint_diagnostics(
    db: &dyn LspDatabase,
    source: Arc<str>,
    version: i32,
) -> Vec<Diagnostic> {
    let _ = (db, version);
    let parsed = ParsedDocument::parse_or_empty(source.to_string());
    diagnostics::collect_with_workspace(&parsed, None)
}

#[salsa::tracked]
pub fn anchor_analysis(
    db: &dyn LspDatabase,
    source: Arc<str>,
    version: i32,
) -> Arc<AnchorAnalysis> {
    let _ = (db, version);
    AnchorAnalysis::from_source(source)
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

    fn constraint_diagnostics(&self, source: Arc<str>, version: i32) -> Vec<Diagnostic> {
        constraint_diagnostics(self, source, version)
    }

    fn anchor_analysis(&self, source: Arc<str>, version: i32) -> Arc<AnchorAnalysis> {
        anchor_analysis(self, source, version)
    }
}

#[salsa::db]
impl Database for LspSalsaDb {}

impl LspSalsaDb {}
