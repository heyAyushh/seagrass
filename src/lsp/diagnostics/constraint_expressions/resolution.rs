use {
    crate::{
        document::ParsedDocument, evidence::AccountSetEvidence,
        lsp::scope::is_const_like_identifier, solana::runtime_catalog, workspace::WorkspaceIndex,
    },
    std::collections::BTreeSet,
    syn::ExprPath,
    tower_lsp::lsp_types::SymbolKind,
};

const PATH_SEPARATOR: &str = "::";
const DECLARED_PROGRAM_ID_VALUE: &str = "ID";
const ASSOCIATED_VALUE_PATH_SEGMENTS: usize = 2;
const LOCAL_PATH_ROOTS: &[&str] = &["crate", "self", "super"];
const BUILTIN_ASSOCIATED_PATH_ROOTS: &[&str] = &[
    "None", "Option", "Pubkey", "Some", "System", "Sysvar", "Vec", "bool", "core", "i8", "i16",
    "i32", "i64", "i128", "isize", "std", "u8", "u16", "u32", "u64", "u128", "usize",
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

pub(super) fn unresolved_call_identifier(
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
        [identifier] if call_identifier_resolves(document, accounts, identifier) => None,
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
        || is_builtin_associated_path_root(identifier)
}

fn call_identifier_resolves(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    identifier: &str,
) -> bool {
    identifier_resolves(document, accounts, identifier)
        || document_has_imported_name(document, identifier)
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
        return local_path_value_resolves(document, workspace_index, segments);
    }
    if document_has_imported_name(document, first) || is_builtin_associated_path_root(first) {
        return true;
    }
    segments.len() == ASSOCIATED_VALUE_PATH_SEGMENTS
        && associated_path_value_resolves(document, workspace_index, segments)
}

fn is_builtin_associated_path_root(identifier: &str) -> bool {
    BUILTIN_ASSOCIATED_PATH_ROOTS.contains(&identifier)
        || runtime_catalog::by_type_ident(identifier).is_some()
}

/// Resolve a `crate::…` / `self::…` / `super::…` qualified path.
///
/// Three resolution strategies are attempted in order:
///
/// 1. **Same-file value item** — the leaf symbol is declared in this file.
/// 2. **Declared program-id constant** — the path ends in `ID` and this file
///    has a `declare_id!`.
/// 3. **Same-file associated value** — `Type::CONST` where `Type` is in this
///    file (checked without workspace context to avoid widening scope).
/// 4. **Trie-based cross-file lookup** — the workspace module-path trie maps
///    the module prefix to the file that declares the leaf symbol; this handles
///    paths like `crate::state::Escrow::INIT_SPACE` where `Escrow` lives in
///    `src/state.rs`.
fn local_path_value_resolves(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    segments: &[String],
) -> bool {
    let Some(name) = segments.last().map(String::as_str) else {
        return false;
    };
    document_has_value_item(document, name)
        || (name == DECLARED_PROGRAM_ID_VALUE && document.symbols().declared_program_id.is_some())
        || associated_path_value_resolves(document, None, segments)
        // Cross-file resolution: the trie maps module-path prefixes to the
        // file that declares the symbol, resolving `crate::module::Symbol`
        // paths without emitting a false-positive absence claim.
        || workspace_index.is_some_and(|index| index.symbol_exists_at_qualified_path(segments))
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
