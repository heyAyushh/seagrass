use {
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    tower_lsp::lsp_types::{CompletionItem, Position},
};

mod associated_values;
mod candidates;
mod error_annotations;
mod expression_scope;
mod members;
mod module_paths;
mod program_ids;
mod recovery;
mod seed_expressions;
mod seeds;
mod slots;
mod space_values;

use {
    candidates::{filter_prefix_for_slot, value_items_for_slot},
    recovery::{
        accounts_struct_at_position, field_after_attribute, recover_accounts_struct_at_position,
    },
    slots::{
        attach_slot_data, completion_matches_value_prefix, constraint_value_context_for_cursor,
    },
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
    if let Some(error_prefix) =
        error_annotations::error_annotation_prefix(document.source(), position)
    {
        let items = error_annotations::error_variant_items(document)
            .into_iter()
            .filter(|item| completion_matches_value_prefix(item, &error_prefix))
            .collect::<Vec<_>>();
        return (!items.is_empty()).then_some(items);
    }

    let context = constraint_value_context_for_cursor(
        document.source(),
        position,
        document.account_attribute_cursor(position),
    )?;
    let recovered_accounts;
    let accounts = match accounts_struct_at_position(document, position) {
        Some(accounts) => accounts,
        None => {
            recovered_accounts = recover_accounts_struct_at_position(document, position)?;
            &recovered_accounts
        }
    };
    let current_field = field_after_attribute(accounts, position.line);

    let items = value_items_for_slot(
        document,
        workspace_index,
        accounts,
        current_field,
        context.slot,
        &context.prefix,
    )
    .into_iter()
    .map(|item| attach_slot_data(item, context.slot))
    .filter(|item| {
        completion_matches_value_prefix(
            item,
            filter_prefix_for_slot(document, context.slot, &context.prefix),
        )
    })
    .collect::<Vec<_>>();

    (!items.is_empty()).then_some(items)
}

#[cfg(test)]
mod tests;
