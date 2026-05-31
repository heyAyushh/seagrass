use {
    crate::{
        account_members::{ResolvedAccountMember, ResolvedAccountMembers},
        document::{ParsedDocument, PdaBump, SymbolRange},
        workspace::{WorkspaceAccountField, WorkspaceIndex},
    },
    tower_lsp::lsp_types::CompletionItemKind,
};

pub(crate) const CONTEXT_ACCOUNTS_MEMBER: &str = "accounts";
pub(crate) const CONTEXT_BUMPS_MEMBER: &str = "bumps";
const CONTEXT_PROGRAM_ID_MEMBER: &str = "program_id";
const CONTEXT_REMAINING_ACCOUNTS_MEMBER: &str = "remaining_accounts";
const GENERATED_BUMPS_SUFFIX: &str = "Bumps";
const CONTEXT_TYPE_NAME: &str = "Context";
const BUMP_TYPE: &str = "u8";
const OPTIONAL_BUMP_TYPE: &str = "Option<u8>";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MissingContextMember {
    pub(crate) member_index: usize,
    pub(crate) receiver_path: String,
    pub(crate) member: String,
    pub(crate) owner_type: String,
    pub(crate) candidates: Vec<String>,
}

pub(crate) fn resolved_context_chain_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    context_type: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let Some((head, tail)) = member_chain.split_first() else {
        return Some(context_root_members(context_type));
    };

    if head != CONTEXT_BUMPS_MEMBER {
        return None;
    }

    match tail {
        [] => generated_bump_members(document, workspace_index, context_type),
        [bump_field]
            if generated_bump_members(document, workspace_index, context_type)?
                .members
                .iter()
                .any(|member| member.name == *bump_field) =>
        {
            None
        }
        _ => None,
    }
}

pub(crate) fn resolved_generated_bumps_chain_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    bumps_type: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let context_type = context_type_from_generated_bumps(bumps_type)?;
    match member_chain {
        [] => generated_bump_members(document, workspace_index, context_type),
        [bump_field]
            if generated_bump_members(document, workspace_index, context_type)?
                .members
                .iter()
                .any(|member| member.name == *bump_field) =>
        {
            None
        }
        _ => None,
    }
}

pub(crate) fn missing_context_member_in_chain(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    context_type: &str,
    receiver: &str,
    member_chain: &[String],
) -> Option<MissingContextMember> {
    let (head, tail) = member_chain.split_first()?;
    let root_members = context_root_members(context_type);

    if !root_members
        .members
        .iter()
        .any(|member| member.name == *head)
    {
        return Some(MissingContextMember {
            member_index: 0,
            receiver_path: receiver.to_string(),
            member: head.clone(),
            owner_type: root_members.owner_type.clone(),
            candidates: member_names(&root_members),
        });
    }

    if head == CONTEXT_ACCOUNTS_MEMBER {
        return None;
    }

    if head != CONTEXT_BUMPS_MEMBER {
        return primitive_tail_missing(receiver, head, tail);
    }

    let bump_members = generated_bump_members(document, workspace_index, context_type)?;
    let Some((bump_field, bump_tail)) = tail.split_first() else {
        return None;
    };
    let Some(resolved_bump) = bump_members
        .members
        .iter()
        .find(|member| member.name == *bump_field)
    else {
        return Some(MissingContextMember {
            member_index: 1,
            receiver_path: format!("{receiver}.{head}"),
            member: bump_field.clone(),
            owner_type: bump_members.owner_type.clone(),
            candidates: member_names(&bump_members),
        });
    };

    bump_tail.first().map(|member| MissingContextMember {
        member_index: 2,
        receiver_path: format!("{receiver}.{head}.{bump_field}"),
        member: member.clone(),
        owner_type: resolved_bump
            .type_name
            .clone()
            .unwrap_or_else(|| BUMP_TYPE.to_string()),
        candidates: Vec::new(),
    })
}

pub(crate) fn missing_generated_bumps_member_in_chain(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    bumps_type: &str,
    receiver: &str,
    member_chain: &[String],
) -> Option<MissingContextMember> {
    let context_type = context_type_from_generated_bumps(bumps_type)?;
    let bump_members = generated_bump_members(document, workspace_index, context_type)?;
    let (head, tail) = member_chain.split_first()?;
    let Some(resolved_bump) = bump_members
        .members
        .iter()
        .find(|member| member.name == *head)
    else {
        return Some(MissingContextMember {
            member_index: 0,
            receiver_path: receiver.to_string(),
            member: head.clone(),
            owner_type: bump_members.owner_type.clone(),
            candidates: member_names(&bump_members),
        });
    };

    tail.first().map(|member| MissingContextMember {
        member_index: 1,
        receiver_path: format!("{receiver}.{head}"),
        member: member.clone(),
        owner_type: resolved_bump
            .type_name
            .clone()
            .unwrap_or_else(|| BUMP_TYPE.to_string()),
        candidates: Vec::new(),
    })
}

fn primitive_tail_missing(
    receiver: &str,
    head: &str,
    tail: &[String],
) -> Option<MissingContextMember> {
    tail.first().map(|member| MissingContextMember {
        member_index: 1,
        receiver_path: format!("{receiver}.{head}"),
        member: member.clone(),
        owner_type: context_root_member_type(head).unwrap_or(head).to_string(),
        candidates: Vec::new(),
    })
}

fn context_root_members(context_type: &str) -> ResolvedAccountMembers {
    ResolvedAccountMembers {
        owner_type: format!("{CONTEXT_TYPE_NAME}<{context_type}>"),
        members: [
            context_root_member(
                CONTEXT_ACCOUNTS_MEMBER,
                "Anchor accounts context",
                Some(context_type.to_string()),
            ),
            context_root_member(
                CONTEXT_BUMPS_MEMBER,
                "Anchor generated PDA bump values",
                Some(generated_bumps_type(context_type)),
            ),
            context_root_member(
                CONTEXT_PROGRAM_ID_MEMBER,
                "currently executing program id",
                Some("Pubkey".to_string()),
            ),
            context_root_member(
                CONTEXT_REMAINING_ACCOUNTS_MEMBER,
                "remaining account infos",
                None,
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn context_root_member(
    name: &str,
    detail: &str,
    type_name: Option<String>,
) -> ResolvedAccountMember {
    ResolvedAccountMember {
        name: name.to_string(),
        detail: detail.to_string(),
        completion_kind: CompletionItemKind::FIELD,
        type_name,
    }
}

fn context_root_member_type(member: &str) -> Option<&'static str> {
    match member {
        CONTEXT_PROGRAM_ID_MEMBER => Some("Pubkey"),
        _ => None,
    }
}

fn generated_bump_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    context_type: &str,
) -> Option<ResolvedAccountMembers> {
    if let Some(accounts) = document.symbols().accounts_structs.get(context_type) {
        return Some(ResolvedAccountMembers {
            owner_type: generated_bumps_type(context_type),
            members: accounts
                .fields
                .iter()
                .filter_map(document_bump_member)
                .collect(),
        });
    }

    workspace_index
        .and_then(|index| index.accounts_struct(context_type))
        .map(|accounts| ResolvedAccountMembers {
            owner_type: generated_bumps_type(context_type),
            members: accounts
                .fields
                .iter()
                .filter_map(workspace_bump_member)
                .collect(),
        })
}

fn document_bump_member(field: &SymbolRange) -> Option<ResolvedAccountMember> {
    has_generated_bump_field(field.pda_constraint.as_ref()?).then(|| {
        bump_member(
            &field.name,
            if field.is_optional {
                OPTIONAL_BUMP_TYPE
            } else {
                BUMP_TYPE
            },
        )
    })
}

fn workspace_bump_member(field: &WorkspaceAccountField) -> Option<ResolvedAccountMember> {
    field
        .account_constraints
        .iter()
        .find_map(|constraint| constraint.pda.as_ref())
        .filter(|pda| has_generated_bump_field(pda))
        .map(|_| {
            bump_member(
                &field.name,
                if field.is_optional {
                    OPTIONAL_BUMP_TYPE
                } else {
                    BUMP_TYPE
                },
            )
        })
}

fn bump_member(name: &str, type_name: &str) -> ResolvedAccountMember {
    ResolvedAccountMember {
        name: name.to_string(),
        detail: format!("Anchor generated PDA bump: `{type_name}`"),
        completion_kind: CompletionItemKind::FIELD,
        type_name: Some(type_name.to_string()),
    }
}

fn has_generated_bump_field(pda: &crate::document::PdaConstraint) -> bool {
    pda.is_init || matches!(pda.bump, PdaBump::Canonical)
}

pub(crate) fn generated_bumps_type(context_type: &str) -> String {
    format!("{context_type}{GENERATED_BUMPS_SUFFIX}")
}

fn context_type_from_generated_bumps(bumps_type: &str) -> Option<&str> {
    bumps_type.strip_suffix(GENERATED_BUMPS_SUFFIX)
}

fn member_names(members: &ResolvedAccountMembers) -> Vec<String> {
    members
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect()
}
