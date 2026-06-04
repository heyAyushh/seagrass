//! Helpers for reading a field/argument `syn::Type`: its last path segment
//! (`type_name`), the segment's source range, and its angle-bracketed generic
//! arguments (e.g. the `T` in `Account<'info, T>`).

use {
    super::NamedRange,
    crate::range::range_from_span,
    syn::{GenericArgument, PathArguments, Type},
    tower_lsp::lsp_types::Range,
};

pub(super) fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

pub(super) fn type_range(ty: &Type) -> Option<Range> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| range_from_span(segment.ident.span()))
}

pub(super) fn generic_type_ranges(ty: &Type) -> Vec<NamedRange> {
    let Type::Path(type_path) = ty else {
        return Vec::new();
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Vec::new();
    };
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Vec::new();
    };

    args.args
        .iter()
        .filter_map(|arg| match arg {
            GenericArgument::Type(Type::Path(type_path)) => {
                type_path.path.segments.last().map(|segment| NamedRange {
                    name: segment.ident.to_string(),
                    range: range_from_span(segment.ident.span()),
                })
            }
            _ => None,
        })
        .collect()
}
