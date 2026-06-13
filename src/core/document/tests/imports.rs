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

#[test]
fn import_origin_recorded_for_plain_use() {
    let document = ParsedDocument::parse("use solana_clock::Clock;\n").unwrap();

    assert_eq!(
        document
            .symbols()
            .import_origins
            .get("Clock")
            .map(String::as_str),
        Some("solana_clock")
    );
}

#[test]
fn import_origin_recorded_for_alias() {
    let document = ParsedDocument::parse("use solana_program::clock::Clock as Klock;\n").unwrap();

    assert_eq!(
        document
            .symbols()
            .import_origins
            .get("Klock")
            .map(String::as_str),
        Some("solana_program")
    );
    assert_eq!(
        document
            .symbols()
            .import_aliases
            .get("Klock")
            .map(String::as_str),
        Some("Clock")
    );
}

#[test]
fn import_origin_local_crate_not_external() {
    let document = ParsedDocument::parse("use crate::state::Clock;\n").unwrap();

    assert_eq!(
        document
            .symbols()
            .import_origins
            .get("Clock")
            .map(String::as_str),
        Some("crate")
    );
}
