use {
    super::{
        expression_type_name_with_item_scope, iterables, local_value_type_name_from_type,
        type_names, typed_pattern_bindings,
    },
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{Expr, ExprClosure, ExprMethodCall, ReturnType},
};

mod wrapper_branches;

type TypeLookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

const WRAPPER_AND_THEN_METHOD: &str = "and_then";
const WRAPPER_MAP_METHOD: &str = "map";

struct WrapperValue {
    kind: type_names::ValueWrapperKind,
    type_name: String,
}

struct WrapperClosureInput<'a> {
    type_name: &'a str,
    expected_wrapper: type_names::ValueWrapperKind,
}

pub(super) fn accessed_item_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    match expr {
        Expr::Index(index) => expression_iterable_item_type_name_with_item_scope(
            document,
            workspace_index,
            &index.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::MethodCall(method_call)
            if iterables::option_value_method_matches(
                &method_call.method.to_string(),
                method_call.args.len(),
            ) =>
        {
            optional_item_type_name_with_scope(
                document,
                workspace_index,
                &method_call.receiver,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }
        Expr::Try(expr_try) => optional_item_type_name_with_scope(
            document,
            workspace_index,
            &expr_try.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Reference(reference) => accessed_item_type_name_with_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => accessed_item_type_name_with_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => accessed_item_type_name_with_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => None,
    }
}

pub(super) fn optional_item_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    match expr {
        Expr::Call(call) => wrapper_constructor_value_type_name(
            document,
            workspace_index,
            call,
            None,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::If(expr_if) => wrapper_branches::if_value_type_name(
            document,
            workspace_index,
            expr_if,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::Match(expr_match) => wrapper_branches::match_value_type_name(
            document,
            workspace_index,
            expr_match,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::Block(block) => wrapper_branches::block_tail_expr(&block.block).and_then(|expr| {
            optional_item_type_name_with_scope(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            wrapper_value_type_name_with_scope(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
            .map(|value| value.type_name)
        }
        Expr::MethodCall(method_call)
            if iterables::collection_option_item_method_matches(
                &method_call.method.to_string(),
                method_call.args.len(),
            ) =>
        {
            expression_iterable_item_type_name_with_item_scope(
                document,
                workspace_index,
                &method_call.receiver,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }
        Expr::MethodCall(method_call)
            if iterables::iterator_option_item_method_matches(
                &method_call.method.to_string(),
                method_call.args.len(),
            ) =>
        {
            expression_iterable_item_type_name_with_item_scope(
                document,
                workspace_index,
                &method_call.receiver,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }
        Expr::MethodCall(_) => wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::Reference(reference) => optional_item_type_name_with_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => optional_item_type_name_with_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => optional_item_type_name_with_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => None,
    }
}

pub(crate) fn expression_iterable_item_type_name_with_item_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    match expr {
        Expr::Call(call) => wrapper_constructor_value_type_name(
            document,
            workspace_index,
            call,
            None,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::If(expr_if) => wrapper_branches::if_value_type_name(
            document,
            workspace_index,
            expr_if,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::Match(expr_match) => wrapper_branches::match_value_type_name(
            document,
            workspace_index,
            expr_match,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        )
        .map(|value| value.type_name),
        Expr::Block(block) => wrapper_branches::block_tail_expr(&block.block).and_then(|expr| {
            expression_iterable_item_type_name_with_item_scope(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::MethodCall(method_call)
            if iterables::item_transforming_iterator_method_matches(
                &method_call.method.to_string(),
                method_call.args.len(),
            ) =>
        {
            iterator_map_item_type_name(
                document,
                workspace_index,
                method_call,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }
        Expr::Reference(reference) => expression_iterable_item_type_name_with_item_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => expression_iterable_item_type_name_with_item_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => expression_iterable_item_type_name_with_item_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => {
            let item_lookup = |name: &str| scope_item_type_name(name);
            iterables::expression_item_type_name(expr, &item_lookup)
        }
    }
}

pub(crate) fn iterator_method_closure_item_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    let method = method_call.method.to_string();
    if iterables::wrapper_closure_value_method_matches(&method, method_call.args.len()) {
        if let Some(value) = wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            &method_call.receiver,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ) {
            return Some(value.type_name);
        }
    }
    if !iterables::item_closure_iterator_method_matches(&method, method_call.args.len()) {
        return None;
    }
    expression_iterable_item_type_name_with_item_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn wrapper_value_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    match expr {
        Expr::Call(call) => wrapper_constructor_value_type_name(
            document,
            workspace_index,
            call,
            None,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::If(expr_if) => wrapper_branches::if_value_type_name(
            document,
            workspace_index,
            expr_if,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Match(expr_match) => wrapper_branches::match_value_type_name(
            document,
            workspace_index,
            expr_match,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Block(block) => wrapper_branches::block_tail_expr(&block.block).and_then(|expr| {
            wrapper_value_type_name_with_scope(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
        }),
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            let name = path.path.segments[0].ident.to_string();
            let kind = scope_type_name(&name)
                .as_deref()
                .and_then(type_names::ValueWrapperKind::from_type_name)?;
            let type_name = scope_item_type_name(&name)?;
            Some(WrapperValue { kind, type_name })
        }
        Expr::MethodCall(method_call) => wrapper_method_value_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Reference(reference) => wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            &reference.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            &paren.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => wrapper_value_type_name_with_scope(
            document,
            workspace_index,
            &group.expr,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => None,
    }
}

fn wrapper_method_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    let method = method_call.method.to_string();
    match (method.as_str(), method_call.args.len()) {
        (WRAPPER_MAP_METHOD, 1) => wrapper_map_value_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        (WRAPPER_AND_THEN_METHOD, 1) => wrapper_and_then_value_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => wrapper_preserving_value_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
    }
}

fn wrapper_constructor_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    call: &syn::ExprCall,
    expected_kind: Option<type_names::ValueWrapperKind>,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    let kind = type_names::wrapper_constructor_kind(call)?;
    if expected_kind.is_some_and(|expected| expected != kind) || call.args.len() != 1 {
        return None;
    }
    let type_lookup = |name: &str| scope_type_name(name);
    let context_lookup = |name: &str| context_type_name(name);
    let item_lookup = |name: &str| scope_item_type_name(name);
    let type_name = expression_type_name_with_item_scope(
        document,
        workspace_index,
        call.args.first()?,
        &type_lookup,
        &context_lookup,
        &item_lookup,
    )?;
    Some(WrapperValue { kind, type_name })
}

fn wrapper_preserving_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    let receiver = wrapper_value_type_name_with_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let method = method_call.method.to_string();
    let kind = iterables::wrapper_value_method_output_kind(
        receiver.kind,
        &method,
        method_call.args.len(),
    )?;
    Some(WrapperValue {
        kind,
        type_name: receiver.type_name,
    })
}

fn wrapper_map_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    let input = wrapper_value_type_name_with_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let closure = single_closure_arg(method_call)?;
    let type_name = closure_return_type_name(
        document,
        workspace_index,
        closure,
        &input.type_name,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    Some(WrapperValue {
        kind: input.kind,
        type_name,
    })
}

fn wrapper_and_then_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<WrapperValue> {
    let input = wrapper_value_type_name_with_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let closure = single_closure_arg(method_call)?;
    let type_name = closure_wrapped_return_type_name(
        document,
        workspace_index,
        closure,
        WrapperClosureInput {
            type_name: &input.type_name,
            expected_wrapper: input.kind,
        },
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    Some(WrapperValue {
        kind: input.kind,
        type_name,
    })
}

fn iterator_map_item_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    let input_type = expression_iterable_item_type_name_with_item_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;
    let closure = single_closure_arg(method_call)?;
    closure_return_type_name(
        document,
        workspace_index,
        closure,
        &input_type,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn closure_wrapped_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    closure: &ExprClosure,
    input: WrapperClosureInput<'_>,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    if let ReturnType::Type(_, ty) = &closure.output {
        return type_names::wrapped_value_type_name_for_kind(ty, Some(input.expected_wrapper));
    }
    let closure_arg = closure.inputs.iter().next()?;
    let closure_values =
        typed_pattern_bindings(document, workspace_index, closure_arg, input.type_name);
    let context_lookup = |name: &str| context_type_name(name);
    let item_lookup = |name: &str| scope_item_type_name(name);
    expression_wrapped_value_type_name(
        document,
        workspace_index,
        &closure.body,
        input.expected_wrapper,
        &|name| {
            closure_values
                .iter()
                .rev()
                .find(|value| value.name == name)
                .map(|value| value.type_name.clone())
                .or_else(|| scope_type_name(name))
        },
        &context_lookup,
        &item_lookup,
    )
}

fn single_closure_arg(method_call: &ExprMethodCall) -> Option<&ExprClosure> {
    let Expr::Closure(closure) = method_call.args.iter().next()? else {
        return None;
    };
    Some(closure)
}

fn closure_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    closure: &ExprClosure,
    input_type: &str,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    if let ReturnType::Type(_, ty) = &closure.output {
        return local_value_type_name_from_type(ty);
    }
    let input = closure.inputs.iter().next()?;
    let closure_values = typed_pattern_bindings(document, workspace_index, input, input_type);
    let context_lookup = |name: &str| context_type_name(name);
    let item_lookup = |name: &str| scope_item_type_name(name);
    expression_type_name_with_item_scope(
        document,
        workspace_index,
        &closure.body,
        &|name| {
            closure_values
                .iter()
                .rev()
                .find(|value| value.name == name)
                .map(|value| value.type_name.clone())
                .or_else(|| scope_type_name(name))
        },
        &context_lookup,
        &item_lookup,
    )
}

fn expression_wrapped_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    expected_wrapper: type_names::ValueWrapperKind,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    match expr {
        Expr::Call(call) => constructor_wrapped_value_type_name(
            document,
            workspace_index,
            call,
            expected_wrapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            wrapper_value_type_name_with_scope(
                document,
                workspace_index,
                expr,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            )
            .and_then(|value| (value.kind == expected_wrapper).then_some(value.type_name))
        }
        Expr::Block(block) => block.block.stmts.last().and_then(|stmt| match stmt {
            syn::Stmt::Expr(expr, None) => expression_wrapped_value_type_name(
                document,
                workspace_index,
                expr,
                expected_wrapper,
                scope_type_name,
                context_type_name,
                scope_item_type_name,
            ),
            _ => None,
        }),
        Expr::Reference(reference) => expression_wrapped_value_type_name(
            document,
            workspace_index,
            &reference.expr,
            expected_wrapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Paren(paren) => expression_wrapped_value_type_name(
            document,
            workspace_index,
            &paren.expr,
            expected_wrapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        Expr::Group(group) => expression_wrapped_value_type_name(
            document,
            workspace_index,
            &group.expr,
            expected_wrapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => None,
    }
}

fn constructor_wrapped_value_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    call: &syn::ExprCall,
    expected_wrapper: type_names::ValueWrapperKind,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    wrapper_constructor_value_type_name(
        document,
        workspace_index,
        call,
        Some(expected_wrapper),
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
    .map(|value| value.type_name)
}
