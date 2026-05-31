use {
    crate::{
        account_members::{self, ResolvedAccountMember},
        document::ParsedDocument,
        lsp::local_types,
        workspace::WorkspaceIndex,
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionTextEdit, Position, Range, TextEdit},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandlerMemberAccess {
    receiver: String,
    member_chain: Vec<String>,
    member_prefix: String,
}

pub(super) fn completions(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Vec<CompletionItem>> {
    let access = handler_member_access(document.source(), position)?;
    let receiver_type =
        visible_receiver_type(document, position, workspace_index, &access.receiver)?;
    let members = account_members::resolved_struct_chain_members(
        document,
        workspace_index,
        &receiver_type,
        &access.member_chain,
    )?;
    let replacement_range = prefix_replacement_range(position, &access.member_prefix);
    let mut items = members
        .members
        .iter()
        .filter(|member| matches_prefix(&member.name, &access.member_prefix))
        .map(|member| member_item(member, &members.owner_type, replacement_range))
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    (!items.is_empty()).then_some(items)
}

fn visible_receiver_type(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
    receiver: &str,
) -> Option<String> {
    local_types::visible_typed_values_at_with_workspace(document, position, workspace_index)
        .into_iter()
        .chain(local_types::text_visible_typed_values_at_with_workspace(
            document,
            position,
            workspace_index,
        ))
        .rev()
        .find(|value| value.name == receiver)
        .map(|value| value.type_name)
}

fn handler_member_access(source: &str, position: Position) -> Option<HandlerMemberAccess> {
    let line = crate::range::line_at(source, position.line)?;
    let cursor = usize::try_from(position.character).ok()?.min(line.len());
    let tail = line[..cursor]
        .rsplit(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.'))
        .next()
        .unwrap_or_default();
    let (receiver_expression, member_prefix) = tail.rsplit_once('.')?;
    if !is_identifier_prefix(member_prefix) {
        return None;
    }
    let mut segments = receiver_expression.split('.');
    let receiver = segments.next()?;
    if !is_identifier(receiver) {
        return None;
    }
    let member_chain = segments.map(str::to_string).collect::<Vec<_>>();
    if !member_chain.iter().all(|segment| is_identifier(segment)) {
        return None;
    }
    Some(HandlerMemberAccess {
        receiver: receiver.to_string(),
        member_chain,
        member_prefix: member_prefix.to_string(),
    })
}

fn member_item(
    member: &ResolvedAccountMember,
    owner_type: &str,
    replacement_range: Range,
) -> CompletionItem {
    CompletionItem {
        label: member.name.clone(),
        kind: Some(member.completion_kind),
        detail: Some(format!("{} in `{owner_type}`", member.detail)),
        insert_text: Some(member.name.clone()),
        text_edit: Some(CompletionTextEdit::Edit(TextEdit {
            range: replacement_range,
            new_text: member.name.clone(),
        })),
        sort_text: Some(format!("000_anchor_handler_member_{}", member.name)),
        data: Some(serde_json::json!({
            "anchorCompletion": "handler-member",
            "ownerType": owner_type,
        })),
        ..CompletionItem::default()
    }
}

fn prefix_replacement_range(position: Position, prefix: &str) -> Range {
    Range {
        start: Position {
            line: position.line,
            character: position
                .character
                .saturating_sub(u32::try_from(prefix.chars().count()).unwrap_or_default()),
        },
        end: position,
    }
}

fn matches_prefix(candidate: &str, prefix: &str) -> bool {
    candidate
        .to_ascii_lowercase()
        .starts_with(&prefix.to_ascii_lowercase())
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(super::cursor_context::is_identifier_char)
}

fn is_identifier_prefix(value: &str) -> bool {
    value.chars().all(super::cursor_context::is_identifier_char)
}
