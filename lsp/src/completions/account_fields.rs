use {
    crate::{
        anchor_types::{self, AnchorFieldCompletion, AnchorFieldCompletionKind},
        document::ParsedDocument,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{
        CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent,
        MarkupKind, Position,
    },
};

#[path = "account_fields/context.rs"]
mod context;

use super::ranking;
use context::{
    account_field_at_position, account_field_name_at_position, account_struct_at_position,
    field_completion_context, is_in_accounts_struct, matches_prefix,
    prefix_is_specific_generic_query, prefix_starts_like_rust_type,
    semantic_inner_type_for_account_field_name, semantic_inner_type_for_field,
    semantic_inner_type_from_source_context, FieldCompletionContext,
};

#[cfg(test)]
pub fn completions(document: &ParsedDocument, position: Position) -> Option<Vec<CompletionItem>> {
    completions_with_workspace(document, position, None)
}

pub fn completions_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Vec<CompletionItem>> {
    if !is_in_accounts_struct(document, position) {
        return None;
    }
    let context = field_completion_context(document.source(), position)?;
    let current_accounts = account_struct_at_position(document, position);
    let current_field = current_accounts
        .and_then(|accounts| account_field_at_position(document, accounts, position));
    let current_field_name = current_field
        .as_ref()
        .map(|field| field.name.clone())
        .or_else(|| account_field_name_at_position(document, position));
    let semantic_inner_type = current_field
        .as_ref()
        .and_then(|field| semantic_inner_type_for_field(document, field));
    let semantic_inner_type = semantic_inner_type.or_else(|| {
        current_accounts
            .zip(current_field_name.as_deref())
            .and_then(|(accounts, field_name)| {
                semantic_inner_type_for_account_field_name(accounts, field_name)
            })
    });
    let semantic_inner_type = semantic_inner_type.or_else(|| {
        current_field_name.as_deref().and_then(|field_name| {
            semantic_inner_type_from_source_context(document.source(), position, field_name)
        })
    });
    let mut items = match &context {
        FieldCompletionContext::TypeArgument { container, prefix } => type_argument_items(
            document,
            workspace_index,
            container,
            prefix,
            current_field_name.as_deref(),
            semantic_inner_type,
        ),
        FieldCompletionContext::FieldName { prefix } => anchor_types::field_completions()
            .iter()
            .filter(|completion| {
                completion_field_name(completion)
                    .is_some_and(|name| prefix.is_empty() || matches_prefix(name, prefix))
            })
            .map(|completion| completion_item(completion, &context))
            .collect::<Vec<_>>(),
        FieldCompletionContext::FieldType { prefix } => anchor_types::field_completions()
            .iter()
            .filter(|completion| {
                prefix.is_empty()
                    || (prefix_starts_like_rust_type(prefix)
                        && matches_prefix(completion.label, prefix))
            })
            .map(|completion| completion_item(completion, &context))
            .collect::<Vec<_>>(),
    };

    if let FieldCompletionContext::FieldType { .. } = &context {
        rank_field_type_items_by_role(&mut items, current_field_name.as_deref());
    }

    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items.dedup_by(|left, right| left.label == right.label);
    (!items.is_empty()).then_some(items)
}

fn completion_item(
    completion: &AnchorFieldCompletion,
    context: &FieldCompletionContext,
) -> CompletionItem {
    let (label, insert_text, kind) = match context {
        FieldCompletionContext::FieldName { .. } => (
            completion.field_label,
            completion.field_insert_text,
            CompletionItemKind::FIELD,
        ),
        FieldCompletionContext::FieldType { .. } => (
            completion.label,
            completion.insert_text,
            completion_kind(completion.kind),
        ),
        FieldCompletionContext::TypeArgument { .. } => {
            return type_argument_item(completion).unwrap_or_default()
        }
    };

    CompletionItem {
        label: label.to_string(),
        kind: Some(kind),
        detail: Some(format!(
            "{} ({})",
            completion.detail, completion.source_path
        )),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "`{}`\n\n{}\n\nGenerated from `{}`.",
                completion.label, completion.detail, completion.source_path
            ),
        })),
        insert_text: Some(insert_text.to_string()),
        insert_text_format: Some(insert_text_format(insert_text)),
        sort_text: Some(format!(
            "{}_anchor_field_{}",
            sort_prefix(completion.kind, context),
            label
        )),
        data: Some(serde_json::json!({
            "anchorCompletion": "account-field",
            "source": completion.source_path,
            "kind": completion_kind_name(completion.kind),
        })),
        ..CompletionItem::default()
    }
}

fn type_argument_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    container: &str,
    prefix: &str,
    current_field_name: Option<&str>,
    semantic_inner_type: Option<&'static str>,
) -> Vec<CompletionItem> {
    let preferred_generic = semantic_inner_type;
    let mut items = anchor_types::field_completions()
        .iter()
        .filter(|completion| completion_matches_container(completion, container))
        .filter_map(|completion| {
            type_argument_item_with_field_hint(
                completion,
                prefix,
                current_field_name,
                preferred_generic,
            )
        })
        .collect::<Vec<_>>();

    if accepts_account_data_type(container) {
        let mut account_data_types = document
            .symbols()
            .account_data_structs
            .values()
            .map(|symbol| symbol.name.clone())
            .chain(
                workspace_index
                    .map(WorkspaceIndex::account_data_struct_names)
                    .unwrap_or_default(),
            )
            .filter(|name| prefix.is_empty() || matches_prefix(name, prefix))
            .collect::<Vec<_>>();
        account_data_types.sort();
        account_data_types.dedup();

        items.extend(account_data_types.into_iter().map(|name| {
            let type_matched = preferred_generic == Some(name.as_str());
            CompletionItem {
                label: name.clone(),
                kind: Some(CompletionItemKind::STRUCT),
                detail: Some("Anchor account data struct".to_string()),
                insert_text: Some(name.clone()),
                sort_text: Some(format!(
                    "{}_anchor_type_arg_{name}",
                    if preferred_generic.is_some() {
                        "100"
                    } else {
                        "000"
                    }
                )),
                data: Some(serde_json::json!({
                    "anchorCompletion": "account-field-type-argument",
                    "kind": "accountData",
                    "typeMatched": type_matched,
                })),
                ..CompletionItem::default()
            }
        }));
    }

    items
}

fn rank_field_type_items_by_role(items: &mut [CompletionItem], current_field_name: Option<&str>) {
    let preferred_type = current_field_name.and_then(anchor_types::field_type_for_field_name);
    if preferred_type.is_none() {
        return;
    }
    let preferred_type = preferred_type.unwrap_or_default();

    for item in items.iter_mut() {
        if item.label == preferred_type {
            item.preselect = Some(true);
            item.sort_text = Some(format!("000_anchor_field_role_{}", item.label));
            ranking::set_data_bool(item, "typeMatched", true);
        } else if let Some(sort_text) = item.sort_text.clone() {
            item.sort_text = Some(format!("100_{sort_text}"));
        }
    }
}

fn type_argument_item(completion: &AnchorFieldCompletion) -> Option<CompletionItem> {
    type_argument_item_with_field_hint(completion, "", None, None)
}

fn type_argument_item_with_field_hint(
    completion: &AnchorFieldCompletion,
    prefix: &str,
    current_field_name: Option<&str>,
    preferred_generic: Option<&str>,
) -> Option<CompletionItem> {
    let generic = last_generic_argument(completion.label)?;
    let field_match = current_field_name.is_some_and(|field_name| {
        completion_field_name(completion).is_some_and(|generated| generated == field_name)
    }) || preferred_generic == Some(generic);
    let matches_expected_field = field_match
        && (prefix.is_empty()
            || current_field_name.is_some_and(|field_name| {
                matches_prefix(field_name, prefix) || matches_prefix(generic, prefix)
            }));
    let matches_catalog_type = if prefix.is_empty() {
        true
    } else {
        prefix_is_specific_generic_query(prefix) && matches_prefix(generic, prefix)
    };
    if !matches_expected_field && !matches_catalog_type {
        return None;
    }
    Some(CompletionItem {
        label: generic.to_string(),
        kind: Some(completion_kind(completion.kind)),
        detail: Some(format!("{} (`{}`)", completion.detail, completion.label)),
        documentation: Some(Documentation::MarkupContent(MarkupContent {
            kind: MarkupKind::Markdown,
            value: format!(
                "`{}`\n\n{}\n\nGenerated from `{}`.",
                completion.label, completion.detail, completion.source_path
            ),
        })),
        insert_text: Some(generic.to_string()),
        insert_text_format: Some(InsertTextFormat::PLAIN_TEXT),
        sort_text: Some(format!(
            "{}_anchor_type_arg_{}",
            if field_match {
                "000"
            } else {
                non_matching_type_argument_sort_prefix(completion.kind)
            },
            generic
        )),
        preselect: field_match.then_some(true),
        data: Some(serde_json::json!({
            "anchorCompletion": "account-field-type-argument",
            "source": completion.source_path,
            "kind": completion_kind_name(completion.kind),
            "fullType": completion.label,
            "typeMatched": field_match,
        })),
        ..CompletionItem::default()
    })
}

fn non_matching_type_argument_sort_prefix(kind: AnchorFieldCompletionKind) -> &'static str {
    match kind {
        AnchorFieldCompletionKind::Sysvar => "100",
        AnchorFieldCompletionKind::Program => "101",
        AnchorFieldCompletionKind::SplAccount => "102",
        AnchorFieldCompletionKind::AccountType => "103",
    }
}

fn completion_field_name(completion: &AnchorFieldCompletion) -> Option<&str> {
    completion
        .field_label
        .split_once(':')
        .map(|(name, _)| name.trim())
}

fn completion_matches_container(completion: &AnchorFieldCompletion, container: &str) -> bool {
    match container {
        "Sysvar" => {
            completion.kind == AnchorFieldCompletionKind::Sysvar
                && completion.label.starts_with("Sysvar<'info,")
        }
        "Program" | "Interface" => completion.kind == AnchorFieldCompletionKind::Program,
        "Account" => {
            completion.kind == AnchorFieldCompletionKind::SplAccount
                && completion.label.starts_with("Account<'info,")
        }
        "InterfaceAccount" => {
            completion.kind == AnchorFieldCompletionKind::SplAccount
                && completion.label.starts_with("InterfaceAccount<'info,")
        }
        _ => false,
    }
}

fn accepts_account_data_type(container: &str) -> bool {
    matches!(
        container,
        "Account" | "AccountLoader" | "Box" | "InterfaceAccount" | "Migration"
    )
}

fn last_generic_argument(label: &str) -> Option<&str> {
    let start = label.rfind(',')? + 1;
    let end = label.rfind('>')?;
    (start < end).then(|| label[start..end].trim())
}

fn insert_text_format(insert_text: &str) -> InsertTextFormat {
    if insert_text.contains("${") {
        InsertTextFormat::SNIPPET
    } else {
        InsertTextFormat::PLAIN_TEXT
    }
}

fn completion_kind(kind: AnchorFieldCompletionKind) -> CompletionItemKind {
    match kind {
        AnchorFieldCompletionKind::AccountType => CompletionItemKind::STRUCT,
        AnchorFieldCompletionKind::Program => CompletionItemKind::MODULE,
        AnchorFieldCompletionKind::Sysvar => CompletionItemKind::STRUCT,
        AnchorFieldCompletionKind::SplAccount => CompletionItemKind::STRUCT,
    }
}

fn completion_kind_name(kind: AnchorFieldCompletionKind) -> &'static str {
    match kind {
        AnchorFieldCompletionKind::AccountType => "accountType",
        AnchorFieldCompletionKind::Program => "program",
        AnchorFieldCompletionKind::Sysvar => "sysvar",
        AnchorFieldCompletionKind::SplAccount => "splAccount",
    }
}

fn sort_prefix(kind: AnchorFieldCompletionKind, context: &FieldCompletionContext) -> &'static str {
    match (context, kind) {
        (FieldCompletionContext::FieldName { .. }, AnchorFieldCompletionKind::Sysvar) => "000",
        (FieldCompletionContext::FieldName { .. }, AnchorFieldCompletionKind::Program) => "001",
        (FieldCompletionContext::FieldName { .. }, AnchorFieldCompletionKind::SplAccount) => "002",
        (FieldCompletionContext::FieldName { .. }, AnchorFieldCompletionKind::AccountType) => "003",
        (FieldCompletionContext::FieldType { .. }, AnchorFieldCompletionKind::Sysvar) => "000",
        (FieldCompletionContext::FieldType { .. }, AnchorFieldCompletionKind::Program) => "001",
        (FieldCompletionContext::FieldType { .. }, AnchorFieldCompletionKind::SplAccount) => "002",
        (FieldCompletionContext::FieldType { .. }, AnchorFieldCompletionKind::AccountType) => "003",
        (FieldCompletionContext::TypeArgument { .. }, AnchorFieldCompletionKind::Sysvar) => "000",
        (FieldCompletionContext::TypeArgument { .. }, AnchorFieldCompletionKind::Program) => "001",
        (FieldCompletionContext::TypeArgument { .. }, AnchorFieldCompletionKind::SplAccount) => {
            "002"
        }
        (FieldCompletionContext::TypeArgument { .. }, AnchorFieldCompletionKind::AccountType) => {
            "003"
        }
    }
}

#[cfg(test)]
#[path = "account_fields/tests.rs"]
mod tests;
