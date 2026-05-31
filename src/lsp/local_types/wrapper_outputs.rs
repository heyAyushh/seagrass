use {
    super::{
        expression_optional_item_type_name_with_item_scope, expression_type_name_with_item_scope,
        local_value_type_name_from_type, type_names, typed_pattern_bindings, TypedLocalValue,
    },
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{Expr, ExprClosure, ExprMethodCall, ReturnType},
};

type TypeLookup<'a> = dyn Fn(&str) -> Option<String> + 'a;

const WRAPPER_MAP_OR_METHOD: &str = "map_or";
const WRAPPER_MAP_OR_ELSE_METHOD: &str = "map_or_else";
const WRAPPER_OUTPUT_METHOD_ARG_COUNT: usize = 2;

pub(super) fn method_output_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    if !is_wrapper_output_method(method_call) {
        return None;
    }
    receiver_wrapper_kind(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )?;

    match method_call.method.to_string().as_str() {
        WRAPPER_MAP_OR_METHOD => map_or_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        WRAPPER_MAP_OR_ELSE_METHOD => map_or_else_type_name(
            document,
            workspace_index,
            method_call,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        _ => None,
    }
}

fn is_wrapper_output_method(method_call: &ExprMethodCall) -> bool {
    method_call.args.len() == WRAPPER_OUTPUT_METHOD_ARG_COUNT
        && matches!(
            method_call.method.to_string().as_str(),
            WRAPPER_MAP_OR_METHOD | WRAPPER_MAP_OR_ELSE_METHOD
        )
}

fn map_or_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    let mut args = method_call.args.iter();
    let default = args.next()?;
    let mapper = closure_arg(args.next()?)?;
    let type_lookup = |name: &str| scope_type_name(name);
    let context_lookup = |name: &str| context_type_name(name);
    let item_lookup = |name: &str| scope_item_type_name(name);
    same_output_type_name([
        expression_type_name_with_item_scope(
            document,
            workspace_index,
            default,
            &type_lookup,
            &context_lookup,
            &item_lookup,
        ),
        mapper_output_type_name(
            document,
            workspace_index,
            &method_call.receiver,
            mapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
    ])
}

fn map_or_else_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    let mut args = method_call.args.iter();
    let default = closure_arg(args.next()?)?;
    let mapper = closure_arg(args.next()?)?;
    same_output_type_name([
        closure_output_type_name(
            document,
            workspace_index,
            default,
            None,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
        mapper_output_type_name(
            document,
            workspace_index,
            &method_call.receiver,
            mapper,
            scope_type_name,
            context_type_name,
            scope_item_type_name,
        ),
    ])
}

fn mapper_output_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver: &Expr,
    mapper: &ExprClosure,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    let input_type = expression_optional_item_type_name_with_item_scope(
        document,
        workspace_index,
        receiver,
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    );
    closure_output_type_name(
        document,
        workspace_index,
        mapper,
        input_type.as_deref(),
        scope_type_name,
        context_type_name,
        scope_item_type_name,
    )
}

fn closure_output_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    closure: &ExprClosure,
    input_type: Option<&str>,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<String> {
    if let ReturnType::Type(_, ty) = &closure.output {
        return local_value_type_name_from_type(ty);
    }
    let closure_values = closure_input_values(document, workspace_index, closure, input_type);
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
        &|name| context_type_name(name),
        &|name| scope_item_type_name(name),
    )
}

fn closure_input_values(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    closure: &ExprClosure,
    input_type: Option<&str>,
) -> Vec<TypedLocalValue> {
    let Some(input_type) = input_type else {
        return Vec::new();
    };
    let Some(input) = closure.inputs.iter().next() else {
        return Vec::new();
    };
    typed_pattern_bindings(document, workspace_index, input, input_type)
}

fn closure_arg(expr: &Expr) -> Option<&ExprClosure> {
    let Expr::Closure(closure) = expr else {
        return None;
    };
    Some(closure)
}

fn receiver_wrapper_kind(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver: &Expr,
    scope_type_name: &TypeLookup<'_>,
    context_type_name: &TypeLookup<'_>,
    scope_item_type_name: &TypeLookup<'_>,
) -> Option<type_names::ValueWrapperKind> {
    let type_lookup = |name: &str| scope_type_name(name);
    let context_lookup = |name: &str| context_type_name(name);
    let item_lookup = |name: &str| scope_item_type_name(name);
    let type_name = expression_type_name_with_item_scope(
        document,
        workspace_index,
        receiver,
        &type_lookup,
        &context_lookup,
        &item_lookup,
    )?;
    type_names::ValueWrapperKind::from_type_name(&type_name)
}

fn same_output_type_name(types: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    let mut resolved = None;
    for type_name in types.into_iter().flatten() {
        if resolved
            .as_ref()
            .is_some_and(|resolved| resolved != &type_name)
        {
            return None;
        }
        resolved = Some(type_name);
    }
    resolved
}
