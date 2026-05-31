use {
    crate::account_members,
    syn::{ExprMethodCall, GenericArgument, PathArguments, Type, TypePath},
};

const TRANSPARENT_ACCOUNT_LOADER_WRAPPERS: &[&str] = &["Box", "Option"];

pub(super) fn local_type_name_from_parts(
    type_name: Option<&str>,
    generic_type_names: &[String],
) -> Option<String> {
    account_members::account_loader_type_name_from_parts(type_name, generic_type_names)
}

pub(super) fn local_type_name_from_type(ty: &Type) -> Option<String> {
    let Type::Path(TypePath { path, .. }) = transparent_type_path(ty) else {
        return None;
    };
    let segment = path.segments.last()?;
    let wrapper = segment.ident.to_string();
    if TRANSPARENT_ACCOUNT_LOADER_WRAPPERS.contains(&wrapper.as_str()) {
        return first_type_argument_type(&segment.arguments).and_then(local_type_name_from_type);
    }
    if wrapper != account_members::ACCOUNT_LOADER_TYPE {
        return None;
    }
    Some(last_type_argument_name(&segment.arguments).map_or_else(
        || account_members::ACCOUNT_LOADER_TYPE.to_string(),
        |inner| account_members::account_loader_type_name(&inner),
    ))
}

pub(super) fn loaded_method_return_type(
    method_call: &ExprMethodCall,
    receiver_type: &str,
) -> Option<String> {
    if !account_members::is_account_loader_loaded_method(&method_call.method.to_string())
        || !method_call.args.is_empty()
    {
        return None;
    }
    account_members::account_loader_inner_type(receiver_type).map(str::to_string)
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

fn first_type_argument_type(arguments: &PathArguments) -> Option<&Type> {
    let PathArguments::AngleBracketed(args) = arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(ty) => Some(ty),
        _ => None,
    })
}

fn last_type_argument_name(arguments: &PathArguments) -> Option<String> {
    let PathArguments::AngleBracketed(args) = arguments else {
        return None;
    };
    args.args.iter().rev().find_map(|arg| match arg {
        GenericArgument::Type(ty) => type_path_name(ty),
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
