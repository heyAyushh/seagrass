use {
    crate::{
        account_members::{self, AccountMemberAccess},
        document::{ParsedDocument, SymbolRange},
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::CompletionItem,
};

pub(super) struct MemberAccessPrefix<'a> {
    pub(super) receiver: &'a str,
    pub(super) member_chain: Vec<String>,
    pub(super) member_prefix: &'a str,
    pub(super) access: AccountMemberAccess,
}

pub(super) fn expression_member_items(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    value_prefix: &str,
) -> Vec<CompletionItem> {
    expression_member_items_with_type(document, workspace_index, accounts, value_prefix, None)
}

pub(super) fn expression_member_items_with_type(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &SymbolRange,
    value_prefix: &str,
    expected_type: Option<&str>,
) -> Vec<CompletionItem> {
    let Some(access_prefix) = member_access_prefix(value_prefix) else {
        return Vec::new();
    };
    let resolved_members = if let Some(field) = accounts
        .fields
        .iter()
        .find(|field| field.name == access_prefix.receiver)
    {
        account_members::resolved_field_chain_members(
            document,
            workspace_index,
            accounts,
            field,
            access_prefix.access,
            &access_prefix.member_chain,
        )
    } else if access_prefix.access == AccountMemberAccess::Direct {
        account_members::instruction_argument_type_name(
            document,
            workspace_index,
            accounts,
            access_prefix.receiver,
        )
        .and_then(|type_name| {
            account_members::resolved_struct_chain_completion_members(
                document,
                workspace_index,
                &type_name,
                &access_prefix.member_chain,
            )
        })
    } else {
        None
    };
    let Some(resolved_members) = resolved_members else {
        return Vec::new();
    };

    let mut items = resolved_members
        .members
        .iter()
        .filter(|member| {
            expected_type.is_none_or(|expected| member.type_name.as_deref() == Some(expected))
        })
        .map(|member| member_item(member, &resolved_members.owner_type))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.sort_text.cmp(&right.sort_text));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

pub(super) fn member_access_prefix(value_prefix: &str) -> Option<MemberAccessPrefix<'_>> {
    let (receiver_expression, member_prefix) = value_prefix.rsplit_once('.')?;
    if !crate::syntax::is_ascii_identifier_prefix(member_prefix) {
        return None;
    }

    if let Some(receiver) = account_loader_receiver(receiver_expression) {
        return Some(MemberAccessPrefix {
            receiver: receiver.name,
            member_chain: receiver.member_chain,
            member_prefix,
            access: AccountMemberAccess::Loaded,
        });
    }

    if !is_identifier_path(receiver_expression) {
        return None;
    }
    let mut segments = receiver_expression.split('.');
    let receiver = segments
        .next()
        .filter(|segment| crate::syntax::is_ascii_identifier_prefix(segment))?;
    Some(MemberAccessPrefix {
        receiver,
        member_chain: segments.map(str::to_string).collect(),
        member_prefix,
        access: AccountMemberAccess::Direct,
    })
}

struct ParsedReceiver<'a> {
    name: &'a str,
    member_chain: Vec<String>,
}

fn account_loader_receiver(receiver_expression: &str) -> Option<ParsedReceiver<'_>> {
    for suffix in account_members::ACCOUNT_LOADER_LOADED_METHOD_SUFFIXES {
        if let Some(receiver) = receiver_expression.strip_suffix(suffix) {
            return parsed_receiver(receiver, "");
        }

        let loaded_prefix = format!("{suffix}.");
        if let Some((receiver, member_chain)) = receiver_expression.split_once(&loaded_prefix) {
            return parsed_receiver(receiver, member_chain);
        }
    }
    None
}

fn parsed_receiver<'a>(receiver: &'a str, member_chain: &str) -> Option<ParsedReceiver<'a>> {
    if !is_identifier_path(receiver)
        || (!member_chain.is_empty() && !is_identifier_path(member_chain))
    {
        return None;
    }
    receiver
        .split('.')
        .next()
        .filter(|name| crate::syntax::is_ascii_identifier_prefix(name))
        .map(|name| ParsedReceiver {
            name,
            member_chain: member_chain
                .split('.')
                .filter(|segment| !segment.is_empty())
                .map(str::to_string)
                .collect(),
        })
}

fn member_item(
    member: &account_members::ResolvedAccountMember,
    owner_type: &str,
) -> CompletionItem {
    CompletionItem {
        label: member.name.clone(),
        kind: Some(member.completion_kind),
        detail: Some(member.detail.clone()),
        sort_text: Some(format!("000_anchor_member_{}", member.name)),
        preselect: Some(true),
        data: Some(serde_json::json!({
            "anchorCompletion": "constraint-member",
            "ownerType": owner_type,
        })),
        ..CompletionItem::default()
    }
}

fn is_identifier_path(value: &str) -> bool {
    !value.is_empty()
        && value
            .split('.')
            .all(crate::syntax::is_ascii_identifier_prefix)
}
