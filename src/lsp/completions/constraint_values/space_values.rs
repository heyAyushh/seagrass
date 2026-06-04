//! Suggestions for the `space = ...` constraint.
//!
//! Offers the discriminator-plus-data idioms documented by Anchor: the classic
//! `8 + T::INIT_SPACE`, the v0.31+ recommended `T::DISCRIMINATOR.len() +
//! T::INIT_SPACE` (custom discriminators are no longer always 8 bytes), and for
//! zero-copy (`AccountLoader`) accounts the `8 + std::mem::size_of::<T>()` form.

use {
    crate::{account_semantics, document::SymbolRange},
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind},
};

const ANCHOR_DISCRIMINATOR_BYTES: u8 = 8;
const ZERO_COPY_ACCOUNT_WRAPPER: &str = "AccountLoader";

pub(super) fn space_items(
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
) -> Vec<CompletionItem> {
    let Some(field) = current_field else {
        return Vec::new();
    };
    if is_spl_account_space_managed_by_anchor(accounts, field) {
        return Vec::new();
    }
    let Some(account_type) = field.generic_type_names.last() else {
        return Vec::new();
    };

    let mut items = vec![
        space_item(
            format!("{ANCHOR_DISCRIMINATOR_BYTES} + {account_type}::INIT_SPACE"),
            format!("Anchor discriminator plus `{account_type}` init space"),
            0,
        ),
        space_item(
            format!("{account_type}::DISCRIMINATOR.len() + {account_type}::INIT_SPACE"),
            format!("Custom-discriminator-safe space for `{account_type}` (Anchor 0.31+)"),
            1,
        ),
    ];

    if is_zero_copy_account(field) {
        items.push(space_item(
            format!("{ANCHOR_DISCRIMINATOR_BYTES} + std::mem::size_of::<{account_type}>()"),
            format!("Discriminator plus zero-copy `{account_type}` layout size"),
            2,
        ));
    }

    items
}

fn space_item(label: String, detail: String, rank: u8) -> CompletionItem {
    CompletionItem {
        label,
        kind: Some(CompletionItemKind::VALUE),
        detail: Some(detail),
        sort_text: Some(format!("00{rank}_anchor_value_space")),
        preselect: Some(rank == 0),
        ..CompletionItem::default()
    }
}

fn is_zero_copy_account(field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some(ZERO_COPY_ACCOUNT_WRAPPER)
}

fn is_spl_account_space_managed_by_anchor(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    account_semantics::field_has_declared_or_expected_account_inner_type(accounts, field, "Mint")
        || account_semantics::field_has_declared_or_expected_account_inner_type(
            accounts,
            field,
            "TokenAccount",
        )
}
