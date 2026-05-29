use {
    crate::{document::ParsedDocument, evidence::AccountSetEvidence},
    syn::ExprPath,
};

const PATH_SEPARATOR: &str = "::";
const DECLARED_PROGRAM_ID_VALUE: &str = "ID";
const LOCAL_PATH_ROOTS: &[&str] = &["crate", "self", "super"];
const BUILTIN_ASSOCIATED_PATH_ROOTS: &[&str] = &[
    "Clock", "None", "Option", "Pubkey", "Rent", "Some", "System", "Sysvar", "Vec", "bool", "core",
    "i8", "i16", "i32", "i64", "i128", "isize", "std", "u8", "u16", "u32", "u64", "u128", "usize",
];

pub(super) fn unresolved_path_identifier(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    path: &ExprPath,
) -> Option<String> {
    if path.qself.is_some() {
        return None;
    }
    let segments = path_segments(path);
    match segments.as_slice() {
        [] => None,
        [identifier] if identifier_resolves(document, accounts, identifier) => None,
        [identifier] => Some(identifier.to_string()),
        _ if path_resolves(document, &segments) => None,
        _ => Some(segments.join(PATH_SEPARATOR)),
    }
}

fn path_segments(path: &ExprPath) -> Vec<String> {
    path.path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn identifier_resolves(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    identifier: &str,
) -> bool {
    accounts.has_account(identifier)
        || accounts.has_instruction_argument(identifier)
        || document_has_value_item(document, identifier)
        || document_has_imported_const_like_name(document, identifier)
        || BUILTIN_ASSOCIATED_PATH_ROOTS.contains(&identifier)
}

fn path_resolves(document: &ParsedDocument, segments: &[String]) -> bool {
    let Some(first) = segments.first().map(String::as_str) else {
        return false;
    };
    let Some(last) = segments.last().map(String::as_str) else {
        return false;
    };

    if LOCAL_PATH_ROOTS.contains(&first) {
        return local_path_value_resolves(document, last);
    }
    document_has_imported_name(document, first)
        || document.symbols().knows_type(first)
        || BUILTIN_ASSOCIATED_PATH_ROOTS.contains(&first)
}

fn local_path_value_resolves(document: &ParsedDocument, name: &str) -> bool {
    document_has_value_item(document, name)
        || (name == DECLARED_PROGRAM_ID_VALUE && document.symbols().declared_program_id.is_some())
}

fn document_has_value_item(document: &ParsedDocument, name: &str) -> bool {
    document
        .symbols()
        .value_items
        .iter()
        .any(|item| item.name == name)
}

fn document_has_imported_const_like_name(document: &ParsedDocument, name: &str) -> bool {
    is_const_like_identifier(name) && document_has_imported_name(document, name)
}

fn document_has_imported_name(document: &ParsedDocument, name: &str) -> bool {
    document
        .symbols()
        .imported_names
        .iter()
        .any(|import| import.name == name)
}

fn is_const_like_identifier(identifier: &str) -> bool {
    identifier.contains('_')
        && identifier.chars().any(|ch| ch.is_ascii_alphabetic())
        && identifier
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}
