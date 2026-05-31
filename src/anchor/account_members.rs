use {
    crate::{
        account_semantics,
        document::{summarize_account_field_type, ParsedDocument, SymbolRange},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::CompletionItemKind,
};

pub(crate) const ACCOUNT_LOADER_LOADED_METHODS: &[&str] = &["load", "load_mut"];
pub(crate) const ACCOUNT_LOADER_LOADED_METHOD_COMPLETIONS: &[&str] = &["load()?", "load_mut()?"];
pub(crate) const ACCOUNT_LOADER_LOADED_METHOD_SUFFIXES: &[&str] = &[".load()?", ".load_mut()?"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccountMemberAccess {
    Direct,
    Loaded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedAccountMembers {
    pub(crate) owner_type: String,
    pub(crate) members: Vec<ResolvedAccountMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedAccountMember {
    pub(crate) name: String,
    pub(crate) detail: String,
    pub(crate) completion_kind: CompletionItemKind,
    pub(crate) type_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingAccountMember {
    pub(crate) receiver_path: String,
    pub(crate) member: String,
    pub(crate) owner_type: String,
    pub(crate) candidates: Vec<String>,
}

impl ResolvedAccountMembers {
    fn member(&self, member: &str) -> Option<&ResolvedAccountMember> {
        self.members
            .iter()
            .find(|candidate| candidate.name == member)
    }
}

pub(crate) fn resolved_field_chain_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
    access: AccountMemberAccess,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let mut members = resolved_field_members(document, workspace_index, accounts, field, access)?;
    for member_name in member_chain {
        let next_type = members.member(member_name)?.type_name.as_ref()?;
        members = struct_members(document, workspace_index, next_type)?;
    }
    Some(members)
}

pub(crate) fn resolved_struct_chain_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let mut members = struct_members(document, workspace_index, receiver_type)?;
    for member_name in member_chain {
        let next_type = members.member(member_name)?.type_name.as_ref()?;
        members = struct_members(document, workspace_index, next_type)?;
    }
    Some(members)
}

pub(crate) fn resolved_struct_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
) -> Option<ResolvedAccountMembers> {
    struct_members(document, workspace_index, receiver_type)
}

pub(crate) fn missing_member_in_chain(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
    access: AccountMemberAccess,
    receiver: &str,
    member_chain: &[String],
) -> Option<MissingAccountMember> {
    let mut receiver_segments = vec![receiver.to_string()];
    let mut members = resolved_field_members(document, workspace_index, accounts, field, access)?;

    for member_name in member_chain {
        let Some(member) = members.member(member_name) else {
            return Some(MissingAccountMember {
                receiver_path: receiver_segments.join("."),
                member: member_name.clone(),
                owner_type: members.owner_type,
                candidates: members
                    .members
                    .iter()
                    .map(|member| member.name.clone())
                    .collect(),
            });
        };
        receiver_segments.push(member_name.clone());

        let next_type = member.type_name.as_ref()?;
        let next_members = struct_members(document, workspace_index, next_type)?;
        members = next_members;
    }

    None
}

pub(crate) fn resolved_field_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
    access: AccountMemberAccess,
) -> Option<ResolvedAccountMembers> {
    if access == AccountMemberAccess::Direct && is_account_loader(field) {
        return Some(account_loader_members(field));
    }

    let account_type = account_semantics::resolve_field_account_type(accounts, field);
    let builtin_members = account_semantics::resolved_account_member_names(account_type);
    if !builtin_members.is_empty() {
        return Some(ResolvedAccountMembers {
            owner_type: account_type.display_name().to_string(),
            members: builtin_members
                .iter()
                .map(|member| ResolvedAccountMember {
                    name: (*member).to_string(),
                    detail: format!("{} field", account_type.display_name()),
                    completion_kind: CompletionItemKind::FIELD,
                    type_name: None,
                })
                .collect(),
        });
    }

    account_data_members(document, workspace_index, field, access)
        .or_else(|| composite_members(document, workspace_index, field, access))
}

fn account_loader_members(field: &SymbolRange) -> ResolvedAccountMembers {
    let owner_type = field
        .generic_type_names
        .last()
        .map(|inner| format!("AccountLoader<{inner}>"))
        .unwrap_or_else(|| "AccountLoader".to_string());

    ResolvedAccountMembers {
        owner_type,
        members: ACCOUNT_LOADER_LOADED_METHOD_COMPLETIONS
            .iter()
            .copied()
            .map(|name| ResolvedAccountMember {
                name: name.to_string(),
                detail: "AccountLoader method".to_string(),
                completion_kind: CompletionItemKind::METHOD,
                type_name: None,
            })
            .collect(),
    }
}

fn account_data_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &SymbolRange,
    access: AccountMemberAccess,
) -> Option<ResolvedAccountMembers> {
    if !account_data_members_are_visible(field, access) {
        return None;
    }

    let account_data_type = field.generic_type_names.last()?;
    if document
        .symbols()
        .account_data_structs
        .contains_key(account_data_type)
    {
        return struct_members(document, workspace_index, account_data_type);
    }

    workspace_index.and_then(|index| {
        let members = index
            .account_context_fields(account_data_type)
            .into_iter()
            .map(|field| ResolvedAccountMember {
                name: field.name,
                detail: member_detail(field.type_display.as_deref()),
                completion_kind: CompletionItemKind::FIELD,
                type_name: field.type_name,
            })
            .collect::<Vec<_>>();
        (!members.is_empty()).then(|| ResolvedAccountMembers {
            owner_type: account_data_type.clone(),
            members,
        })
    })
}

fn account_data_members_are_visible(field: &SymbolRange, access: AccountMemberAccess) -> bool {
    matches!(
        (field.type_name.as_deref(), access),
        (Some("AccountLoader"), AccountMemberAccess::Loaded)
            | (
                Some("Account" | "InterfaceAccount" | "LazyAccount"),
                AccountMemberAccess::Direct
            )
    )
}

fn composite_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &SymbolRange,
    access: AccountMemberAccess,
) -> Option<ResolvedAccountMembers> {
    if access != AccountMemberAccess::Direct {
        return None;
    }

    let container_type = field.type_name.as_deref()?;
    struct_members(document, workspace_index, container_type)
}

fn is_account_loader(field: &SymbolRange) -> bool {
    field.type_name.as_deref() == Some("AccountLoader")
}

pub(crate) fn is_account_loader_loaded_method(method: &str) -> bool {
    ACCOUNT_LOADER_LOADED_METHODS.contains(&method)
}

fn struct_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    container_type: &str,
) -> Option<ResolvedAccountMembers> {
    if let Some(container) = document.symbols().all_structs.get(container_type) {
        return Some(ResolvedAccountMembers {
            owner_type: container_type.to_string(),
            members: container.fields.iter().map(symbol_member).collect(),
        });
    }
    if let Some(members) = tree_sitter_struct_members(document, container_type) {
        return Some(members);
    }

    workspace_index.and_then(|index| {
        let members = index
            .account_context_fields(container_type)
            .into_iter()
            .map(|field| ResolvedAccountMember {
                name: field.name,
                detail: member_detail(field.type_display.as_deref()),
                completion_kind: CompletionItemKind::FIELD,
                type_name: field.type_name,
            })
            .collect::<Vec<_>>();
        (!members.is_empty()).then(|| ResolvedAccountMembers {
            owner_type: container_type.to_string(),
            members,
        })
    })
}

fn tree_sitter_struct_members(
    document: &ParsedDocument,
    container_type: &str,
) -> Option<ResolvedAccountMembers> {
    let members = document
        .tree_sitter()?
        .struct_fields_named(document.source(), container_type)
        .into_iter()
        .filter_map(|field| {
            let name = field.name?;
            let type_name = field
                .type_text
                .as_deref()
                .and_then(field_type_name_from_text);
            Some(ResolvedAccountMember {
                name,
                detail: member_detail(field.type_text.as_deref()),
                completion_kind: CompletionItemKind::FIELD,
                type_name,
            })
        })
        .collect::<Vec<_>>();
    (!members.is_empty()).then(|| ResolvedAccountMembers {
        owner_type: container_type.to_string(),
        members,
    })
}

fn field_type_name_from_text(type_text: &str) -> Option<String> {
    syn::parse_str::<syn::Type>(type_text)
        .ok()
        .and_then(|ty| summarize_account_field_type(&ty).type_name)
}

fn symbol_member(field: &SymbolRange) -> ResolvedAccountMember {
    ResolvedAccountMember {
        name: field.name.clone(),
        detail: member_detail(symbol_type_display(field).as_deref()),
        completion_kind: CompletionItemKind::FIELD,
        type_name: field.type_name.clone(),
    }
}

fn member_detail(type_display: Option<&str>) -> String {
    type_display
        .map(|display| format!("Account data field: `{display}`"))
        .unwrap_or_else(|| "Account data field".to_string())
}

fn symbol_type_display(field: &SymbolRange) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if field.generic_type_names.is_empty() {
        Some(type_name.clone())
    } else {
        Some(format!(
            "{}<{}>",
            type_name,
            field.generic_type_names.join(", ")
        ))
    }
}
