use {
    crate::{
        account_members, context_members,
        document::ParsedDocument,
        lsp::scope::{TextHandlerBinding, TextHandlerScope},
        workspace::WorkspaceIndex,
    },
    syn::{Expr, ExprField, Member, Pat, Type},
    tower_lsp::lsp_types::Position,
};

mod account_loader;
mod call_returns;
mod expression_branches;
mod iterables;
mod iterator_chains;
mod method_returns;
mod patterns;
mod text_inference;
mod type_names;
mod visible;
mod wrapper_outputs;

const TRANSPARENT_RECEIVER_METHODS: &[&str] = &["as_ref", "deref", "deref_mut"];

pub(crate) use {
    iterator_chains::{
        expression_iterable_item_type_name_with_item_scope, iterator_method_closure_item_type_name,
    },
    patterns::typed_pattern_bindings,
    patterns::typed_pattern_bindings_with_wrapped_item,
    type_names::{
        account_data_type_name_from_parts, account_data_type_name_from_text,
        context_type_name_from_text, context_type_name_from_type, shallow_type_name,
    },
};

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
    visible::typed_values_at(document, position, workspace_index)
}

pub(crate) fn visible_iterable_item_values_at_with_workspace(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<TypedLocalValue> {
    visible::iterable_item_values_at(document, position, workspace_index)
}

pub(crate) fn visible_context_values_at(
    document: &ParsedDocument,
    position: Position,
) -> Vec<ContextLocalValue> {
    visible::context_values_at(document, position)
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
        if let Some(value) = text_inference::typed_value_from_binding(
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

pub(crate) fn expression_type_name_with_context_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    expression_type_name_with_item_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        context_type_name,
        &|_| None,
    )
}

pub(crate) fn expression_type_name_with_item_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            scope_type_name(&path.path.segments[0].ident.to_string())
                .or_else(|| type_names::wrapper_non_value_path_type_name(path))
        }
        Expr::Index(_) => iterator_chains::accessed_item_type_name_with_scope(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
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
                scope_item_type_name,
            )
        }),
        Expr::Try(expr_try) => iterator_chains::accessed_item_type_name_with_scope(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .or_else(|| {
            method_returns::try_method_return_type_name(
                document,
                workspace_index,
                &expr_try.expr,
                scope_type_name,
                context_type_name,
            )
        })
        .or_else(|| {
            call_returns::try_call_return_type_name(document, workspace_index, &expr_try.expr)
        })
        .or_else(|| {
            expression_type_name_with_item_scope(
                document,
                workspace_index,
                &expr_try.expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::Call(call) => type_names::wrapper_constructor_type_name(call)
            .or_else(|| type_names::wrapper_non_value_constructor_type_name(call))
            .or_else(|| call_returns::call_return_type_name(document, workspace_index, call))
            .or_else(|| constructed_type_name(expr)),
        Expr::Block(block) => expression_branches::block_type_name(
            document,
            workspace_index,
            &block.block,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::If(expr_if) => expression_branches::if_expression_type_name(
            document,
            workspace_index,
            expr_if,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Match(expr_match) => expression_branches::match_expression_type_name(
            document,
            workspace_index,
            expr_match,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::MethodCall(method_call) => iterator_chains::accessed_item_type_name_with_scope(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .or_else(|| {
            wrapper_outputs::method_output_type_name(
                document,
                workspace_index,
                method_call,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        })
        .or_else(|| {
            account_loader_loaded_method_type_name(
                document,
                workspace_index,
                method_call,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        })
        .or_else(|| {
            transparent_receiver_method_type_name(
                document,
                workspace_index,
                method_call,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        })
        .or_else(|| {
            method_returns::method_return_type_name(
                document,
                workspace_index,
                method_call,
                scope_type_name,
                context_type_name,
            )
        })
        .or_else(|| constructed_type_name(expr)),
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            transparent_deref_type_name(
                document,
                workspace_index,
                unary,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }
        Expr::Reference(reference) => expression_type_name_with_item_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => expression_type_name_with_item_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => expression_type_name_with_item_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => constructed_type_name(expr),
    }
}

fn account_loader_loaded_method_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &syn::ExprMethodCall,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let receiver_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    account_loader::loaded_method_return_type(method_call, &receiver_type)
}

fn transparent_receiver_method_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &syn::ExprMethodCall,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if !method_call.args.is_empty()
        || !TRANSPARENT_RECEIVER_METHODS.contains(&method_call.method.to_string().as_str())
        || context_account_access(&method_call.receiver).is_none()
    {
        return None;
    }
    let receiver_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    account_members::resolved_struct_members(document, workspace_index, &receiver_type)
        .map(|_| receiver_type)
}

fn transparent_deref_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    unary: &syn::ExprUnary,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let receiver_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        &unary.expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    account_members::resolved_struct_members(document, workspace_index, &receiver_type)
        .map(|_| receiver_type)
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
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let base_type = expression_type_name_with_item_scope(
        document,
        workspace_index,
        &field.base,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let Member::Named(member) = &field.member else {
        return None;
    };
    account_members::resolved_struct_member_type_name(
        document,
        workspace_index,
        &base_type,
        &member.to_string(),
    )
}

pub(crate) fn local_type_name_with_item_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    local: &syn::Local,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    explicit_pattern_type_name(&local.pat).or_else(|| {
        local.init.as_ref().and_then(|init| {
            expression_type_name_with_item_scope(
                document,
                workspace_index,
                &init.expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        })
    })
}

pub(crate) fn assignment_target_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            Some(path.path.segments[0].ident.to_string())
        }
        Expr::Paren(paren) => assignment_target_name(&paren.expr),
        Expr::Group(group) => assignment_target_name(&group.expr),
        _ => None,
    }
}

pub(crate) fn local_iterable_item_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    local: &syn::Local,
    scope_type_name: &dyn Fn(&str) -> Option<String>,
    context_type_name: &dyn Fn(&str) -> Option<String>,
    scope_item_type_name: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    explicit_pattern_iterable_item_type_name(&local.pat).or_else(|| {
        local.init.as_ref().and_then(|init| {
            expression_iterable_item_type_name_with_item_scope(
                document,
                workspace_index,
                &init.expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        })
    })
}

pub(crate) fn expression_optional_item_type_name_with_item_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &dyn Fn(&str) -> Option<String>,
    context_type_name: &dyn Fn(&str) -> Option<String>,
    scope_item_type_name: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    iterator_chains::optional_item_type_name_with_scope(
        document,
        workspace_index,
        expr,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

pub(crate) fn explicit_pattern_iterable_item_type_name(pat: &Pat) -> Option<String> {
    iterables::explicit_pattern_item_type_name(pat)
}

pub(crate) fn wrapper_pattern_type_name(pat: &Pat) -> Option<&'static str> {
    patterns::wrapper_type_name(pat)
}

pub(crate) fn explicit_pattern_type_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Type(typed) => local_value_type_name_from_type(&typed.ty),
        Pat::Reference(reference) => explicit_pattern_type_name(&reference.pat),
        Pat::Paren(paren) => explicit_pattern_type_name(&paren.pat),
        _ => None,
    }
}

pub(crate) fn local_value_type_name_from_type(ty: &Type) -> Option<String> {
    account_loader::local_type_name_from_type(ty).or_else(|| shallow_type_name(ty))
}

pub(crate) fn iterable_item_type_name_from_type(ty: &Type) -> Option<String> {
    iterables::item_type_name_from_type(ty)
}
