use {
    crate::{
        document::{ParsedDocument, SymbolRange},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind},
};

pub(super) fn expression_scope_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
) -> Vec<CompletionItem> {
    let mut items = accounts
        .fields
        .iter()
        .map(|field| CompletionItem {
            label: field.name.clone(),
            kind: Some(CompletionItemKind::FIELD),
            detail: Some("Anchor account field".to_string()),
            sort_text: Some(format!("000_anchor_expr_account_{}", field.name)),
            preselect: Some(true),
            data: Some(serde_json::json!({
                "anchorCompletion": "constraint-expression-value",
                "constraintExpressionValueKind": "account-field",
            })),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    items.extend(instruction_argument_items(
        document,
        workspace_index,
        accounts,
    ));
    items.extend(
        document
            .symbols()
            .constants
            .iter()
            .map(|constant| CompletionItem {
                label: constant.name.clone(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some("Rust const in scope".to_string()),
                sort_text: Some(format!("020_anchor_expr_const_{}", constant.name)),
                data: Some(serde_json::json!({
                    "anchorCompletion": "constraint-expression-value",
                    "constraintExpressionValueKind": "const",
                })),
                ..CompletionItem::default()
            }),
    );

    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

fn instruction_argument_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
) -> Vec<CompletionItem> {
    let mut items = document
        .symbols()
        .instructions
        .iter()
        .filter(|instruction| {
            instruction
                .context
                .as_ref()
                .is_some_and(|context| context.name == accounts.name)
        })
        .flat_map(|instruction| instruction.arguments.iter())
        .map(|argument| CompletionItem {
            label: argument.name.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some("Instruction argument".to_string()),
            sort_text: Some(format!(
                "010_anchor_expr_instruction_arg_{:04}_{:04}_{}",
                argument.range.start.line, argument.range.start.character, argument.name
            )),
            data: Some(serde_json::json!({
                "anchorCompletion": "constraint-expression-value",
                "constraintExpressionValueKind": "instruction-argument",
            })),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    items.extend(
        workspace_index
            .map(|index| index.instruction_argument_names_for_context(&accounts.name))
            .unwrap_or_default()
            .into_iter()
            .map(|argument_name| CompletionItem {
                label: argument_name,
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some("Workspace instruction argument".to_string()),
                sort_text: Some("011_anchor_expr_workspace_instruction_arg".to_string()),
                data: Some(serde_json::json!({
                    "anchorCompletion": "constraint-expression-value",
                    "constraintExpressionValueKind": "instruction-argument",
                })),
                ..CompletionItem::default()
            }),
    );

    items
}
