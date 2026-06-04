//! Suggestions for `seeds = [...]` array elements. Every element must resolve
//! to `&[u8]`, so each candidate carries the idiomatic byte conversion: the
//! current field as a static `b"name"`, sibling account keys, instruction
//! arguments by type, and file-level byte constants.

use {
    super::seed_expressions,
    crate::document::{ParsedDocument, SymbolRange},
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind},
};

const BYTE_SLICE_REF_SUFFIX: &str = ".as_ref()";

pub(super) fn seed_items(
    document: &ParsedDocument,
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
) -> Vec<CompletionItem> {
    let current_field_name = current_field.map(|field| field.name.as_str());
    let mut items = Vec::new();

    if let Some(field) = current_field {
        items.push(CompletionItem {
            label: format!("b\"{}\"", field.name),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("Static PDA seed from the current account field name".to_string()),
            sort_text: Some(format!("020_anchor_seed_static_{}", field.name)),
            ..CompletionItem::default()
        });
    }

    items.extend(
        accounts
            .fields
            .iter()
            .filter(|field| current_field_name != Some(field.name.as_str()))
            .filter(|field| is_account_seed_candidate(field))
            .map(|field| CompletionItem {
                label: format!("{}.key().as_ref()", field.name),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(format!("PDA seed from `{}` account key", field.name)),
                sort_text: Some(format!("000_anchor_seed_account_{}", field.name)),
                ..CompletionItem::default()
            }),
    );

    items.extend(instruction_seed_items(document, accounts));
    items.extend(constant_seed_items(document));
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

fn instruction_seed_items(
    document: &ParsedDocument,
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
        .filter_map(|argument| {
            let (suffix, detail) = seed_expressions::expression_for_argument_type(
                argument.type_name.as_deref(),
                argument.type_signature.as_deref(),
            )?;
            Some(CompletionItem {
                label: format!("{}{}", argument.name, suffix),
                kind: Some(CompletionItemKind::VARIABLE),
                detail: Some(format!(
                    "{detail} from `{}` instruction argument",
                    argument.name
                )),
                sort_text: Some(format!(
                    "010_anchor_seed_argument_{:04}_{:04}_{}",
                    argument.range.start.line, argument.range.start.character, argument.name
                )),
                ..CompletionItem::default()
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

/// File-level constants used as PDA seeds. Seed constants are overwhelmingly
/// byte slices/arrays/`Pubkey`s (e.g. `const MY_SEED: &[u8]`), so we offer the
/// `.as_ref()` form that coerces them to `&[u8]`.
fn constant_seed_items(document: &ParsedDocument) -> Vec<CompletionItem> {
    document
        .symbols()
        .constants
        .iter()
        .map(|constant| CompletionItem {
            label: format!("{}{BYTE_SLICE_REF_SUFFIX}", constant.name),
            kind: Some(CompletionItemKind::CONSTANT),
            detail: Some(format!("PDA seed bytes from `{}` constant", constant.name)),
            sort_text: Some(format!("030_anchor_seed_const_{}", constant.name)),
            ..CompletionItem::default()
        })
        .collect()
}

fn is_account_seed_candidate(field: &SymbolRange) -> bool {
    !field.is_optional
}
