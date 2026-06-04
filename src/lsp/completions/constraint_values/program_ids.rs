//! Suggestions for the canonical Solana program / sysvar addresses that humans
//! write on the right-hand side of `address`, `owner`, and `seeds::program`.
//!
//! Each entry is the idiomatic `::ID` path used in official Anchor code, paired
//! with the on-chain address and the official source that defines it. Addresses
//! are documentation only (shown in `detail`); the inserted text is the path so
//! the program keeps resolving through the user's imports.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, InsertTextFormat};

/// A well-known program/sysvar address offered as a ready value.
struct WellKnownAddress {
    /// Idiomatic path inserted into the source, e.g. `token::ID`.
    path: &'static str,
    /// On-chain base58 address, shown in `detail` for confirmation.
    address: &'static str,
    /// Human description plus the official crate/module that declares it.
    description: &'static str,
}

/// The core set: the addresses proven high-frequency in official Anchor/Solana
/// code. Sources are cited per entry so the catalog can be audited.
const WELL_KNOWN_ADDRESSES: &[WellKnownAddress] = &[
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
    // anchor_lang::solana_program::sysvar::rent::ID (solana_sdk_ids::sysvar::rent).
    WellKnownAddress {
        path: "sysvar::rent::ID",
        address: "SysvarRent111111111111111111111111111111111",
        description: "Rent sysvar — solana_program::sysvar::rent::ID",
    },
    // anchor_lang::solana_program::sysvar::clock::ID (solana_sdk_ids::sysvar::clock).
    WellKnownAddress {
        path: "sysvar::clock::ID",
        address: "SysvarC1ock11111111111111111111111111111111",
        description: "Clock sysvar — solana_program::sysvar::clock::ID",
    },
    // anchor_lang::solana_program::sysvar::instructions::ID (solana_sdk_ids::sysvar::instructions).
    WellKnownAddress {
        path: "sysvar::instructions::ID",
        address: "Sysvar1nstructions1111111111111111111111111",
        description: "Instructions sysvar — solana_program::sysvar::instructions::ID",
    },
    // metaplex-foundation/mpl-token-metadata declare_id! (programs/token-metadata/program/src/lib.rs).
    WellKnownAddress {
        path: "mpl_token_metadata::ID",
        address: "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s",
        description: "Metaplex Token Metadata program — mpl_token_metadata::ID",
    },
];

const PUBKEY_LITERAL_LABEL: &str = "pubkey!(\"…\")";
const PUBKEY_LITERAL_SNIPPET: &str = "pubkey!(\"$1\")";

pub(super) fn well_known_address_items() -> Vec<CompletionItem> {
    let mut items = WELL_KNOWN_ADDRESSES
        .iter()
        .enumerate()
        .map(|(rank, address)| well_known_address_item(address, rank))
        .collect::<Vec<_>>();
    items.push(pubkey_literal_item());
    items
}

fn well_known_address_item(address: &WellKnownAddress, rank: usize) -> CompletionItem {
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
