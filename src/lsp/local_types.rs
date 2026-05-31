use {
    crate::{
        account_members, context_members,
        document::ParsedDocument,
        lsp::scope::{TextHandlerBinding, TextHandlerScope},
        workspace::WorkspaceIndex,
    },
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        Expr, ExprField, FnArg, GenericArgument, ItemFn, Member, Pat, PathArguments, Stmt, Type,
        TypePath,
    },
    tower_lsp::lsp_types::Position,
};

const TRANSPARENT_LOCAL_TYPE_WRAPPERS: &[&str] = &["Box"];
const ACCOUNT_DATA_TYPE_WRAPPERS: &[&str] = &[
    "Account",
    "InterfaceAccount",
    "LazyAccount",
    "AccountLoader",
];
const TRANSPARENT_ACCOUNT_FIELD_WRAPPERS: &[&str] = &["Box", "Option"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypedLocalValue {
    pub(crate) name: String,
    pub(crate) type_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContextLocalValue {
    pub(crate) name: String,
    pub(crate) accounts_type_name: String,
}

pub(crate) fn visible_typed_values_at_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<TypedLocalValue> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleTypedValueCollector {
        document,
        workspace_index,
        source: document.source(),
        cursor_offset,
        values: Vec::new(),
        context_values: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector.values
}

pub(crate) fn visible_context_values_at(
    document: &ParsedDocument,
    position: Position,
) -> Vec<ContextLocalValue> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleTypedValueCollector {
        document,
        workspace_index: None,
        source: document.source(),
        cursor_offset,
        values: Vec::new(),
        context_values: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector
        .context_values
        .into_iter()
        .map(|value| ContextLocalValue {
            name: value.name,
            accounts_type_name: value.type_name,
        })
        .collect()
}

pub(crate) fn text_visible_typed_values_at_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<TypedLocalValue> {
    let Some(scope) = TextHandlerScope::at_position(document.source(), position) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for (idx, binding) in scope.bindings().iter().enumerate() {
        if let Some(value) = text_typed_value_from_binding(
            document,
            workspace_index,
            &values,
            &scope.bindings()[..idx],
            binding,
        ) {
            values.push(value);
        }
    }
    values
}

pub(crate) fn text_visible_context_values_at(
    document: &ParsedDocument,
    position: Position,
) -> Vec<ContextLocalValue> {
    let Some(scope) = TextHandlerScope::at_position(document.source(), position) else {
        return Vec::new();
    };
    scope
        .bindings()
        .iter()
        .filter_map(|binding| {
            binding
                .type_display
                .as_deref()
                .and_then(context_type_name_from_text)
                .map(|accounts_type_name| ContextLocalValue {
                    name: binding.name.clone(),
                    accounts_type_name,
                })
        })
        .collect()
}

pub(crate) fn context_type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(text).ok()?;
    context_type_name_from_type(&ty)
}

pub(crate) fn context_type_name_from_type(ty: &Type) -> Option<String> {
    context_type_name(ty)
}

pub(crate) fn account_data_type_name_from_text(type_text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(type_text).ok()?;
    account_data_type_name(&ty)
}

pub(crate) fn account_data_type_name_from_parts(
    type_name: Option<&str>,
    generic_type_names: &[String],
) -> Option<String> {
    generic_type_names
        .last()
        .cloned()
        .or_else(|| type_name.map(str::to_string))
}

pub(crate) fn text_context_account_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    bindings: &[TextHandlerBinding],
    expr: &Expr,
) -> Option<String> {
    let access = context_account_access(expr)?;
    let context_name = bindings.iter().rev().find_map(|binding| {
        (binding.name == access.context_binding)
            .then(|| {
                binding
                    .type_display
                    .as_deref()
                    .and_then(context_type_name_from_text)
            })
            .flatten()
    })?;
    context_account_field_type_name(
        document,
        workspace_index,
        &context_name,
        &access.account_field,
    )
}

fn context_account_field_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    context_name: &str,
    field_name: &str,
) -> Option<String> {
    document
        .symbols()
        .accounts_structs
        .get(context_name)
        .and_then(|accounts| {
            accounts
                .fields
                .iter()
                .find(|field| field.name == field_name)
                .and_then(|field| {
                    account_data_type_name_from_parts(
                        field.type_name.as_deref(),
                        &field.generic_type_names,
                    )
                })
        })
        .or_else(|| {
            workspace_index.and_then(|index| {
                index.accounts_struct(context_name).and_then(|accounts| {
                    accounts
                        .fields
                        .iter()
                        .find(|field| field.name == field_name)
                        .and_then(|field| {
                            account_data_type_name_from_parts(
                                field.type_name.as_deref(),
                                &field.generic_type_names,
                            )
                        })
                })
            })
        })
        .or_else(|| {
            document.tree_sitter().and_then(|syntax| {
                syntax
                    .struct_fields_named(document.source(), context_name)
                    .into_iter()
                    .find(|field| field.name.as_deref() == Some(field_name))
                    .and_then(|field| {
                        field
                            .type_text
                            .as_deref()
                            .and_then(account_data_type_name_from_text)
                    })
            })
        })
}

struct ContextAccountAccess {
    context_binding: String,
    account_field: String,
}

fn context_account_access(expr: &Expr) -> Option<ContextAccountAccess> {
    let segments = expression_field_segments(expr)?;
    context_account_access_from_segments(segments)
}

fn context_account_access_from_field(field: &ExprField) -> Option<ContextAccountAccess> {
    let segments = field_expression_segments(field)?;
    context_account_access_from_segments(segments)
}

fn context_account_access_from_segments(segments: Vec<String>) -> Option<ContextAccountAccess> {
    if segments.len() != 3 || segments.get(1).map(String::as_str) != Some("accounts") {
        return None;
    }
    Some(ContextAccountAccess {
        context_binding: segments[0].clone(),
        account_field: segments[2].clone(),
    })
}

fn expression_field_segments(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Some(vec![path.path.segments[0].ident.to_string()])
        }
        Expr::Field(field) => field_expression_segments(field),
        Expr::Reference(reference) => expression_field_segments(&reference.expr),
        Expr::Paren(paren) => expression_field_segments(&paren.expr),
        Expr::Group(group) => expression_field_segments(&group.expr),
        _ => None,
    }
}

fn field_expression_segments(field: &ExprField) -> Option<Vec<String>> {
    let mut segments = expression_field_segments(&field.base)?;
    let Member::Named(member) = &field.member else {
        return None;
    };
    segments.push(member.to_string());
    Some(segments)
}

fn context_type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = transparent_type_path(ty) else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    (segment.ident == "Context").then(|| first_type_argument(&segment.arguments))?
}

fn account_data_type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = transparent_type_path(ty) else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let wrapper = segment.ident.to_string();
    if TRANSPARENT_ACCOUNT_FIELD_WRAPPERS.contains(&wrapper.as_str()) {
        return first_type_argument_type(&segment.arguments).and_then(account_data_type_name);
    }
    if ACCOUNT_DATA_TYPE_WRAPPERS.contains(&wrapper.as_str()) {
        return last_type_argument(&segment.arguments);
    }
    Some(wrapper)
}

fn transparent_type_path(ty: &Type) -> &Type {
    match ty {
        Type::Reference(reference) => transparent_type_path(&reference.elem),
        Type::Ptr(pointer) => transparent_type_path(&pointer.elem),
        Type::Paren(paren) => transparent_type_path(&paren.elem),
        Type::Group(group) => transparent_type_path(&group.elem),
        _ => ty,
    }
}

fn first_type_argument(arguments: &PathArguments) -> Option<String> {
    first_type_argument_type(arguments).and_then(type_path_name)
}

fn last_type_argument(arguments: &PathArguments) -> Option<String> {
    let PathArguments::AngleBracketed(args) = arguments else {
        return None;
    };
    args.args.iter().rev().find_map(|arg| match arg {
        GenericArgument::Type(ty) => type_path_name(ty),
        _ => None,
    })
}

fn first_type_argument_type(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn type_path_name(ty: &Type) -> Option<String> {
    let Type::Path(TypePath { path, .. }) = transparent_type_path(ty) else {
        return None;
    };
    path.segments
        .last()
        .map(|segment| segment.ident.to_string())
}

pub(crate) fn shallow_type_name(ty: &Type) -> Option<String> {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        Type::Ptr(pointer) => pointer.elem.as_ref(),
        Type::Paren(paren) => paren.elem.as_ref(),
        Type::Group(group) => group.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let type_name = segment.ident.to_string();
    if !TRANSPARENT_LOCAL_TYPE_WRAPPERS.contains(&type_name.as_str()) {
        return Some(type_name);
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Some(type_name);
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return Some(type_name);
    };
    shallow_type_name(inner).or(Some(type_name))
}

pub(crate) fn constructed_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Struct(expr_struct) => expr_struct
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Reference(reference) => constructed_type_name(&reference.expr),
        Expr::Paren(paren) => constructed_type_name(&paren.expr),
        Expr::Group(group) => constructed_type_name(&group.expr),
        _ => None,
    }
}

pub(crate) fn expression_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    expression_type_name_with_context_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        &|_| None,
    )
}

fn expression_type_name_with_context_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            scope_type_name(&path.path.segments[0].ident.to_string())
        }
        Expr::Field(field) => context_account_field_type_name_from_expr(
            document,
            workspace_index,
            field,
            context_type_name,
        )
        .or_else(|| context_member_type_name_from_expr(field, context_type_name))
        .or_else(|| {
            field_expression_type_name(
                document,
                workspace_index,
                field,
                scope_type_name,
                context_type_name,
            )
        }),
        Expr::Reference(reference) => expression_type_name_with_context_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
        ),
        Expr::Paren(paren) => expression_type_name_with_context_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
        ),
        Expr::Group(group) => expression_type_name_with_context_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
        ),
        _ => constructed_type_name(expr),
    }
}

fn context_account_field_type_name_from_expr(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &ExprField,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let access = context_account_access_from_field(field)?;
    let context_name = context_type_name(&access.context_binding)?;
    context_account_field_type_name(
        document,
        workspace_index,
        &context_name,
        &access.account_field,
    )
}

fn context_member_type_name_from_expr(
    field: &ExprField,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let segments = field_expression_segments(field)?;
    if segments.len() != 2 {
        return None;
    }
    let context_name = context_type_name(&segments[0])?;
    match segments[1].as_str() {
        context_members::CONTEXT_ACCOUNTS_MEMBER => Some(context_name),
        context_members::CONTEXT_BUMPS_MEMBER => {
            Some(context_members::generated_bumps_type(&context_name))
        }
        _ => None,
    }
}

fn field_expression_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    field: &ExprField,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let base_type = expression_type_name_with_context_scope(
        document,
        workspace_index,
        &field.base,
        scope_type_name,
        context_type_name,
    )?;
    let Member::Named(member) = &field.member else {
        return None;
    };
    account_members::resolved_struct_members(document, workspace_index, &base_type)?
        .members
        .iter()
        .find(|candidate| candidate.name == member.to_string())?
        .type_name
        .clone()
}

fn text_typed_value_from_binding(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    visible_values: &[TypedLocalValue],
    visible_bindings: &[TextHandlerBinding],
    binding: &TextHandlerBinding,
) -> Option<TypedLocalValue> {
    let type_name = binding
        .type_display
        .as_deref()
        .and_then(type_name_from_text)
        .or_else(|| {
            binding.initializer_text.as_deref().and_then(|initializer| {
                let expr = syn::parse_str::<syn::Expr>(initializer).ok()?;
                text_context_account_type_name(document, workspace_index, visible_bindings, &expr)
            })
        })
        .or_else(|| {
            binding
                .initializer_text
                .as_deref()
                .and_then(text_constructed_type_name)
        })
        .or_else(|| {
            binding.initializer_text.as_deref().and_then(|initializer| {
                text_inferred_type_name(document, workspace_index, visible_values, initializer)
            })
        })?;

    Some(TypedLocalValue {
        name: binding.name.clone(),
        type_name,
    })
}

fn type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(text).ok()?;
    shallow_type_name(&ty)
}

fn text_constructed_type_name(initializer: &str) -> Option<String> {
    initializer
        .split_once('{')
        .map(|(head, _)| head.trim())
        .filter(|head| is_identifier_path(head))?
        .rsplit("::")
        .next()
        .map(str::to_string)
}

fn text_inferred_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    visible_values: &[TypedLocalValue],
    initializer: &str,
) -> Option<String> {
    let expr = syn::parse_str::<syn::Expr>(initializer).ok()?;
    expression_type_name_with_scope(document, workspace_index, &expr, &|name| {
        visible_values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    })
}

fn is_identifier_path(value: &str) -> bool {
    !value.is_empty() && value.split("::").all(is_identifier)
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || ch == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

struct VisibleTypedValueCollector<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    source: &'a str,
    cursor_offset: usize,
    values: Vec<TypedLocalValue>,
    context_values: Vec<TypedLocalValue>,
    found_cursor_function: bool,
}

impl VisibleTypedValueCollector<'_> {
    fn collect_function(&mut self, item_fn: &ItemFn) {
        self.collect_function_inputs(item_fn);
        self.collect_block_bindings(&item_fn.block);
        self.found_cursor_function = true;
    }

    fn collect_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            let FnArg::Typed(pat_type) = input else {
                continue;
            };
            if let Some(context_name) = context_type_name_from_type(&pat_type.ty) {
                self.add_context_pattern_candidate(&pat_type.pat, context_name);
            }
            let Some(type_name) = shallow_type_name(&pat_type.ty) else {
                continue;
            };
            self.add_pattern_candidate(&pat_type.pat, type_name);
        }
    }

    fn collect_block_bindings(&mut self, block: &syn::Block) -> bool {
        for stmt in &block.stmts {
            let stmt_range = crate::range::range_from_span(stmt.span());
            let Some(stmt_start) = crate::range::byte_offset_at(self.source, stmt_range.start)
            else {
                continue;
            };
            let Some(stmt_end) = crate::range::byte_offset_at(self.source, stmt_range.end) else {
                continue;
            };

            if self.cursor_offset < stmt_start {
                return true;
            }
            if self.cursor_offset <= stmt_end {
                return self.collect_bindings_inside_statement(stmt);
            }
            if let Stmt::Local(local) = stmt {
                self.collect_local(local);
            }
        }
        false
    }

    fn collect_bindings_inside_statement(&mut self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Expr(expr, _) => self.collect_bindings_inside_expr(expr),
            Stmt::Local(_) | Stmt::Item(_) | Stmt::Macro(_) => true,
        }
    }

    fn collect_bindings_inside_expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Block(block) => self.collect_block_bindings(&block.block),
            Expr::ForLoop(for_loop) if self.span_contains_cursor(for_loop.body.span()) => {
                self.collect_block_bindings(&for_loop.body)
            }
            Expr::If(if_expr) => {
                if self.span_contains_cursor(if_expr.then_branch.span()) {
                    return self.collect_block_bindings(&if_expr.then_branch);
                }
                if let Some((_, else_branch)) = &if_expr.else_branch {
                    return self.collect_bindings_inside_expr(else_branch);
                }
                true
            }
            Expr::Loop(loop_expr) if self.span_contains_cursor(loop_expr.body.span()) => {
                self.collect_block_bindings(&loop_expr.body)
            }
            Expr::While(while_expr) if self.span_contains_cursor(while_expr.body.span()) => {
                self.collect_block_bindings(&while_expr.body)
            }
            Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
                    if self.span_contains_cursor(arm.body.span()) {
                        return self.collect_bindings_inside_expr(&arm.body);
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn collect_local(&mut self, local: &syn::Local) {
        let Some(type_name) = local_type_name_with_scope(
            self.document,
            self.workspace_index,
            local,
            &|name| self.visible_type_name(name),
            &|name| self.visible_context_type_name(name),
        ) else {
            return;
        };
        self.add_pattern_candidate(&local.pat, type_name);
    }

    fn add_pattern_candidate(&mut self, pat: &Pat, type_name: String) {
        let Some(name) = crate::lsp::scope::pattern_binding_name(pat) else {
            return;
        };
        self.values.push(TypedLocalValue { name, type_name });
    }

    fn add_context_pattern_candidate(&mut self, pat: &Pat, type_name: String) {
        let Some(name) = crate::lsp::scope::pattern_binding_name(pat) else {
            return;
        };
        self.context_values
            .push(TypedLocalValue { name, type_name });
    }

    fn span_contains_cursor(&self, span: proc_macro2::Span) -> bool {
        let range = crate::range::range_from_span(span);
        let Some(start) = crate::range::byte_offset_at(self.source, range.start) else {
            return false;
        };
        let Some(end) = crate::range::byte_offset_at(self.source, range.end) else {
            return false;
        };
        start <= self.cursor_offset && self.cursor_offset <= end
    }

    fn visible_type_name(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    }

    fn visible_context_type_name(&self, name: &str) -> Option<String> {
        self.context_values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    }
}

impl<'ast> Visit<'ast> for VisibleTypedValueCollector<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.found_cursor_function || !self.span_contains_cursor(node.block.span()) {
            return;
        }
        self.collect_function(node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !self.found_cursor_function {
            visit::visit_item_mod(self, node);
        }
    }
}

pub(crate) fn local_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    local: &syn::Local,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    explicit_pattern_type_name(&local.pat).or_else(|| {
        local.init.as_ref().and_then(|init| {
            expression_type_name_with_context_scope(
                document,
                workspace_index,
                &init.expr,
                scope_type_name,
                context_type_name,
            )
        })
    })
}

fn explicit_pattern_type_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Type(typed) => shallow_type_name(&typed.ty),
        Pat::Reference(reference) => explicit_pattern_type_name(&reference.pat),
        Pat::Paren(paren) => explicit_pattern_type_name(&paren.pat),
        _ => None,
    }
}
