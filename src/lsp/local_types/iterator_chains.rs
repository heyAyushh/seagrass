use {
    super::{
        expression_type_name_with_item_scope, iterables, local_value_type_name_from_type,
        typed_pattern_bindings,
    },
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{Expr, ExprClosure, ExprMethodCall, ReturnType},
};

type TypeLookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

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

fn optional_item_type_name_with_scope(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    match expr {
        Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            scope_item_type_name(&path.path.segments[0].ident.to_string())
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
    if !iterables::item_closure_iterator_method_matches(
        &method_call.method.to_string(),
        method_call.args.len(),
    ) {
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
