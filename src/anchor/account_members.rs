use {
    crate::{
        account_semantics,
        document::{
            summarize_account_field_type, AssociatedValueKind, ParsedDocument, SymbolRange,
        },
        workspace::{WorkspaceAccountField, WorkspaceAccountsStruct, WorkspaceIndex},
    },
    tower_lsp::lsp_types::{CompletionItemKind, Range, SymbolKind},
};

pub(crate) const ACCOUNT_LOADER_TYPE: &str = "AccountLoader";
pub(crate) const ACCOUNT_LOADER_LOADED_METHODS: &[&str] = &["load", "load_mut", "load_init"];
pub(crate) const ACCOUNT_LOADER_LOADED_METHOD_COMPLETIONS: &[&str] =
    &["load()?", "load_mut()?", "load_init()?"];
pub(crate) const ACCOUNT_LOADER_LOADED_METHOD_SUFFIXES: &[&str] =
    &[".load()?", ".load_mut()?", ".load_init()?"];

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
        members = resolved_struct_members(document, workspace_index, next_type)?;
    }
    Some(members)
}

pub(crate) fn resolved_struct_chain_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let mut members = resolved_struct_members(document, workspace_index, receiver_type)?;
    for member_name in member_chain {
        members = resolved_struct_member_members(
            document,
            workspace_index,
            &members.owner_type,
            member_name,
        )?;
    }
    Some(members)
}

pub(crate) fn resolved_struct_chain_completion_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let mut members =
        resolved_struct_chain_members(document, workspace_index, receiver_type, member_chain)?;
    extend_with_inherent_methods(document, workspace_index, &mut members);
    Some(members)
}

pub(crate) fn resolved_struct_member_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
    member_name: &str,
) -> Option<String> {
    account_context_field_type_name(document, workspace_index, owner_type, member_name).or_else(
        || {
            resolved_struct_members(document, workspace_index, owner_type)?
                .member(member_name)?
                .type_name
                .clone()
        },
    )
}

pub(crate) fn resolved_struct_member_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
    member_name: &str,
) -> Option<ResolvedAccountMembers> {
    account_context_field_members(document, workspace_index, owner_type, member_name).or_else(
        || {
            let next_type = resolved_struct_members(document, workspace_index, owner_type)?
                .member(member_name)?
                .type_name
                .as_ref()?
                .clone();
            resolved_struct_members(document, workspace_index, &next_type)
        },
    )
}

pub(crate) fn resolved_struct_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
) -> Option<ResolvedAccountMembers> {
    if receiver_type == ACCOUNT_LOADER_TYPE || account_loader_inner_type(receiver_type).is_some() {
        return Some(account_loader_members(receiver_type.to_string()));
    }
    struct_members(document, workspace_index, receiver_type)
}

fn account_context_field_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_type: &str,
    field_name: &str,
) -> Option<String> {
    document
        .symbols()
        .accounts_structs
        .get(accounts_type)
        .and_then(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == field_name)
                .and_then(|field| account_context_field_local_type_name(accounts, field))
        })
        .or_else(|| {
            workspace_index
                .and_then(|index| index.accounts_struct(accounts_type))
                .and_then(|accounts| {
                    accounts
                        .fields
                        .iter()
                        .find(|field| field.name == field_name)
                        .and_then(|field| {
                            let accounts_symbol = workspace_accounts_symbol(accounts);
                            let field_symbol = workspace_account_field_symbol(field);
                            account_context_field_local_type_name(&accounts_symbol, &field_symbol)
                        })
                })
        })
}

fn account_context_field_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts_type: &str,
    field_name: &str,
) -> Option<ResolvedAccountMembers> {
    document
        .symbols()
        .accounts_structs
        .get(accounts_type)
        .and_then(|accounts| {
            let field = accounts
                .fields
                .iter()
                .find(|field| field.name == field_name)?;
            resolved_field_members(
                document,
                workspace_index,
                accounts,
                field,
                AccountMemberAccess::Direct,
            )
        })
        .or_else(|| {
            workspace_index
                .and_then(|index| index.accounts_struct(accounts_type))
                .and_then(|accounts| {
                    let field = accounts
                        .fields
                        .iter()
                        .find(|field| field.name == field_name)?;
                    resolved_field_members(
                        document,
                        workspace_index,
                        &workspace_accounts_symbol(accounts),
                        &workspace_account_field_symbol(field),
                        AccountMemberAccess::Direct,
                    )
                })
        })
}

fn workspace_accounts_symbol(accounts: &WorkspaceAccountsStruct) -> SymbolRange {
    SymbolRange {
        name: accounts.name.clone(),
        range: Range::default(),
        selection_range: Range::default(),
        fields: accounts
            .fields
            .iter()
            .map(workspace_account_field_symbol)
            .collect(),
        type_name: None,
        type_range: None,
        generic_type_names: Vec::new(),
        generic_type_ranges: Vec::new(),
        is_optional: false,
        account_constraints: Vec::new(),
        pda_constraint: None,
        instruction_arguments: accounts.instruction_arguments.clone(),
        derive_accounts_range: None,
    }
}

fn workspace_account_field_symbol(field: &WorkspaceAccountField) -> SymbolRange {
    SymbolRange {
        name: field.name.clone(),
        range: Range::default(),
        selection_range: Range::default(),
        fields: Vec::new(),
        type_name: field.type_name.clone(),
        type_range: None,
        generic_type_names: field.generic_type_names.clone(),
        generic_type_ranges: Vec::new(),
        is_optional: field.is_optional,
        account_constraints: field.account_constraints.clone(),
        pda_constraint: field
            .account_constraints
            .iter()
            .find_map(|constraint| constraint.pda.clone()),
        instruction_arguments: Vec::new(),
        derive_accounts_range: None,
    }
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

pub(crate) fn missing_struct_member_in_chain(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    receiver: &str,
    member_chain: &[String],
) -> Option<MissingAccountMember> {
    let mut receiver_segments = vec![receiver.to_string()];
    let mut members = resolved_struct_members(document, workspace_index, receiver_type)?;

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
        members = resolved_struct_members(document, workspace_index, next_type)?;
    }

    None
}

pub(crate) fn instruction_argument_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    receiver: &str,
) -> Option<String> {
    accounts
        .instruction_arguments
        .iter()
        .find(|argument| instruction_argument_names_match(&argument.name, receiver))
        .and_then(|argument| argument.type_name.clone())
        .or_else(|| {
            document
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
                .find(|argument| instruction_argument_names_match(&argument.name, receiver))
                .and_then(|argument| argument.type_name.clone())
        })
        .or_else(|| {
            workspace_index.and_then(|index| {
                index.instruction_argument_type_for_context(&accounts.name, receiver)
            })
        })
}

pub(crate) fn resolved_field_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    field: &SymbolRange,
    access: AccountMemberAccess,
) -> Option<ResolvedAccountMembers> {
    if access == AccountMemberAccess::Direct && is_account_loader(field) {
        return Some(account_loader_members(account_loader_field_type_name(
            field,
        )));
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

fn account_loader_members(owner_type: String) -> ResolvedAccountMembers {
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
        (Some(ACCOUNT_LOADER_TYPE), AccountMemberAccess::Loaded)
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
    field.type_name.as_deref() == Some(ACCOUNT_LOADER_TYPE)
}

pub(crate) fn is_account_loader_loaded_method(method: &str) -> bool {
    ACCOUNT_LOADER_LOADED_METHODS.contains(&method)
}

pub(crate) fn account_loader_type_name(inner_type: &str) -> String {
    format!("{ACCOUNT_LOADER_TYPE}<{inner_type}>")
}

pub(crate) fn account_loader_type_name_from_parts(
    type_name: Option<&str>,
    generic_type_names: &[String],
) -> Option<String> {
    (type_name == Some(ACCOUNT_LOADER_TYPE)).then(|| {
        generic_type_names.last().map_or_else(
            || ACCOUNT_LOADER_TYPE.to_string(),
            |inner| account_loader_type_name(inner),
        )
    })
}

pub(crate) fn account_loader_inner_type(type_name: &str) -> Option<&str> {
    type_name
        .strip_prefix(ACCOUNT_LOADER_TYPE)?
        .strip_prefix('<')?
        .strip_suffix('>')
}

fn account_loader_field_type_name(field: &SymbolRange) -> String {
    account_loader_type_name_from_parts(field.type_name.as_deref(), &field.generic_type_names)
        .unwrap_or_else(|| ACCOUNT_LOADER_TYPE.to_string())
}

fn account_context_field_local_type_name(
    accounts: &SymbolRange,
    field: &SymbolRange,
) -> Option<String> {
    if is_account_loader(field) {
        return Some(account_loader_field_type_name(field));
    }
    account_semantics::declared_or_expected_account_inner_type(accounts, field).map(str::to_string)
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

fn extend_with_inherent_methods(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    members: &mut ResolvedAccountMembers,
) {
    let mut method_names = local_inherent_method_names(document, &members.owner_type);
    method_names.extend(tree_sitter_inherent_method_names(
        document,
        &members.owner_type,
    ));
    method_names.extend(workspace_inherent_method_names(
        workspace_index,
        &members.owner_type,
    ));
    method_names.sort();
    method_names.dedup();

    for method_name in method_names {
        let completion_name = format!("{method_name}()");
        if members
            .members
            .iter()
            .any(|member| member.name == completion_name)
        {
            continue;
        }
        members.members.push(ResolvedAccountMember {
            name: completion_name,
            detail: "Account data method".to_string(),
            completion_kind: CompletionItemKind::METHOD,
            type_name: None,
        });
    }
}

fn local_inherent_method_names(document: &ParsedDocument, owner_type: &str) -> Vec<String> {
    document
        .symbols()
        .associated_value_items
        .get(owner_type)
        .into_iter()
        .flat_map(|items| items.iter())
        .filter(|item| item.kind == AssociatedValueKind::Method)
        .map(|item| item.name.clone())
        .collect()
}

fn tree_sitter_inherent_method_names(document: &ParsedDocument, owner_type: &str) -> Vec<String> {
    document
        .tree_sitter()
        .map(|syntax| syntax.inherent_method_names(document.source(), owner_type))
        .unwrap_or_default()
}

fn workspace_inherent_method_names(
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
) -> Vec<String> {
    workspace_index
        .into_iter()
        .flat_map(|index| index.associated_values_in_container(owner_type))
        .filter(|value| value.kind == SymbolKind::METHOD)
        .map(|value| value.name)
        .collect()
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

fn instruction_argument_names_match(left: &str, right: &str) -> bool {
    left == right || left.trim_start_matches('_') == right.trim_start_matches('_')
}
