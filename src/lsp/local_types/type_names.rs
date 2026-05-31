use {
    super::account_loader,
    syn::{GenericArgument, PathArguments, ReturnType, Type, TypePath},
};

const TRANSPARENT_LOCAL_TYPE_WRAPPERS: &[&str] = &["Box"];
const TRY_UNWRAP_RETURN_TYPE_WRAPPERS: &[&str] = &["Result"];
const ACCOUNT_DATA_TYPE_WRAPPERS: &[&str] = &[
    "Account",
    "InterfaceAccount",
    "LazyAccount",
    "AccountLoader",
];
const TRANSPARENT_ACCOUNT_FIELD_WRAPPERS: &[&str] = &["Box", "Option"];

pub(crate) fn context_type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(text).ok()?;
    context_type_name_from_type(&ty)
}

pub(crate) fn context_type_name_from_type(ty: &Type) -> Option<String> {
    context_type_name(ty)
}

pub(crate) fn account_data_type_name_from_text(type_text: &str) -> Option<String> {
    let ty = syn::parse_str::<syn::Type>(type_text).ok()?;
    account_loader::local_type_name_from_type(&ty).or_else(|| account_data_type_name(&ty))
}

pub(crate) fn account_data_type_name_from_parts(
    type_name: Option<&str>,
    generic_type_names: &[String],
) -> Option<String> {
    account_loader::local_type_name_from_parts(type_name, generic_type_names).or_else(|| {
        generic_type_names
            .last()
            .cloned()
            .or_else(|| type_name.map(str::to_string))
    })
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

pub(super) fn return_type_name(output: &ReturnType) -> Option<String> {
    let ReturnType::Type(_, ty) = output else {
        return None;
    };
    local_value_type_name(ty)
}

pub(super) fn return_type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<Type>(text).ok()?;
    local_value_type_name(&ty)
}

pub(super) fn try_return_type_name(output: &ReturnType) -> Option<String> {
    let ReturnType::Type(_, ty) = output else {
        return None;
    };
    try_unwrapped_return_type_name(ty)
}

pub(super) fn try_return_type_name_from_text(text: &str) -> Option<String> {
    let ty = syn::parse_str::<Type>(text).ok()?;
    try_unwrapped_return_type_name(&ty)
}

fn try_unwrapped_return_type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = transparent_type_path(ty) else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let wrapper = segment.ident.to_string();
    if !TRY_UNWRAP_RETURN_TYPE_WRAPPERS.contains(&wrapper.as_str()) {
        return None;
    }
    first_type_argument_type(&segment.arguments).and_then(local_value_type_name)
}

fn local_value_type_name(ty: &Type) -> Option<String> {
    account_loader::local_type_name_from_type(ty).or_else(|| shallow_type_name(ty))
}
