use syn::{GenericArgument, PathArguments, Type};

const OPTIONAL_ACCOUNT_FIELD_WRAPPER: &str = "Option";
const TRANSPARENT_ACCOUNT_FIELD_WRAPPERS: &[&str] = &["Box"];

pub(super) fn account_field_type(ty: &Type) -> (&Type, bool) {
    account_field_type_inner(ty, false)
}

fn account_field_type_inner(ty: &Type, is_optional: bool) -> (&Type, bool) {
    let Type::Path(type_path) = ty else {
        return (ty, is_optional);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return (ty, is_optional);
    };

    let wrapper = segment.ident.to_string();
    let is_optional_wrapper = wrapper == OPTIONAL_ACCOUNT_FIELD_WRAPPER;
    if !is_optional_wrapper && !TRANSPARENT_ACCOUNT_FIELD_WRAPPERS.contains(&wrapper.as_str()) {
        return (ty, is_optional);
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return (ty, is_optional);
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return (ty, is_optional);
    };

    account_field_type_inner(inner, is_optional || is_optional_wrapper)
}
