use {
    super::type_names::ValueWrapperKind,
    syn::{GenericArgument, Pat, PathArguments, Type},
};

const ITERABLE_VALUE_TYPES: &[&str] = &[
    "BTreeSet",
    "BinaryHeap",
    "HashSet",
    "LinkedList",
    "Option",
    "Result",
    "Vec",
    "VecDeque",
];
const TRANSPARENT_ITERABLE_TYPES: &[&str] = &["Box"];
// These std Iterator adapters preserve the item type, so the shallow resolver
// can carry a known struct type through them without invoking rust-analyzer.
const ITERATOR_METHODS: &[&str] = &[
    "by_ref",
    "cloned",
    "copied",
    "fuse",
    "into_iter",
    "iter",
    "iter_mut",
    "peekable",
    "rev",
];
const ITEM_PRESERVING_ITERATOR_METHODS_WITH_ONE_ARG: &[&str] = &[
    "filter",
    "inspect",
    "skip",
    "skip_while",
    "step_by",
    "take",
    "take_while",
];
const ITEM_TRANSFORMING_ITERATOR_METHODS_WITH_ONE_ARG: &[&str] = &["map"];
const ITEM_CLOSURE_ITERATOR_METHODS_WITH_ONE_ARG: &[&str] = &[
    "all",
    "any",
    "filter",
    "find",
    "inspect",
    "map",
    "position",
    "skip_while",
    "take_while",
];
const WRAPPER_CLOSURE_VALUE_METHODS_WITH_ONE_ARG: &[&str] = &[
    "and_then",
    "filter",
    "inspect",
    "is_none_or",
    "is_some_and",
    "map",
];
const WRAPPER_VALUE_PRESERVING_METHODS: &[&str] = &["as_mut", "as_ref", "inspect"];
const OPTION_VALUE_PRESERVING_METHODS_WITH_ONE_ARG: &[&str] = &["filter", "or", "or_else", "xor"];
const OPTION_TO_RESULT_METHODS_WITH_ONE_ARG: &[&str] = &["ok_or", "ok_or_else"];
const RESULT_VALUE_PRESERVING_METHODS_WITH_ONE_ARG: &[&str] =
    &["inspect_err", "map_err", "or", "or_else"];
const RESULT_OK_TO_OPTION_METHODS: &[&str] = &["ok"];
const OPTION_VALUE_METHODS: &[&str] = &["unwrap", "unwrap_or_default"];
const OPTION_VALUE_METHODS_WITH_ONE_ARG: &[&str] = &["expect", "unwrap_or", "unwrap_or_else"];
const COLLECTION_OPTION_ITEM_METHODS: &[&str] = &["first", "last", "pop"];
const COLLECTION_OPTION_ITEM_METHODS_WITH_ONE_ARG: &[&str] = &["get", "get_mut"];
const ITERATOR_OPTION_ITEM_METHODS: &[&str] = &["last", "next"];
const ITERATOR_OPTION_ITEM_METHODS_WITH_ONE_ARG: &[&str] = &["find", "nth"];

pub(super) fn item_type_name_from_type(ty: &Type) -> Option<String> {
    match transparent_type(ty) {
        Type::Array(array) => super::local_value_type_name_from_type(&array.elem),
        Type::Slice(slice) => super::local_value_type_name_from_type(&slice.elem),
        Type::Path(type_path) => {
            let segment = type_path.path.segments.last()?;
            let type_name = segment.ident.to_string();
            if TRANSPARENT_ITERABLE_TYPES.contains(&type_name.as_str()) {
                return first_type_argument_type(&segment.arguments)
                    .and_then(item_type_name_from_type);
            }
            if ITERABLE_VALUE_TYPES.contains(&type_name.as_str()) {
                return first_type_argument_type(&segment.arguments)
                    .and_then(super::local_value_type_name_from_type);
            }
            None
        }
        _ => None,
    }
}

pub(super) fn explicit_pattern_item_type_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Type(typed) => item_type_name_from_type(&typed.ty),
        Pat::Reference(reference) => explicit_pattern_item_type_name(&reference.pat),
        Pat::Paren(paren) => explicit_pattern_item_type_name(&paren.pat),
        _ => None,
    }
}

pub(super) fn expression_item_type_name(
    expr: &syn::Expr,
    scope_item_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    match expr {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => {
            scope_item_type_name(&path.path.segments[0].ident.to_string())
        }
        syn::Expr::MethodCall(method_call)
            if item_preserving_iterator_method_matches(
                &method_call.method.to_string(),
                method_call.args.len(),
            ) =>
        {
            expression_item_type_name(&method_call.receiver, scope_item_type_name)
        }
        syn::Expr::Reference(reference) => {
            expression_item_type_name(&reference.expr, scope_item_type_name)
        }
        syn::Expr::Paren(paren) => expression_item_type_name(&paren.expr, scope_item_type_name),
        syn::Expr::Group(group) => expression_item_type_name(&group.expr, scope_item_type_name),
        _ => None,
    }
}

pub(super) fn option_value_method_matches(method: &str, arg_count: usize) -> bool {
    (arg_count == 0 && OPTION_VALUE_METHODS.contains(&method))
        || (arg_count == 1 && OPTION_VALUE_METHODS_WITH_ONE_ARG.contains(&method))
}

pub(super) fn collection_option_item_method_matches(method: &str, arg_count: usize) -> bool {
    (arg_count == 0 && COLLECTION_OPTION_ITEM_METHODS.contains(&method))
        || (arg_count == 1 && COLLECTION_OPTION_ITEM_METHODS_WITH_ONE_ARG.contains(&method))
}

pub(super) fn item_preserving_iterator_method_matches(method: &str, arg_count: usize) -> bool {
    (arg_count == 0 && ITERATOR_METHODS.contains(&method))
        || (arg_count == 1 && ITEM_PRESERVING_ITERATOR_METHODS_WITH_ONE_ARG.contains(&method))
}

pub(super) fn item_transforming_iterator_method_matches(method: &str, arg_count: usize) -> bool {
    arg_count == 1 && ITEM_TRANSFORMING_ITERATOR_METHODS_WITH_ONE_ARG.contains(&method)
}

pub(super) fn item_closure_iterator_method_matches(method: &str, arg_count: usize) -> bool {
    arg_count == 1 && ITEM_CLOSURE_ITERATOR_METHODS_WITH_ONE_ARG.contains(&method)
}

pub(super) fn wrapper_closure_value_method_matches(method: &str, arg_count: usize) -> bool {
    arg_count == 1 && WRAPPER_CLOSURE_VALUE_METHODS_WITH_ONE_ARG.contains(&method)
}

pub(super) fn wrapper_value_method_output_kind(
    kind: ValueWrapperKind,
    method: &str,
    arg_count: usize,
) -> Option<ValueWrapperKind> {
    if arg_count == 0 && WRAPPER_VALUE_PRESERVING_METHODS.contains(&method) {
        return Some(kind);
    }

    match kind {
        ValueWrapperKind::Option => option_value_method_output_kind(method, arg_count),
        ValueWrapperKind::Result => result_value_method_output_kind(method, arg_count),
    }
}

fn option_value_method_output_kind(method: &str, arg_count: usize) -> Option<ValueWrapperKind> {
    if arg_count == 1 && OPTION_VALUE_PRESERVING_METHODS_WITH_ONE_ARG.contains(&method) {
        return Some(ValueWrapperKind::Option);
    }
    if arg_count == 1 && OPTION_TO_RESULT_METHODS_WITH_ONE_ARG.contains(&method) {
        return Some(ValueWrapperKind::Result);
    }
    None
}

fn result_value_method_output_kind(method: &str, arg_count: usize) -> Option<ValueWrapperKind> {
    if arg_count == 0 && RESULT_OK_TO_OPTION_METHODS.contains(&method) {
        return Some(ValueWrapperKind::Option);
    }
    if arg_count == 1 && RESULT_VALUE_PRESERVING_METHODS_WITH_ONE_ARG.contains(&method) {
        return Some(ValueWrapperKind::Result);
    }
    None
}

pub(super) fn iterator_option_item_method_matches(method: &str, arg_count: usize) -> bool {
    (arg_count == 0 && ITERATOR_OPTION_ITEM_METHODS.contains(&method))
        || (arg_count == 1 && ITERATOR_OPTION_ITEM_METHODS_WITH_ONE_ARG.contains(&method))
}

fn transparent_type(ty: &Type) -> &Type {
    match ty {
        Type::Reference(reference) => transparent_type(&reference.elem),
        Type::Ptr(pointer) => transparent_type(&pointer.elem),
        Type::Paren(paren) => transparent_type(&paren.elem),
        Type::Group(group) => transparent_type(&group.elem),
        _ => ty,
    }
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
