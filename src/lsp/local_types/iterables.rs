use syn::{GenericArgument, Pat, PathArguments, Type};

const ITERABLE_VALUE_TYPES: &[&str] = &[
    "BTreeSet",
    "BinaryHeap",
    "HashSet",
    "LinkedList",
    "Option",
    "Vec",
    "VecDeque",
];
const TRANSPARENT_ITERABLE_TYPES: &[&str] = &["Box"];
const ITERATOR_METHODS: &[&str] = &["into_iter", "iter", "iter_mut"];

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
            if ITERATOR_METHODS.contains(&method_call.method.to_string().as_str())
                && method_call.args.is_empty() =>
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
