use {
    crate::{
        account_members::{self, ResolvedAccountMember},
        document::ParsedDocument,
        lsp::local_types::{self, TypedLocalValue},
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
    let receiver_type = visible_receiver_type(document, position, &access.receiver)?;
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
    receiver: &str,
) -> Option<String> {
    local_types::visible_typed_values_at(document, position)
        .into_iter()
        .chain(text_visible_typed_values(document.source(), position))
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

fn text_visible_typed_values(source: &str, position: Position) -> Vec<TypedLocalValue> {
    let Some(offset) = crate::range::byte_offset_at(source, position) else {
        return Vec::new();
    };
    let before_cursor = &source[..offset.min(source.len())];
    let Some(function_start) = super::cursor_context::last_function_keyword_before(before_cursor)
    else {
        return Vec::new();
    };
    let function_prefix = &before_cursor[function_start..];
    let mut values = text_function_input_values(function_prefix);
    if let Some((_, body_prefix)) = function_prefix.split_once('{') {
        let completed_body_lines = body_prefix
            .rsplit_once('\n')
            .map_or("", |(completed_lines, _)| completed_lines);
        values.extend(text_local_typed_values(completed_body_lines));
    }
    values
}

fn text_function_input_values(function_prefix: &str) -> Vec<TypedLocalValue> {
    let signature = function_prefix
        .split_once('{')
        .map_or(function_prefix, |(signature, _)| signature);
    let Some(open) = signature.find('(') else {
        return Vec::new();
    };
    let Some(close) = signature[open..].rfind(')').map(|idx| open + idx) else {
        return Vec::new();
    };
    split_top_level_commas(&signature[open + '('.len_utf8()..close])
        .into_iter()
        .filter_map(text_typed_value_from_binding)
        .collect()
}

fn text_local_typed_values(body_prefix: &str) -> Vec<TypedLocalValue> {
    let mut depth = 0usize;
    let mut values = Vec::new();
    for line in body_prefix.lines() {
        if depth == 0 {
            values.extend(text_local_typed_value(line));
        }
        depth = line.chars().fold(depth, |depth, ch| match ch {
            '{' => depth + 1,
            '}' => depth.saturating_sub(1),
            _ => depth,
        });
    }
    values
}

fn text_local_typed_value(line: &str) -> Option<TypedLocalValue> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix("let ")?;
    let (left, right) = rest.split_once('=')?;
    text_typed_value_from_binding(left)
        .or_else(|| text_constructed_value(left, right.trim().trim_end_matches(';').trim()))
}

fn text_typed_value_from_binding(binding: &str) -> Option<TypedLocalValue> {
    let (name, ty) = binding.trim().split_once(':')?;
    let name = name.trim().strip_prefix("mut ").unwrap_or(name.trim());
    if !is_identifier(name) {
        return None;
    }
    let type_name = type_name_from_text(ty.trim())?;
    Some(TypedLocalValue {
        name: name.to_string(),
        type_name,
    })
}

fn text_constructed_value(left: &str, right: &str) -> Option<TypedLocalValue> {
    let name = left.trim().strip_prefix("mut ").unwrap_or(left.trim());
    if !is_identifier(name) {
        return None;
    }
    let type_name = right
        .split_once('{')
        .map(|(head, _)| head.trim())
        .filter(|head| is_identifier_path(head))?
        .rsplit("::")
        .next()?
        .to_string();
    Some(TypedLocalValue {
        name: name.to_string(),
        type_name,
    })
}

fn type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(text).ok()?;
    local_types::shallow_type_name(&ty)
}

fn split_top_level_commas(text: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        match ch {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(text[start..idx].trim());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(text[start..].trim());
    parts
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

fn is_identifier_path(value: &str) -> bool {
    !value.is_empty() && value.split("::").all(is_identifier)
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
