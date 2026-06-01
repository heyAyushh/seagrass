use crate::{
    account_members::{self, MissingAccountMember, ResolvedAccountMembers},
    document::ParsedDocument,
    evidence::AccountSetEvidence,
    workspace::WorkspaceIndex,
};

pub(super) fn missing_member_in_chain(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    receiver: &str,
    member_chain: &[String],
) -> Option<MissingAccountMember> {
    let receiver_type = account_members::instruction_argument_type_name(
        document,
        workspace_index,
        accounts.accounts,
        receiver,
    )?;
    account_members::missing_struct_member_in_chain(
        document,
        workspace_index,
        &receiver_type,
        receiver,
        member_chain,
    )
}

pub(super) fn resolved_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    accounts: &AccountSetEvidence<'_>,
    receiver: &str,
    member_chain: &[String],
) -> Option<ResolvedAccountMembers> {
    let receiver_type = account_members::instruction_argument_type_name(
        document,
        workspace_index,
        accounts.accounts,
        receiver,
    )?;
    account_members::resolved_struct_chain_completion_members(
        document,
        workspace_index,
        &receiver_type,
        member_chain,
    )
}
