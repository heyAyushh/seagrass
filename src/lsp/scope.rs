use syn::{GenericArgument, Pat, PathArguments, Type};

pub(crate) fn collect_pattern_bindings(pat: &Pat, names: &mut Vec<String>) {
    match pat {
        Pat::Ident(ident) => names.push(ident.ident.to_string()),
        Pat::Reference(reference) => collect_pattern_bindings(&reference.pat, names),
        Pat::Slice(slice) => {
            for element in &slice.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::Struct(strukt) => {
            for field in &strukt.fields {
                collect_pattern_bindings(&field.pat, names);
            }
        }
        Pat::Tuple(tuple) => {
            for element in &tuple.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::TupleStruct(tuple) => {
            for element in &tuple.elems {
                collect_pattern_bindings(element, names);
            }
        }
        Pat::Type(typed) => collect_pattern_bindings(&typed.pat, names),
        Pat::Or(or) => {
            for case in &or.cases {
                collect_pattern_bindings(case, names);
            }
        }
        Pat::Paren(paren) => collect_pattern_bindings(&paren.pat, names),
        _ => {}
    }
}

pub(crate) fn pattern_binding_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(ident) => Some(ident.ident.to_string()),
        Pat::Reference(reference) => pattern_binding_name(&reference.pat),
        Pat::Type(typed) => pattern_binding_name(&typed.pat),
        Pat::Paren(paren) => pattern_binding_name(&paren.pat),
        _ => None,
    }
}

pub(crate) fn item_fn_has_anchor_context_arg(item_fn: &syn::ItemFn) -> bool {
    item_fn.sig.inputs.iter().any(|input| match input {
        syn::FnArg::Typed(pat_type) => type_has_anchor_context_arg(pat_type.ty.as_ref()),
        syn::FnArg::Receiver(_) => false,
    })
}

pub(crate) fn type_has_anchor_context_arg(ty: &Type) -> bool {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(type_path) = ty else {
        return false;
    };
    type_path.path.segments.iter().any(|segment| {
        (segment.ident == "Context" || segment.ident.to_string().ends_with("Context"))
            && matches!(&segment.arguments, PathArguments::AngleBracketed(_))
            && context_type_has_account_argument(&segment.arguments)
    })
}

pub(crate) fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn context_type_has_account_argument(arguments: &PathArguments) -> bool {
    let PathArguments::AngleBracketed(args) = arguments else {
        return false;
    };
    args.args
        .iter()
        .any(|arg| matches!(arg, GenericArgument::Type(Type::Path(_))))
}
