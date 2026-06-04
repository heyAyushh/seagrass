use {super::*, crate::document::ParsedDocument};

fn accounts_source(use_decls: &str, declare_id: &str, address_prefix: &str) -> String {
    format!(
        r#"
{use_decls}
{declare_id}
#[derive(Accounts)]
pub struct Run<'info> {{
    #[account(address = {address_prefix})]
    pub mint: AccountInfo<'info>,
}}
"#
    )
}

#[test]
fn completes_program_id_for_self_imported_module() {
    let source = accounts_source(
        "use anchor_spl::token::{self, Mint, Token, TokenAccount};",
        "",
        "token::",
    );
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(&document, position_after(&source, "address = token::"))
        .expect("module path completions for self-imported module");
    assert!(items.iter().any(|item| item.label == "ID"));
}

#[test]
fn completes_program_id_for_external_crate_module() {
    let source = accounts_source("use mpl_token_metadata;", "", "mpl_token_metadata::");
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(
        &document,
        position_after(&source, "address = mpl_token_metadata::"),
    )
    .expect("module path completions for external crate");
    assert!(items.iter().any(|item| item.label == "ID"));
}

#[test]
fn completes_program_id_for_nested_module_path() {
    let source = accounts_source(
        "use anchor_spl::token::{self, Token};",
        "",
        "anchor_spl::token::",
    );
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(
        &document,
        position_after(&source, "address = anchor_spl::token::"),
    )
    .expect("module path completions for nested module path");
    assert!(items.iter().any(|item| item.label == "ID"));
}

#[test]
fn filters_program_id_by_typed_prefix() {
    let source = accounts_source("use anchor_spl::token::{self, Token};", "", "token::I");
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(&document, position_after(&source, "address = token::I"))
        .expect("prefix-filtered module path completions");
    // `token::I` matches both the `ID` const and the `id()` accessor.
    assert!(items.iter().any(|item| item.label == "ID"));
    assert!(items
        .iter()
        .all(|item| item.label.eq_ignore_ascii_case("id") || item.label == "id()"));
}

#[test]
fn completes_program_id_function_form() {
    let source = accounts_source("use anchor_spl::token::{self, Token};", "", "token::");
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(&document, position_after(&source, "address = token::"))
        .expect("module path completions");
    assert!(items.iter().any(|item| item.label == "id()"));
    assert!(items.iter().any(|item| item.label == "ID"));
}

#[test]
fn completes_program_id_for_crate_relative_path_with_declared_id() {
    let source = accounts_source(
        "",
        r#"declare_id!("11111111111111111111111111111111");"#,
        "crate::",
    );
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(&document, position_after(&source, "address = crate::"))
        .expect("crate-relative completions with declared program id");
    assert!(items.iter().any(|item| item.label == "ID"));
}

#[test]
fn omits_program_id_for_crate_relative_path_without_declared_id() {
    let source = accounts_source("", "", "crate::");
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(&document, position_after(&source, "address = crate::"));
    let has_program_id = items
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|item| item.label == "ID");
    assert!(!has_program_id);
}

#[test]
fn omits_program_id_for_unimported_module() {
    let source = accounts_source("", "", "unknown_module::");
    let document = ParsedDocument::parse(&source).unwrap();

    let items = completions(
        &document,
        position_after(&source, "address = unknown_module::"),
    );
    assert!(items.is_none() || items.unwrap().iter().all(|item| item.label != "ID"));
}
