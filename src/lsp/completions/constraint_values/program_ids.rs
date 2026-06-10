//! Suggestions for the canonical Solana program / sysvar addresses that humans
//! write on the right-hand side of `address`, `owner`, and `seeds::program`.
//!
//! Each entry is the idiomatic `::ID` path used in official Anchor code, paired
//! with the on-chain address and the official source that defines it. Addresses
//! are documentation only (shown in `detail`); the inserted text is the path so
//! the program keeps resolving through the user's imports.
//!
//! Non-sysvar program addresses are pinned as static entries here. Sysvar entries
//! are generated at call time from [`crate::solana::runtime_catalog::SYSVARS`] so
//! that sysvar address strings have exactly one canonical source.

use {
    crate::solana::runtime_catalog,
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat},
};

/// A well-known non-sysvar program address offered as a ready completion value.
struct WellKnownAddress {
    /// Idiomatic path inserted into the source, e.g. `token::ID`.
    path: &'static str,
    /// On-chain base58 address, shown in `detail` for confirmation.
    address: &'static str,
    /// Human description plus the official crate/module that declares it.
    description: &'static str,
}

/// The non-sysvar program entries: addresses for SPL programs and Metaplex whose
/// strings are not covered by the runtime sysvar catalog.
///
/// Sysvar addresses (`sysvar::rent::ID`, `sysvar::clock::ID`, etc.) are generated
/// from [`runtime_catalog::SYSVARS`] by [`sysvar_address_items`] to keep them in
/// sync with the verified catalog.
const NON_SYSVAR_ADDRESSES: &[WellKnownAddress] = &[
    // anchor_lang::system_program::ID -> solana_sdk_ids::system_program (declare_id!).
    WellKnownAddress {
        path: "system_program::ID",
        address: "11111111111111111111111111111111",
        description: "System Program — anchor_lang::system_program::ID",
    },
    // anchor_spl::token::ID re-exports spl_token::ID (spl/src/token.rs).
    WellKnownAddress {
        path: "token::ID",
        address: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        description: "SPL Token program — anchor_spl::token::ID",
    },
    // anchor_spl::token_2022::ID re-exports spl_token_2022::ID (spl/src/token_2022.rs).
    WellKnownAddress {
        path: "token_2022::ID",
        address: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
        description: "SPL Token-2022 program — anchor_spl::token_2022::ID",
    },
    // anchor_spl::associated_token::ID re-exports the ATA program ID (spl/src/associated_token.rs).
    WellKnownAddress {
        path: "associated_token::ID",
        address: "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL",
        description: "Associated Token program — anchor_spl::associated_token::ID",
    },
    // metaplex-foundation/mpl-token-metadata declare_id! (programs/token-metadata/program/src/lib.rs).
    WellKnownAddress {
        path: "mpl_token_metadata::ID",
        address: "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
        description: "Metaplex Token Metadata program — mpl_token_metadata::ID",
    },
];

/// The subset of sysvar names offered as `sysvar::<name>::ID` constraint values.
///
/// Only the three sysvars that appear frequently in `address =` constraints are
/// offered here; the full catalog is available when using the `Sysvar<'info, _>`
/// account type. The names must match [`runtime_catalog::SysvarSpec::name`].
const SYSVAR_ID_NAMES: &[&str] = &["rent", "clock", "instructions"];

const PUBKEY_LITERAL_LABEL: &str = "pubkey!(\"…\")";
const PUBKEY_LITERAL_SNIPPET: &str = "pubkey!(\"$1\")";

pub(super) fn well_known_address_items() -> Vec<CompletionItem> {
    let non_sysvar_items = NON_SYSVAR_ADDRESSES
        .iter()
        .enumerate()
        .map(|(rank, address)| non_sysvar_address_item(address, rank));

    let sysvar_items = sysvar_address_items(NON_SYSVAR_ADDRESSES.len());

    let mut items: Vec<CompletionItem> = non_sysvar_items.chain(sysvar_items).collect();
    items.push(pubkey_literal_item());
    items
}

/// Builds completion items for the common sysvar address constants from the
/// verified runtime catalog. The `rank_offset` is added to the sort index so
/// sysvar entries sort after the non-sysvar entries.
fn sysvar_address_items(rank_offset: usize) -> impl Iterator<Item = CompletionItem> {
    SYSVAR_ID_NAMES
        .iter()
        .enumerate()
        .filter_map(move |(rank, sysvar_name)| {
            let spec = runtime_catalog::by_name(sysvar_name)?;
            // The idiomatic Anchor path for a sysvar address constant.
            let path = format!("sysvar::{}::ID", spec.name);
            let description = format!(
                "{} sysvar — solana_program::sysvar::{}::ID",
                spec.type_ident, spec.name
            );
            Some(CompletionItem {
                label: path.clone(),
                kind: Some(CompletionItemKind::CONSTANT),
                detail: Some(format!("{description} ({})", spec.account_id_str)),
                sort_text: Some(format!(
                    "008_anchor_program_id_{:02}_{path}",
                    rank_offset + rank
                )),
                data: Some(serde_json::json!({
                    "anchorCompletion": "well-known-program-id",
                })),
                ..CompletionItem::default()
            })
        })
}

fn non_sysvar_address_item(address: &WellKnownAddress, rank: usize) -> CompletionItem {
    CompletionItem {
        label: address.path.to_string(),
        kind: Some(CompletionItemKind::CONSTANT),
        detail: Some(format!("{} ({})", address.description, address.address)),
        sort_text: Some(format!("008_anchor_program_id_{rank:02}_{}", address.path)),
        data: Some(serde_json::json!({
            "anchorCompletion": "well-known-program-id",
        })),
        ..CompletionItem::default()
    }
}

/// `pubkey!("…")` lets a human paste a base58 address with no import — the
/// idiomatic way to pin an external program that has no crate-level `ID` const.
fn pubkey_literal_item() -> CompletionItem {
    CompletionItem {
        label: PUBKEY_LITERAL_LABEL.to_string(),
        kind: Some(CompletionItemKind::SNIPPET),
        detail: Some("Inline base58 address literal".to_string()),
        insert_text: Some(PUBKEY_LITERAL_SNIPPET.to_string()),
        insert_text_format: Some(InsertTextFormat::SNIPPET),
        sort_text: Some("009_anchor_program_id_pubkey_literal".to_string()),
        data: Some(serde_json::json!({
            "anchorCompletion": "pubkey-literal",
        })),
        ..CompletionItem::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every sysvar entry in the completions list must reference an address string that
    /// comes from the runtime catalog — verifying there is no independent copy.
    #[test]
    fn sysvar_address_completion_strings_match_runtime_catalog() {
        let items = well_known_address_items();

        for sysvar_name in SYSVAR_ID_NAMES {
            let spec = runtime_catalog::by_name(sysvar_name)
                .unwrap_or_else(|| panic!("catalog missing sysvar '{sysvar_name}'"));

            let expected_label = format!("sysvar::{}::ID", spec.name);
            let item = items
                .iter()
                .find(|item| item.label == expected_label)
                .unwrap_or_else(|| panic!("no completion item for '{expected_label}'"));

            // The detail string must embed the canonical address string from the catalog,
            // not an independent copy.
            let detail = item.detail.as_deref().unwrap_or("");
            assert!(
                detail.contains(spec.account_id_str),
                "detail for '{expected_label}' does not contain catalog address '{}'; \
                 got: {detail:?}",
                spec.account_id_str
            );
        }
    }
}
