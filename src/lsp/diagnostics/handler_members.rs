use {
    super::{
        diagnostic_from_range,
        lint::{run_lint_visitor, Applicability, Confidence, LintVisitor, Region},
        registry::AnchorDiagnosticKind,
    },
    crate::{
        account_members, context_members,
        document::ParsedDocument,
        lsp::{
            local_types,
            scope::{
                has_attr, item_fn_has_anchor_context_arg, pattern_binding_name, TextHandlerScope,
            },
        },
        range::range_from_span,
        workspace::WorkspaceIndex,
    },
    std::collections::{HashMap, HashSet},
    syn::{
        visit::{self, Visit},
        Expr, ExprField, FnArg, ItemFn, ItemMod, Member,
    },
    tower_lsp::lsp_types::{Diagnostic, Position, Range},
};

const TOPIC: &str = "seagrass/anchor.account.usage";
const REASON: &str = "unknown-handler-member";
const EVIDENCE_SOURCE: &str = "parsed-anchor-handler-local-types";

pub fn collect_with_workspace(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let mut diagnostics = run_lint_visitor(
        document,
        HandlerMemberVisitor::new(document, workspace_index),
    );
    if document.syntax().items.is_empty() {
        diagnostics.extend(collect_text_recovered_members(document, workspace_index));
    }
    diagnostics
}

struct HandlerMemberVisitor<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    scopes: TypedScopeStack,
    in_program_module: bool,
    diagnostics: Vec<Diagnostic>,
    emitted: HashSet<(u32, u32)>,
}

impl<'a> HandlerMemberVisitor<'a> {
    fn new(document: &'a ParsedDocument, workspace_index: Option<&'a WorkspaceIndex>) -> Self {
        Self {
            document,
            workspace_index,
            scopes: TypedScopeStack::default(),
            in_program_module: false,
            diagnostics: Vec::new(),
            emitted: HashSet::new(),
        }
    }

    fn visit_anchor_function(&mut self, item_fn: &ItemFn) {
        self.scopes.push();
        self.declare_function_inputs(item_fn);
        self.visit_block(&item_fn.block);
        self.scopes.pop();
    }

    fn declare_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            let FnArg::Typed(pat_type) = input else {
                continue;
            };
            if let Some(context_name) = local_types::context_type_name_from_type(&pat_type.ty) {
                self.scopes.declare_context_pat(&pat_type.pat, context_name);
            }
            let Some(type_name) = local_types::shallow_type_name(&pat_type.ty) else {
                continue;
            };
            self.scopes.declare_pat(&pat_type.pat, type_name);
        }
    }

    fn report_unknown_member(&mut self, node: &ExprField) {
        let Some(access) = FieldAccess::from_expr_field(node) else {
            return;
        };
        if let Some(context_type) = self.scopes.get_context(&access.receiver).or_else(|| {
            text_receiver_context_type(self.document, access.end_position()?, &access.receiver)
        }) {
            if let Some(diagnostic) = unknown_context_member_diagnostic(
                self.document,
                self.workspace_index,
                &context_type,
                &access,
            ) {
                self.push_diagnostic_once(diagnostic);
                return;
            }
        }

        let Some(receiver_type) = self.scopes.get(&access.receiver).or_else(|| {
            text_receiver_type(
                self.document,
                self.workspace_index,
                access.end_position()?,
                &access.receiver,
            )
        }) else {
            return;
        };
        let Some(diagnostic) =
            unknown_member_diagnostic(self.document, self.workspace_index, &receiver_type, &access)
        else {
            return;
        };
        self.push_diagnostic_once(diagnostic);
    }

    fn push_diagnostic_once(&mut self, diagnostic: Diagnostic) {
        let key = (
            diagnostic.range.start.line,
            diagnostic.range.start.character,
        );
        if self.emitted.insert(key) {
            self.diagnostics.push(diagnostic);
        }
    }
}

impl<'ast> LintVisitor<'ast> for HandlerMemberVisitor<'_> {
    const SCOPE: &'static [Region] = &[Region::InstructionBody, Region::HelperFnBody];
    const CONFIDENCE: Confidence = Confidence::Derived;
    const APPLICABILITY: Applicability = Applicability::Unspecified;
    const TOPIC: &'static str = TOPIC;

    fn finish(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl<'ast> Visit<'ast> for HandlerMemberVisitor<'_> {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let was_program_module = self.in_program_module;
        if has_attr(&node.attrs, "program") {
            self.in_program_module = true;
        }
        visit::visit_item_mod(self, node);
        self.in_program_module = was_program_module;
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.in_program_module || item_fn_has_anchor_context_arg(node) {
            self.visit_anchor_function(node);
        }
    }

    fn visit_block(&mut self, node: &'ast syn::Block) {
        self.scopes.push();
        visit::visit_block(self, node);
        self.scopes.pop();
    }

    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let Some(init) = &node.init {
            self.visit_expr(&init.expr);
            if let Some((_, diverge)) = &init.diverge {
                self.visit_expr(diverge);
            }
        }
        if let Some(type_name) = local_types::local_type_name_with_scope(
            self.document,
            self.workspace_index,
            node,
            &|name| self.scopes.get(name),
            &|name| self.scopes.get_context(name),
        ) {
            self.scopes.declare_pat(&node.pat, type_name);
        }
    }

    fn visit_expr_field(&mut self, node: &'ast ExprField) {
        self.report_unknown_member(node);
        visit::visit_expr_field(self, node);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldAccess {
    receiver: String,
    members: Vec<FieldMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldMember {
    name: String,
    range: Range,
}

impl FieldAccess {
    fn from_expr_field(node: &ExprField) -> Option<Self> {
        let mut members = Vec::new();
        collect_field_access(node, &mut members).map(|receiver| Self { receiver, members })
    }

    fn end_position(&self) -> Option<Position> {
        self.members.last().map(|member| member.range.end)
    }
}

fn collect_field_access(node: &ExprField, members: &mut Vec<FieldMember>) -> Option<String> {
    let receiver = match node.base.as_ref() {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            path.path.segments[0].ident.to_string()
        }
        Expr::Field(base) => collect_field_access(base, members)?,
        Expr::Paren(paren) => {
            let Expr::Field(field) = paren.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        Expr::Group(group) => {
            let Expr::Field(field) = group.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        Expr::Reference(reference) => {
            let Expr::Field(field) = reference.expr.as_ref() else {
                return None;
            };
            collect_field_access(field, members)?
        }
        _ => return None,
    };
    let Member::Named(ident) = &node.member else {
        return None;
    };
    members.push(FieldMember {
        name: ident.to_string(),
        range: range_from_span(ident.span()),
    });
    Some(receiver)
}

fn unknown_member_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    access: &FieldAccess,
) -> Option<Diagnostic> {
    let member_names = access
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    if let Some(missing) = context_members::missing_generated_bumps_member_in_chain(
        document,
        workspace_index,
        receiver_type,
        &access.receiver,
        &member_names,
    ) {
        let member_range = access.members.get(missing.member_index)?.range;
        return Some(handler_member_diagnostic(
            member_range,
            access,
            receiver_type,
            missing,
        ));
    }

    let mut owner_type = receiver_type.to_string();
    let mut members =
        account_members::resolved_struct_members(document, workspace_index, &owner_type)?;
    let mut receiver_path = access.receiver.clone();

    for member in &access.members {
        let Some(resolved) = members
            .members
            .iter()
            .find(|candidate| candidate.name == member.name)
        else {
            return Some(handler_member_diagnostic(
                member.range,
                access,
                receiver_type,
                context_members::MissingContextMember {
                    member_index: 0,
                    receiver_path,
                    member: member.name.clone(),
                    owner_type: members.owner_type,
                    candidates: members
                        .members
                        .iter()
                        .map(|member| member.name.clone())
                        .collect::<Vec<_>>(),
                },
            ));
        };

        receiver_path.push('.');
        receiver_path.push_str(&member.name);
        let Some(next_type) = resolved.type_name.as_ref() else {
            return None;
        };
        owner_type = next_type.clone();
        members = account_members::resolved_struct_members(document, workspace_index, &owner_type)?;
    }

    None
}

fn unknown_context_member_diagnostic(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    context_type: &str,
    access: &FieldAccess,
) -> Option<Diagnostic> {
    let member_names = access
        .members
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    let missing = context_members::missing_context_member_in_chain(
        document,
        workspace_index,
        context_type,
        &access.receiver,
        &member_names,
    )?;
    let member_range = access.members.get(missing.member_index)?.range;

    Some(handler_member_diagnostic(
        member_range,
        access,
        context_type,
        missing,
    ))
}

fn handler_member_diagnostic(
    range: Range,
    access: &FieldAccess,
    receiver_type: &str,
    missing: context_members::MissingContextMember,
) -> Diagnostic {
    diagnostic_from_range(
        range,
        AnchorDiagnosticKind::AnchorMissingAccountReference,
        format!(
            "`{}.{}` does not resolve; `{}` has no field `{}`.",
            missing.receiver_path, missing.member, missing.owner_type, missing.member
        ),
        Some(serde_json::json!({
            "topic": TOPIC,
            "reason": REASON,
            "receiver": access.receiver,
            "receiverType": receiver_type,
            "field": missing.member,
            "ownerType": missing.owner_type,
            "candidates": missing.candidates,
            "evidenceSource": EVIDENCE_SOURCE,
            "confidence": Confidence::Derived.as_str(),
            "applicability": Applicability::Unspecified.as_str(),
        })),
    )
}

fn collect_text_recovered_members(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut emitted = HashSet::new();
    for (line_idx, line) in document.source().lines().enumerate() {
        let Ok(line_number) = u32::try_from(line_idx) else {
            continue;
        };
        let code = line_code_before_comment(line);
        for access in text_member_accesses(code, line_number) {
            let position = access.end_position;
            let Some(scope) = TextHandlerScope::at_position(document.source(), position) else {
                continue;
            };
            if !scope.has_anchor_context() {
                continue;
            }
            if let Some(context_type) =
                text_receiver_context_type(document, position, &access.receiver)
            {
                if let Some(diagnostic) = unknown_context_member_diagnostic(
                    document,
                    workspace_index,
                    &context_type,
                    &access.clone().into(),
                ) {
                    let key = (
                        diagnostic.range.start.line,
                        diagnostic.range.start.character,
                    );
                    if emitted.insert(key) {
                        diagnostics.push(diagnostic);
                    }
                    continue;
                }
            }
            let Some(receiver_type) =
                text_receiver_type(document, workspace_index, position, &access.receiver)
            else {
                continue;
            };
            let Some(diagnostic) = unknown_member_diagnostic(
                document,
                workspace_index,
                &receiver_type,
                &access.into(),
            ) else {
                continue;
            };
            let key = (
                diagnostic.range.start.line,
                diagnostic.range.start.character,
            );
            if emitted.insert(key) {
                diagnostics.push(diagnostic);
            }
        }
    }
    diagnostics
}

fn text_receiver_type(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    position: Position,
    receiver: &str,
) -> Option<String> {
    local_types::text_visible_typed_values_at_with_workspace(document, position, workspace_index)
        .into_iter()
        .rev()
        .find(|value| value.name == receiver)
        .map(|value| value.type_name)
}

fn text_receiver_context_type(
    document: &ParsedDocument,
    position: Position,
    receiver: &str,
) -> Option<String> {
    local_types::text_visible_context_values_at(document, position)
        .into_iter()
        .rev()
        .find(|value| value.name == receiver)
        .map(|value| value.accounts_type_name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextMemberAccess {
    receiver: String,
    members: Vec<FieldMember>,
    end_position: Position,
}

impl From<TextMemberAccess> for FieldAccess {
    fn from(access: TextMemberAccess) -> Self {
        Self {
            receiver: access.receiver,
            members: access.members,
        }
    }
}

fn text_member_accesses(line: &str, line_number: u32) -> Vec<TextMemberAccess> {
    let mut accesses = Vec::new();
    let mut idx = 0usize;
    while idx < line.len() {
        let Some((receiver, receiver_end)) = parse_identifier_at(line, idx) else {
            idx = next_char_boundary(line, idx);
            continue;
        };
        let mut cursor = receiver_end;
        let mut members = Vec::new();
        while line.as_bytes().get(cursor) == Some(&b'.') {
            let member_start = cursor + '.'.len_utf8();
            let Some((member, member_end)) = parse_identifier_at(line, member_start) else {
                break;
            };
            members.push(FieldMember {
                name: member,
                range: Range {
                    start: position_for_line_byte(line, line_number, member_start),
                    end: position_for_line_byte(line, line_number, member_end),
                },
            });
            cursor = member_end;
        }
        if !members.is_empty() && is_member_access_boundary(line, idx, cursor) {
            accesses.push(TextMemberAccess {
                receiver,
                members,
                end_position: position_for_line_byte(line, line_number, cursor),
            });
        }
        idx = next_char_boundary(line, cursor.max(idx + 1));
    }
    accesses
}

fn parse_identifier_at(line: &str, start: usize) -> Option<(String, usize)> {
    let tail = line.get(start..)?;
    let mut chars = tail.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }
    let mut end = start + first.len_utf8();
    for (relative_idx, ch) in chars {
        if !is_identifier_char(ch) {
            break;
        }
        end = start + relative_idx + ch.len_utf8();
    }
    Some((line[start..end].to_string(), end))
}

fn is_member_access_boundary(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_char(ch) && ch != '.')
        && after.is_none_or(|ch| !is_identifier_char(ch) && ch != '.')
}

fn line_code_before_comment(line: &str) -> &str {
    let mut escaped = false;
    let mut in_string = false;
    let mut previous = '\0';
    for (idx, ch) in line.char_indices() {
        if ch == '"' && previous != '\'' && !escaped {
            in_string = !in_string;
        }
        if !in_string && previous == '/' && ch == '/' {
            return &line[..idx - '/'.len_utf8()];
        }
        escaped = ch == '\\' && !escaped;
        if ch != '\\' {
            escaped = false;
        }
        previous = ch;
    }
    line
}

fn position_for_line_byte(line: &str, line_number: u32, byte: usize) -> Position {
    Position {
        line: line_number,
        character: u32::try_from(line[..byte.min(line.len())].chars().count()).unwrap_or_default(),
    }
}

fn next_char_boundary(line: &str, idx: usize) -> usize {
    if idx >= line.len() {
        return line.len();
    }
    line[idx..]
        .chars()
        .next()
        .map_or(line.len(), |ch| idx + ch.len_utf8())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[derive(Default)]
struct TypedScopeStack {
    scopes: Vec<HashMap<String, String>>,
    context_scopes: Vec<HashMap<String, String>>,
}

impl TypedScopeStack {
    fn push(&mut self) {
        self.scopes.push(HashMap::new());
        self.context_scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop();
        self.context_scopes.pop();
    }

    fn declare_pat(&mut self, pat: &syn::Pat, type_name: String) {
        let Some(name) = pattern_binding_name(pat) else {
            return;
        };
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, type_name);
        }
    }

    fn declare_context_pat(&mut self, pat: &syn::Pat, type_name: String) {
        let Some(name) = pattern_binding_name(pat) else {
            return;
        };
        if let Some(scope) = self.context_scopes.last_mut() {
            scope.insert(name, type_name);
        }
    }

    fn get(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    fn get_context(&self, name: &str) -> Option<String> {
        self.context_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}

#[cfg(test)]
mod tests;
