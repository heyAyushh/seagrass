use crate::document::ParsedDocument;

#[test]
fn has_glob_import_false_for_named_import() {
    let document = ParsedDocument::parse("use crate::state::Escrow;\n").unwrap();

    assert!(!document.symbols().has_glob_import);
}

#[test]
fn has_glob_import_true_for_star_import() {
    let document = ParsedDocument::parse("use crate::state::*;\n").unwrap();

    assert!(document.symbols().has_glob_import);
}

#[test]
fn has_glob_import_true_for_group_containing_star() {
    let document = ParsedDocument::parse("use crate::state::{Escrow, *};\n").unwrap();

    assert!(document.symbols().has_glob_import);
}

#[test]
fn has_local_glob_import_false_for_external_prelude() {
    let document = ParsedDocument::parse("use anchor_lang::prelude::*;\n").unwrap();

    assert!(document.symbols().has_glob_import);
    assert!(!document.symbols().has_local_glob_import);
}

#[test]
fn has_local_glob_import_true_for_declared_module_glob() {
    let document = ParsedDocument::parse("mod state;\nuse state::*;\n").unwrap();

    assert!(document.symbols().has_local_glob_import);
}
