mod account_aliases;
pub mod account_constraints;
mod account_fields;
mod account_paths;
mod constraint_values;
mod cursor_context;
mod handler_values;
mod instruction_attributes;
#[cfg(test)]
mod proptest_support;
mod ranking;

use {
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::{
        CompletionItem, CompletionItemKind, Documentation, InsertTextFormat, MarkupContent,
        MarkupKind, Position,
    },
};

pub use cursor_context::{completion_signature, CompletionSignature};
#[cfg(test)]
pub use cursor_context::{should_offer_completion, CompletionSignatureKind};
pub(crate) use cursor_context::{
    AccountConstraintCompletionContext, CursorContext, CursorContextKind, ResolvedCursorContext,
};

const INIT_CONSTRAINT_KEY: &str = "init";
const INIT_IF_NEEDED_CONSTRAINT_KEY: &str = "init_if_needed";
const PAYER_CONSTRAINT_KEY: &str = "payer";
const SPACE_CONSTRAINT_KEY: &str = "space";
const PAYER_CONSTRAINT_LABEL: &str = "payer =";
const SPACE_CONSTRAINT_LABEL: &str = "space =";
const INIT_PAYER_RANK: u8 = 0;
const INIT_SPACE_RANK: u8 = 1;
const SNIPPET_RANK: u8 = 10;
const DEFAULT_CONSTRAINT_RANK: u8 = 20;

#[cfg(test)]
pub fn completions(document: &ParsedDocument, position: Position) -> Option<Vec<CompletionItem>> {
    completions_with_workspace(document, position, None)
}

pub fn completions_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Vec<CompletionItem>> {
    let cursor_context = CursorContext::classify_document(document, position);
    let mut items = match cursor_context.kind() {
        CursorContextKind::ContextType { prefix } => {
            context_type_completions(document, workspace_index, prefix)
        }
        CursorContextKind::InstructionAttribute { .. } => {
            instruction_attributes::completions(document, position)
        }
        CursorContextKind::AccountPath { .. } => {
            account_paths::completions(document, position, workspace_index)
        }
        CursorContextKind::AccountConstraintValue { .. } => {
            constraint_values::completions_with_workspace(document, position, workspace_index)
        }
        CursorContextKind::AccountConstraintKey { context } => {
            Some(account_constraint_items(context))
        }
        CursorContextKind::AccountsField { .. } => {
            account_fields::completions_with_workspace(document, position, workspace_index)
        }
        CursorContextKind::HandlerValue { prefix } => handler_values::completions(
            document,
            position,
            workspace_index,
            cursor_context.context(),
            prefix,
        ),
        CursorContextKind::NotAnchor => None,
    }?;

    let ranking_context = ranking::RankingContext::new(&cursor_context);
    ranking::rank_completion_items(&ranking_context, &mut items);
    items.sort_by(|left, right| {
        left.sort_text
            .cmp(&right.sort_text)
            .then_with(|| left.label.cmp(&right.label))
    });
    Some(items)
}

pub fn resolve(mut item: CompletionItem) -> CompletionItem {
    if item.documentation.is_none() {
        let key = item
            .data
            .as_ref()
            .and_then(|data| data.get("anchorCompletion"))
            .and_then(|value| value.as_str())
            .unwrap_or(&item.label);
        let spec = account_constraints::CONSTRAINTS
            .iter()
            .find(|spec| {
                spec.label == key
                    || spec
                        .label
                        .strip_suffix(" =")
                        .is_some_and(|label| label == key.trim_end_matches(" ="))
            })
            .or_else(|| {
                account_constraints::CONSTRAINTS.iter().find(|spec| {
                    spec.label == item.label
                        || spec
                            .label
                            .strip_suffix(" =")
                            .is_some_and(|label| label == item.label.trim_end_matches(" ="))
                })
            });
        if let Some(spec) = spec {
            item.documentation = Some(Documentation::MarkupContent(MarkupContent {
                kind: MarkupKind::Markdown,
                value: crate::constraint_catalog::markdown_doc(spec),
            }));
        }
    }
    item
}

fn context_type_completions(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    prefix: &str,
) -> Option<Vec<CompletionItem>> {
    let mut candidates = document
        .symbols()
        .accounts_structs
        .keys()
        .cloned()
        .chain(
            workspace_index
                .map(WorkspaceIndex::accounts_struct_names)
                .unwrap_or_default(),
        )
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();

    let mut items = candidates
        .into_iter()
        .filter(|name| {
            name.to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
        })
        .map(|name| CompletionItem {
            label: name.clone(),
            kind: Some(CompletionItemKind::STRUCT),
            detail: Some("Anchor accounts context".to_string()),
            sort_text: Some(format!("000_anchor_context_{name}")),
            data: Some(serde_json::json!({
                "anchorCompletion": "context-type",
                "contextType": name,
            })),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    (!items.is_empty()).then_some({
        items.sort_by(|left, right| left.label.cmp(&right.label));
        items
    })
}

fn snippet_items() -> Vec<CompletionItem> {
    [
        (
            "init account",
            "init, payer = ${1:user}, space = 8 + ${2:Account}::INIT_SPACE",
            "Initialize an account with payer and space.",
        ),
        (
            "seeds bump",
            "seeds = [${1:seed}], bump",
            "PDA seeds and bump.",
        ),
        ("mut", "mut", "Mark this account mutable."),
        (
            "has_one",
            "has_one = ${1:authority}",
            "Require stored key match.",
        ),
    ]
    .into_iter()
    .map(|(label, insert_text, detail)| CompletionItem {
        label: label.to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some(detail.to_string()),
        insert_text: Some(insert_text.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some(format!("{SNIPPET_RANK:03}_anchor_{label}")),
        preselect: Some(label == "init account"),
        data: Some(serde_json::json!({ "anchorCompletion": label })),
        ..CompletionItem::default()
    })
    .collect()
}

fn completion_matches_prefix(item: &CompletionItem, prefix: &str) -> bool {
    item.label
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
        || item.insert_text.as_deref().is_some_and(|text| {
            text.to_ascii_lowercase()
                .starts_with(&prefix.to_ascii_lowercase())
        })
}

fn constraint_matches_prefix(label: &str, prefix: &str) -> bool {
    let key = label.trim_end_matches(" =");
    key.to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn account_constraint_items(context: &AccountConstraintCompletionContext) -> Vec<CompletionItem> {
    let mut items = snippet_items()
        .into_iter()
        .filter(|item| completion_matches_prefix(item, &context.prefix))
        .chain(
            account_constraints::CONSTRAINTS
                .iter()
                .filter(|spec| constraint_matches_prefix(spec.label, &context.prefix))
                .map(|spec| account_constraint_item(spec, context)),
        )
        .collect::<Vec<_>>();

    items.sort_by_key(|item| {
        (
            account_constraint_rank(item.label.as_str(), context),
            item.label.clone(),
        )
    });
    items
}

fn account_constraint_item(
    spec: &crate::constraint_catalog::ConstraintSpec,
    context: &AccountConstraintCompletionContext,
) -> CompletionItem {
    let rank = account_constraint_rank(spec.label, context);
    CompletionItem {
        label: spec.label.to_string(),
        kind: Some(CompletionItemKind::KEYWORD),
        detail: Some(spec.detail.to_string()),
        sort_text: Some(format!("{rank:03}_anchor_{}", spec.label)),
        data: Some(account_constraint_data(spec.label, context)),
        ..CompletionItem::default()
    }
}

fn account_constraint_data(
    label: &str,
    context: &AccountConstraintCompletionContext,
) -> serde_json::Value {
    let mut data = serde_json::json!({ "anchorCompletion": label });
    if let serde_json::Value::Object(object) = &mut data {
        if let Some(account_field) = context.account_field.as_deref() {
            object.insert(
                "accountField".to_string(),
                serde_json::Value::String(account_field.to_string()),
            );
        }
        if is_pending_fix_constraint(label, context) {
            object.insert(
                "pendingFixTarget".to_string(),
                serde_json::Value::Bool(true),
            );
        }
    }
    data
}

fn is_pending_fix_constraint(label: &str, context: &AccountConstraintCompletionContext) -> bool {
    context.has_init_constraint
        && ((label == PAYER_CONSTRAINT_LABEL && !context.has_payer_constraint)
            || (label == SPACE_CONSTRAINT_LABEL && !context.has_space_constraint))
}

fn account_constraint_rank(label: &str, context: &AccountConstraintCompletionContext) -> u8 {
    if context.has_init_constraint
        && !context.has_payer_constraint
        && label == PAYER_CONSTRAINT_LABEL
    {
        return INIT_PAYER_RANK;
    }
    if context.has_init_constraint
        && !context.has_space_constraint
        && label == SPACE_CONSTRAINT_LABEL
    {
        return INIT_SPACE_RANK;
    }
    if label.ends_with(" =") {
        return DEFAULT_CONSTRAINT_RANK;
    }
    SNIPPET_RANK
}

#[cfg(test)]
mod tests;
