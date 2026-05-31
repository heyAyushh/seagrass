use {
    crate::{
        account_members::{self, ResolvedAccountMember},
        context_members,
        document::ParsedDocument,
        lsp::local_types,
        workspace::WorkspaceIndex,
    },
    syn::{
        visit::{self, Visit},
        Expr, ExprField, Member,
    },
    tower_lsp::lsp_types::{CompletionItem, CompletionTextEdit, Position, Range, TextEdit},
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct HandlerMemberAccess {
    receiver: Option<String>,
    receiver_type: Option<String>,
    member_chain: Vec<String>,
    member_prefix: String,
}

pub(super) fn completions(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<Vec<CompletionItem>> {
    let access = syntax_handler_member_access(document, position, workspace_index)
        .or_else(|| text_handler_member_access(document.source(), position))?;
    if let Some(receiver) = access.receiver.as_deref() {
        if let Some(context_type) = visible_receiver_context_type(document, position, receiver) {
            if let Some(members) = context_members::resolved_context_chain_members(
                document,
                workspace_index,
                &context_type,
                &access.member_chain,
            ) {
                return completion_items(position, &access.member_prefix, members);
            }
        }
    }

    let receiver_type = access.receiver_type.or_else(|| {
        access.receiver.as_deref().and_then(|receiver| {
            visible_receiver_type(document, position, workspace_index, receiver)
        })
    })?;
    if let Some(members) = context_members::resolved_generated_bumps_chain_members(
        document,
        workspace_index,
        &receiver_type,
        &access.member_chain,
    ) {
        return completion_items(position, &access.member_prefix, members);
    }
    let members = account_members::resolved_struct_chain_completion_members(
        document,
        workspace_index,
        &receiver_type,
        &access.member_chain,
    )?;
    completion_items(position, &access.member_prefix, members)
}

fn completion_items(
    position: Position,
    member_prefix: &str,
    members: account_members::ResolvedAccountMembers,
) -> Option<Vec<CompletionItem>> {
    let replacement_range = prefix_replacement_range(position, member_prefix);
    let mut items = members
        .members
        .iter()
        .filter(|member| matches_prefix(&member.name, member_prefix))
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

fn visible_receiver_context_type(
    document: &ParsedDocument,
    position: Position,
    receiver: &str,
) -> Option<String> {
    local_types::visible_context_values_at(document, position)
        .into_iter()
        .chain(local_types::text_visible_context_values_at(
            document, position,
        ))
        .rev()
        .find(|value| value.name == receiver)
        .map(|value| value.accounts_type_name)
}

fn syntax_handler_member_access(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Option<HandlerMemberAccess> {
    let mut visitor = SyntaxMemberAccessVisitor {
        document,
        position,
        workspace_index,
        access: None,
    };
    visitor.visit_file(document.syntax());
    visitor.access
}

struct SyntaxMemberAccessVisitor<'a> {
    document: &'a ParsedDocument,
    position: Position,
    workspace_index: Option<&'a WorkspaceIndex>,
    access: Option<HandlerMemberAccess>,
}

impl<'ast> Visit<'ast> for SyntaxMemberAccessVisitor<'_> {
    fn visit_expr_field(&mut self, node: &'ast ExprField) {
        if self.access.is_none() && member_contains_position(node, self.position) {
            self.access = self.access_from_field(node);
        }
        visit::visit_expr_field(self, node);
    }
}

impl SyntaxMemberAccessVisitor<'_> {
    fn access_from_field(&self, node: &ExprField) -> Option<HandlerMemberAccess> {
        let mut members = Vec::new();
        let receiver = collect_field_chain(node, &mut members)?;
        let member_prefix = members.pop()?;
        let receiver_type = self.receiver_type(receiver)?;
        Some(HandlerMemberAccess {
            receiver: receiver_name(receiver),
            receiver_type: Some(receiver_type),
            member_chain: members,
            member_prefix,
        })
    }

    fn receiver_type(&self, receiver: &Expr) -> Option<String> {
        let typed_values = local_types::visible_typed_values_at_with_workspace(
            self.document,
            self.position,
            self.workspace_index,
        );
        let context_values = local_types::visible_context_values_at(self.document, self.position);
        let iterable_values = local_types::visible_iterable_item_values_at_with_workspace(
            self.document,
            self.position,
            self.workspace_index,
        );
        local_types::expression_type_name_with_item_scope(
            self.document,
            self.workspace_index,
            receiver,
            &|name| {
                typed_values
                    .iter()
                    .rev()
                    .find(|value| value.name == name)
                    .map(|value| value.type_name.clone())
            },
            &|name| {
                context_values
                    .iter()
                    .rev()
                    .find(|value| value.name == name)
                    .map(|value| value.accounts_type_name.clone())
            },
            &|name| {
                iterable_values
                    .iter()
                    .rev()
                    .find(|value| value.name == name)
                    .map(|value| value.type_name.clone())
            },
        )
    }
}

fn member_contains_position(field: &ExprField, position: Position) -> bool {
    let Member::Named(member) = &field.member else {
        return false;
    };
    contains_position(crate::range::range_from_span(member.span()), position)
}

fn collect_field_chain<'a>(field: &'a ExprField, members: &mut Vec<String>) -> Option<&'a Expr> {
    let receiver = match field.base.as_ref() {
        Expr::Field(base) => collect_field_chain(base, members)?,
        Expr::Paren(paren) => match paren.expr.as_ref() {
            Expr::Field(base) => collect_field_chain(base, members)?,
            expr => expr,
        },
        Expr::Group(group) => match group.expr.as_ref() {
            Expr::Field(base) => collect_field_chain(base, members)?,
            expr => expr,
        },
        Expr::Reference(reference) => match reference.expr.as_ref() {
            Expr::Field(base) => collect_field_chain(base, members)?,
            expr => expr,
        },
        expr => expr,
    };
    let Member::Named(member) = &field.member else {
        return None;
    };
    members.push(member.to_string());
    Some(receiver)
}

fn receiver_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        Expr::Paren(paren) => receiver_name(&paren.expr),
        Expr::Group(group) => receiver_name(&group.expr),
        Expr::Reference(reference) => receiver_name(&reference.expr),
        _ => None,
    }
}

fn contains_position(range: Range, position: Position) -> bool {
    (position.line > range.start.line
        || (position.line == range.start.line && position.character >= range.start.character))
        && (position.line < range.end.line
            || (position.line == range.end.line && position.character <= range.end.character))
}

fn text_handler_member_access(source: &str, position: Position) -> Option<HandlerMemberAccess> {
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
        receiver: Some(receiver.to_string()),
        receiver_type: None,
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
