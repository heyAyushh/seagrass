use {
    crate::{
        document::{AssociatedValueKind, ParsedDocument},
        workspace::{WorkspaceAssociatedValue, WorkspaceIndex},
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, SymbolKind},
};

const ASSOCIATED_PATH_SEPARATOR: &str = "::";
const INIT_SPACE_ASSOCIATED_CONST: &str = "INIT_SPACE";

pub(super) struct AssociatedValuePrefix<'a> {
    pub(super) owner_type: &'a str,
    pub(super) value_prefix: &'a str,
}

pub(super) fn associated_value_prefix(value_prefix: &str) -> Option<AssociatedValuePrefix<'_>> {
    let (owner_type, value_prefix) = value_prefix.rsplit_once(ASSOCIATED_PATH_SEPARATOR)?;
    is_type_path(owner_type).then_some(AssociatedValuePrefix {
        owner_type,
        value_prefix,
    })
}

pub(super) fn associated_value_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    value_prefix: &str,
) -> Vec<CompletionItem> {
    let Some(prefix) = associated_value_prefix(value_prefix) else {
        return Vec::new();
    };
    let mut items = local_associated_value_items(document, prefix.owner_type);
    items.extend(workspace_associated_value_items(
        workspace_index,
        prefix.owner_type,
    ));
    if document
        .symbols()
        .derived_init_space_types
        .contains(prefix.owner_type)
    {
        items.push(associated_const_item(
            INIT_SPACE_ASSOCIATED_CONST,
            "Generated InitSpace associated const",
        ));
    }
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

pub(super) fn filter_prefix(value_prefix: &str) -> Option<&str> {
    associated_value_prefix(value_prefix).map(|prefix| prefix.value_prefix)
}

fn local_associated_value_items(
    document: &ParsedDocument,
    owner_type: &str,
) -> Vec<CompletionItem> {
    document
        .symbols()
        .associated_value_items
        .get(owner_type)
        .into_iter()
        .flat_map(|items| items.iter())
        .map(|item| match item.kind {
            AssociatedValueKind::Constant => associated_const_item(&item.name, "Associated const"),
            AssociatedValueKind::Function => {
                associated_function_item(&item.name, "Associated function")
            }
        })
        .collect()
}

fn workspace_associated_value_items(
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
) -> Vec<CompletionItem> {
    workspace_index
        .into_iter()
        .flat_map(|index| index.associated_values_in_container(owner_type))
        .filter_map(workspace_associated_value_item)
        .collect()
}

fn workspace_associated_value_item(value: WorkspaceAssociatedValue) -> Option<CompletionItem> {
    match value.kind {
        SymbolKind::CONSTANT => Some(associated_const_item(
            &value.name,
            "Workspace associated const",
        )),
        SymbolKind::METHOD | SymbolKind::FUNCTION => Some(associated_function_item(
            &value.name,
            "Workspace associated function",
        )),
        _ => None,
    }
}

fn associated_const_item(name: &str, detail: &str) -> CompletionItem {
    CompletionItem {
        label: name.to_string(),
        kind: Some(CompletionItemKind::CONSTANT),
        detail: Some(detail.to_string()),
        sort_text: Some(format!("000_anchor_associated_value_{name}")),
        preselect: Some(true),
        data: Some(serde_json::json!({
            "anchorCompletion": "associated-value",
            "associatedValueKind": "const",
        })),
        ..CompletionItem::default()
    }
}

fn associated_function_item(name: &str, detail: &str) -> CompletionItem {
    let label = format!("{name}()");
    CompletionItem {
        label,
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(detail.to_string()),
        insert_text: Some(format!("{name}()")),
        sort_text: Some(format!("010_anchor_associated_value_{name}")),
        data: Some(serde_json::json!({
            "anchorCompletion": "associated-value",
            "associatedValueKind": "function",
        })),
        ..CompletionItem::default()
    }
}

fn is_type_path(value: &str) -> bool {
    !value.is_empty()
        && value.split(ASSOCIATED_PATH_SEPARATOR).all(|segment| {
            segment
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_uppercase())
                && segment
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        })
}
