use {
    super::{associated_values, members, slots::ConstraintValueSlot},
    crate::{
        account_semantics::{self, ResolvedAccountType},
        constraint_catalog::ConstraintValueKind,
        document::{ParsedDocument, SymbolRange},
        workspace::WorkspaceIndex,
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{CompletionItem, CompletionItemKind},
};

const DIRECT_ACCOUNT_COMPLETION_VARIANT_RANK: u8 = 0;
const ACCOUNT_EXPRESSION_COMPLETION_VARIANT_RANK: u8 = 1;
const PDA_BUMP_CONSTRAINT_KEY: &str = "bump";
const PDA_BUMP_TYPE: &str = "u8";

pub(super) fn value_items_for_slot(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
    slot: ConstraintValueSlot,
    value_prefix: &str,
) -> Vec<CompletionItem> {
    match slot.value_kind {
        ConstraintValueKind::ProgramReference => {
            program_reference_items(accounts, current_field, slot.key)
        }
        ConstraintValueKind::SignerReference => {
            signer_value_items(accounts, current_field, slot.key)
        }
        ConstraintValueKind::AccountReference if slot.key == "has_one" => {
            has_one_items(document, workspace_index, accounts, current_field)
        }
        ConstraintValueKind::AccountReference => {
            account_name_items(accounts, current_field, slot.key)
        }
        ConstraintValueKind::InstructionArgument if slot.key == PDA_BUMP_CONSTRAINT_KEY => {
            bump_value_items(document, workspace_index, accounts, value_prefix)
        }
        ConstraintValueKind::InstructionArgument => instruction_argument_items(
            document,
            workspace_index,
            accounts,
            PDA_BUMP_TYPE,
            "Instruction argument",
        ),
        ConstraintValueKind::Boolean => boolean_items(),
        ConstraintValueKind::Keyword => keyword_items(slot.key),
        ConstraintValueKind::Space => {
            if associated_values::associated_value_prefix(value_prefix).is_some() {
                associated_values::associated_value_items(document, workspace_index, value_prefix)
            } else {
                space_items(accounts, current_field)
            }
        }
        ConstraintValueKind::Seeds => seed_items(document, accounts, current_field),
        ConstraintValueKind::AnyExpression => {
            if associated_values::associated_value_prefix(value_prefix).is_some() {
                associated_values::associated_value_items(document, workspace_index, value_prefix)
            } else {
                members::expression_member_items(document, workspace_index, accounts, value_prefix)
            }
        }
        ConstraintValueKind::None => Vec::new(),
    }
}

pub(super) fn filter_prefix_for_slot(slot: ConstraintValueSlot, value_prefix: &str) -> &str {
    if slot.value_kind == ConstraintValueKind::AnyExpression || slot.key == PDA_BUMP_CONSTRAINT_KEY
    {
        if let Some(prefix) = associated_values::filter_prefix(value_prefix) {
            return prefix;
        }
        members::member_access_prefix(value_prefix)
            .map(|access_prefix| access_prefix.member_prefix)
            .unwrap_or(value_prefix)
    } else if slot.value_kind == ConstraintValueKind::Space {
        associated_values::filter_prefix(value_prefix).unwrap_or(value_prefix)
    } else {
        value_prefix
    }
}

fn bump_value_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    value_prefix: &str,
) -> Vec<CompletionItem> {
    if members::member_access_prefix(value_prefix).is_some() {
        return members::expression_member_items_with_type(
            document,
            workspace_index,
            accounts,
            value_prefix,
            Some(PDA_BUMP_TYPE),
        );
    }

    instruction_argument_items(
        document,
        workspace_index,
        accounts,
        PDA_BUMP_TYPE,
        "PDA bump instruction argument",
    )
}

fn signer_account_items(accounts: &SymbolRange, key: &str) -> Vec<CompletionItem> {
    let mut items = accounts
        .fields
        .iter()
        .filter(|field| is_signer_account(field))
        .map(|field| account_item(accounts, field, "Signer account", key))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn signer_key_items(accounts: &SymbolRange, key: &str) -> Vec<CompletionItem> {
    let mut items = accounts
        .fields
        .iter()
        .filter(|field| is_signer_account(field))
        .map(|field| account_expression_item(accounts, field, "Signer public key", ".key()", key))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn signer_reference_items(accounts: &SymbolRange, key: &str) -> Vec<CompletionItem> {
    let mut items = signer_account_items(accounts, key)
        .into_iter()
        .chain(signer_key_items(accounts, key))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn signer_value_items(
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
    key: &str,
) -> Vec<CompletionItem> {
    if key.ends_with("payer") {
        return signer_account_items(accounts, key);
    }
    if key == "token::authority" {
        let mut items = signer_account_items(accounts, key);
        if let Some(field) = current_field.filter(|field| is_token_account(accounts, field)) {
            items.push(account_item(
                accounts,
                field,
                "PDA token account authority",
                key,
            ));
        }
        items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
        return items;
    }
    if key.starts_with("mint::") || key.starts_with("extensions::") {
        let mut items = signer_reference_items(accounts, key);
        if let Some(field) = current_field.filter(|field| is_mint_account(accounts, field)) {
            items.push(account_expression_item(
                accounts,
                field,
                "PDA authority public key",
                ".key()",
                key,
            ));
        }
        items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
        return items;
    }
    signer_account_items(accounts, key)
}

fn program_reference_items(
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
    key: &str,
) -> Vec<CompletionItem> {
    let mut items = accounts
        .fields
        .iter()
        .filter(|field| is_program_reference_candidate(accounts, field, current_field, key))
        .flat_map(|field| {
            [
                account_item(accounts, field, "Program account", key),
                account_expression_item(accounts, field, "Program id", ".key()", key),
            ]
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn boolean_items() -> Vec<CompletionItem> {
    ["true", "false"]
        .into_iter()
        .map(|label| CompletionItem {
            label: label.to_string(),
            kind: Some(CompletionItemKind::VALUE),
            detail: Some("Boolean constraint value".to_string()),
            sort_text: Some(format!("000_anchor_value_{label}")),
            ..CompletionItem::default()
        })
        .collect()
}

fn keyword_items(key: &str) -> Vec<CompletionItem> {
    match key {
        "rent_exempt" => ["skip", "enforce"]
            .into_iter()
            .map(|label| CompletionItem {
                label: label.to_string(),
                kind: Some(CompletionItemKind::KEYWORD),
                detail: Some("Anchor rent exemption mode".to_string()),
                sort_text: Some(format!("000_anchor_keyword_{label}")),
                ..CompletionItem::default()
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn instruction_argument_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    preferred_type: &'static str,
    detail: &'static str,
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
        .filter(|argument| argument.type_name.as_deref() == Some(preferred_type))
        .map(|argument| CompletionItem {
            label: argument.name.clone(),
            kind: Some(CompletionItemKind::VARIABLE),
            detail: Some(detail.to_string()),
            preselect: Some(true),
            sort_text: Some(format!(
                "000_anchor_value_{:04}_{:04}_{}",
                argument.range.start.line, argument.range.start.character, argument.name
            )),
            ..CompletionItem::default()
        })
        .collect::<Vec<_>>();

    let workspace_argument_names = workspace_index
        .map(|index| index.instruction_argument_names_for_context(&accounts.name))
        .unwrap_or_default();
    items.extend(
        workspace_argument_names
            .into_iter()
            .filter_map(|argument_name| {
                let argument_type = workspace_index.and_then(|index| {
                    index.instruction_argument_type_for_context(&accounts.name, &argument_name)
                })?;
                (argument_type == preferred_type).then(|| CompletionItem {
                    label: argument_name,
                    kind: Some(CompletionItemKind::VARIABLE),
                    detail: Some(detail.to_string()),
                    preselect: Some(true),
                    sort_text: Some("010_anchor_workspace_instruction_argument".to_string()),
                    ..CompletionItem::default()
                })
            }),
    );

    items.sort_by(|left, right| left.label.cmp(&right.label));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

fn account_name_items(
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
    key: &str,
) -> Vec<CompletionItem> {
    let base_fields = accounts
        .fields
        .iter()
        .filter(|field| {
            current_field
                .map(|current| current.name != field.name)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    let candidates = account_reference_candidates(accounts, &base_fields, current_field, key);
    let fields = if candidates.is_empty() {
        base_fields
    } else {
        candidates
    };

    let mut items = fields
        .into_iter()
        .map(|field| account_item(accounts, field, "Account", key))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn has_one_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    current_field: Option<&SymbolRange>,
) -> Vec<CompletionItem> {
    let Some(current_field) = current_field else {
        return account_name_items(accounts, None, "has_one");
    };
    let Some(account_data_type) = current_field.generic_type_names.last() else {
        return account_name_items(accounts, Some(current_field), "has_one");
    };
    let account_data_fields =
        account_data_pubkey_field_names(document, workspace_index, account_data_type);
    if account_data_fields.is_empty() {
        return account_name_items(accounts, Some(current_field), "has_one");
    }

    let mut items = accounts
        .fields
        .iter()
        .filter(|field| field.name != current_field.name)
        .filter(|field| account_data_fields.contains(field.name.as_str()))
        .map(|field| {
            account_item(
                accounts,
                field,
                "Account referenced by account data field",
                "has_one",
            )
        })
        .collect::<Vec<_>>();

    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items
}

fn account_data_pubkey_field_names(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    account_data_type: &str,
) -> HashSet<String> {
    if let Some(account_data) = document
        .symbols()
        .account_data_structs
        .get(account_data_type)
    {
        return account_data
            .fields
            .iter()
            .filter(|field| is_pubkey_type(field.type_name.as_deref()))
            .map(|field| field.name.clone())
            .collect();
    }

    workspace_index
        .map(|index| {
            index
                .account_context_fields(account_data_type)
                .into_iter()
                .filter(|field| is_pubkey_type(field.type_display.as_deref()))
                .map(|field| field.name)
                .collect()
        })
        .unwrap_or_default()
}

fn space_items(accounts: &SymbolRange, current_field: Option<&SymbolRange>) -> Vec<CompletionItem> {
    let Some(field) = current_field else {
        return Vec::new();
    };
    if is_spl_account_space_managed_by_anchor(accounts, field) {
        return Vec::new();
    }
    let Some(account_type) = field.generic_type_names.last() else {
        return Vec::new();
    };

    vec![CompletionItem {
        label: format!("8 + {account_type}::INIT_SPACE"),
        kind: Some(CompletionItemKind::VALUE),
        detail: Some(format!(
            "Anchor discriminator plus `{account_type}` init space"
        )),
        sort_text: Some("000_anchor_value_space".to_string()),
        preselect: Some(true),
        ..CompletionItem::default()
    }]
}

fn seed_items(
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
            let (suffix, detail) =
                seed_expression_for_argument_type(argument.type_name.as_deref())?;
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

fn seed_expression_for_argument_type(
    type_name: Option<&str>,
) -> Option<(&'static str, &'static str)> {
    match type_name? {
        "Pubkey" => Some((".as_ref()", "PDA seed bytes")),
        "String" => Some((".as_bytes()", "UTF-8 PDA seed bytes")),
        "u8" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "u16" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "u32" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "u64" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "u128" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "i8" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "i16" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "i32" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "i64" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        "i128" => Some((".to_le_bytes().as_ref()", "little-endian PDA seed bytes")),
        _ => None,
    }
}

fn is_account_seed_candidate(field: &SymbolRange) -> bool {
    !field.is_optional
}

fn is_spl_account_space_managed_by_anchor(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    account_semantics::field_has_declared_or_expected_account_inner_type(accounts, field, "Mint")
        || account_semantics::field_has_declared_or_expected_account_inner_type(
            accounts,
            field,
            "TokenAccount",
        )
}

fn is_signer_account(field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some("Signer")
        || field
            .account_constraints
            .iter()
            .any(|constraint| constraint_has_key(&constraint.text, "signer"))
}

fn is_program_account(field: &SymbolRange) -> bool {
    matches!(
        field.type_name.as_deref(),
        Some("Program") | Some("Interface")
    )
}

fn is_program_reference_candidate(
    accounts: &SymbolRange,
    field: &SymbolRange,
    current_field: Option<&SymbolRange>,
    key: &str,
) -> bool {
    if !is_program_account(field) {
        return false;
    }

    if key.ends_with("token_program") {
        if current_field.is_some_and(|field| is_token_interface_account(accounts, field)) {
            return field.type_name.as_deref() == Some("Interface")
                && has_generic_type(field, "TokenInterface");
        }
        return is_token_program_account(field);
    }

    true
}

fn account_reference_candidates<'a>(
    accounts: &'a SymbolRange,
    fields: &[&'a SymbolRange],
    current_field: Option<&'a SymbolRange>,
    key: &str,
) -> Vec<&'a SymbolRange> {
    match key {
        "token::mint" | "associated_token::mint" => fields
            .iter()
            .copied()
            .filter(|field| is_mint_account(accounts, field))
            .collect(),
        "token::authority" => {
            let mut candidates = fields
                .iter()
                .copied()
                .filter(|field| is_authority_candidate(field))
                .collect::<Vec<_>>();
            if let Some(field) = current_field.filter(|field| is_token_account(accounts, field)) {
                candidates.push(field);
            }
            candidates
        }
        "associated_token::authority" => fields
            .iter()
            .copied()
            .filter(|field| is_authority_candidate(field))
            .collect(),
        _ => Vec::new(),
    }
}

fn is_mint_account(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    account_semantics::resolve_field_account_type(accounts, field) == ResolvedAccountType::Mint
}

fn is_token_account(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    account_semantics::resolve_field_account_type(accounts, field)
        == ResolvedAccountType::TokenAccount
}

fn is_token_interface_account(accounts: &SymbolRange, field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some("InterfaceAccount")
        && matches!(
            account_semantics::resolve_field_account_type(accounts, field),
            ResolvedAccountType::Mint | ResolvedAccountType::TokenAccount
        )
}

fn is_authority_candidate(field: &SymbolRange) -> bool {
    is_signer_account(field)
}

fn is_token_program_account(field: &SymbolRange) -> bool {
    has_generic_type(field, "Token")
        || has_generic_type(field, "Token2022")
        || has_generic_type(field, "TokenInterface")
}

fn account_item(
    accounts: &SymbolRange,
    field: &SymbolRange,
    detail: &'static str,
    key: &str,
) -> CompletionItem {
    let type_matched = role_score(accounts, field, key) == 0;
    CompletionItem {
        label: field.name.clone(),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some(detail.to_string()),
        sort_text: Some(sort_text(
            accounts,
            field,
            key,
            DIRECT_ACCOUNT_COMPLETION_VARIANT_RANK,
        )),
        preselect: Some(type_matched),
        data: Some(serde_json::json!({
            "anchorCompletion": "constraint-value",
            "constraintKey": key,
            "typeMatched": type_matched,
        })),
        ..CompletionItem::default()
    }
}

fn account_expression_item(
    accounts: &SymbolRange,
    field: &SymbolRange,
    detail: &'static str,
    suffix: &'static str,
    key: &str,
) -> CompletionItem {
    let type_matched = role_score(accounts, field, key) == 0;
    CompletionItem {
        label: format!("{}{}", field.name, suffix),
        kind: Some(CompletionItemKind::VARIABLE),
        detail: Some(detail.to_string()),
        sort_text: Some(sort_text(
            accounts,
            field,
            key,
            ACCOUNT_EXPRESSION_COMPLETION_VARIANT_RANK,
        )),
        preselect: Some(type_matched),
        data: Some(serde_json::json!({
            "anchorCompletion": "constraint-value",
            "constraintKey": key,
            "typeMatched": type_matched,
        })),
        ..CompletionItem::default()
    }
}

fn sort_text(accounts: &SymbolRange, field: &SymbolRange, key: &str, variant_rank: u8) -> String {
    format!(
        "{:03}_{:03}_{:04}_{:04}_anchor_value_{}",
        role_score(accounts, field, key),
        variant_rank,
        field.selection_range.start.line,
        field.selection_range.start.character,
        field.name
    )
}

fn role_score(accounts: &SymbolRange, field: &SymbolRange, key: &str) -> u8 {
    match key {
        "token::mint" | "associated_token::mint" => {
            if is_mint_account(accounts, field) {
                0
            } else {
                8
            }
        }
        "token::authority"
        | "associated_token::authority"
        | "mint::authority"
        | "mint::freeze_authority"
        | "extensions::group_pointer::authority"
        | "extensions::group_member_pointer::authority"
        | "extensions::metadata_pointer::authority"
        | "extensions::close_authority::authority"
        | "extensions::transfer_hook::authority"
        | "extensions::pausable::authority"
        | "extensions::permanent_delegate::delegate" => authority_score(field),
        "payer" | "realloc::payer" => payer_score(field),
        key if key.ends_with("token_program") => {
            if is_token_program_account(field) {
                0
            } else if is_program_account(field) {
                1
            } else {
                8
            }
        }
        "seeds::program" | "extensions::transfer_hook::program_id" => {
            if is_program_account(field) {
                0
            } else {
                8
            }
        }
        _ => {
            if field.name == key
                || key
                    .rsplit("::")
                    .next()
                    .is_some_and(|tail| field.name == tail || field.name.ends_with(tail))
            {
                0
            } else {
                5
            }
        }
    }
}

fn authority_score(field: &SymbolRange) -> u8 {
    if field.type_name.as_deref() == Some("Signer") {
        0
    } else if is_signer_account(field) {
        1
    } else {
        8
    }
}

fn payer_score(field: &SymbolRange) -> u8 {
    if field.type_name.as_deref() == Some("Signer") {
        0
    } else if is_signer_account(field) {
        1
    } else {
        8
    }
}

fn constraint_has_key(text: &str, key: &str) -> bool {
    let mut search_end = text.len();
    while let Some(idx) = text[..search_end].rfind(key) {
        if constraint_key_boundary(text, idx, key.len()) {
            return true;
        }
        search_end = idx;
    }
    false
}

fn constraint_key_boundary(text: &str, idx: usize, key_len: usize) -> bool {
    let previous = text[..idx].chars().next_back();
    let next = text[idx + key_len..].chars().next();
    previous
        .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
        .unwrap_or(true)
        && next
            .map(|ch| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':'))
            .unwrap_or(true)
}

fn is_pubkey_type(type_name: Option<&str>) -> bool {
    type_name
        .and_then(last_type_segment)
        .is_some_and(|segment| segment == "Pubkey" || segment == "pubkey")
}

fn last_type_segment(type_name: &str) -> Option<&str> {
    let without_generics = type_name.split('<').next().unwrap_or(type_name);
    without_generics
        .split("::")
        .last()
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
}

fn has_generic_type(field: &SymbolRange, generic: &str) -> bool {
    field.generic_type_names.iter().any(|name| name == generic)
}
