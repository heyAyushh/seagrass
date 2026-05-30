use {
    crate::{document::ParsedDocument, evidence::AccountSetEvidence, workspace::WorkspaceIndex},
    std::collections::BTreeSet,
    syn::ExprPath,
    tower_lsp::lsp_types::SymbolKind,
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
    workspace_index: Option<&WorkspaceIndex>,
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
        _ if path_resolves(document, workspace_index, &segments) => None,
        _ => Some(segments.join(PATH_SEPARATOR)),
    }
}

pub(super) fn identifier_replacement_candidates(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
) -> Vec<String> {
    accounts
        .account_names()
        .chain(accounts.instruction_argument_names())
        .map(str::to_string)
        .chain(
            document
                .symbols()
                .value_items
                .iter()
                .map(|item| item.name.clone()),
        )
        .chain(
            document
                .symbols()
                .imported_names
                .iter()
                .filter(|import| is_const_like_identifier(&import.name))
                .map(|import| import.name.clone()),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
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

fn path_resolves(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    segments: &[String],
) -> bool {
    let Some(first) = segments.first().map(String::as_str) else {
        return false;
    };
    if segments.last().is_none() {
        return false;
    }

    if LOCAL_PATH_ROOTS.contains(&first) {
        return local_path_value_resolves(document, segments);
    }
    if document_has_imported_name(document, first) || BUILTIN_ASSOCIATED_PATH_ROOTS.contains(&first)
    {
        return true;
    }
    associated_path_value_resolves(document, workspace_index, segments)
}

fn local_path_value_resolves(document: &ParsedDocument, segments: &[String]) -> bool {
    let Some(name) = segments.last().map(String::as_str) else {
        return false;
    };
    document_has_value_item(document, name)
        || (name == DECLARED_PROGRAM_ID_VALUE && document.symbols().declared_program_id.is_some())
        || associated_path_value_resolves(document, None, segments)
}

fn associated_path_value_resolves(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    segments: &[String],
) -> bool {
    let Some((owner_type, value_name)) = associated_path_owner_and_value(segments) else {
        return false;
    };
    document
        .symbols()
        .type_has_associated_value(owner_type, value_name)
        || workspace_has_associated_value(workspace_index, owner_type, value_name)
}

fn associated_path_owner_and_value(segments: &[String]) -> Option<(&str, &str)> {
    let value_name = segments.last()?.as_str();
    let owner_type = segments.iter().rev().nth(1)?.as_str();
    document_type_segment(owner_type).then_some((owner_type, value_name))
}

fn document_type_segment(segment: &str) -> bool {
    segment
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn workspace_has_associated_value(
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
    value_name: &str,
) -> bool {
    let Some(index) = workspace_index else {
        return false;
    };
    !index
        .symbol_locations_in_container(
            value_name,
            &[
                SymbolKind::CONSTANT,
                SymbolKind::METHOD,
                SymbolKind::FUNCTION,
            ],
            owner_type,
        )
        .is_empty()
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
