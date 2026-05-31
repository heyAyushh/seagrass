use {
    super::{type_names::ValueWrapperKind, TypedLocalValue},
    crate::{account_members, document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{Member, Pat},
};

const OPTION_PATTERN_CONSTRUCTOR: &str = "Some";
const OPTION_PATTERN_TYPE: &str = "Option";
const RESULT_OK_PATTERN_CONSTRUCTOR: &str = "Ok";
const RESULT_PATTERN_TYPE: &str = "Result";

pub(crate) fn typed_pattern_bindings(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &Pat,
    type_name: &str,
) -> Vec<TypedLocalValue> {
    typed_pattern_bindings_with_wrapped_item(document, workspace_index, pat, type_name, None)
}

pub(crate) fn typed_pattern_bindings_with_wrapped_item(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &Pat,
    type_name: &str,
    wrapped_item_type_name: Option<&str>,
) -> Vec<TypedLocalValue> {
    let mut values = Vec::new();
    collect_typed_pattern_bindings(
        document,
        workspace_index,
        pat,
        type_name,
        wrapped_item_type_name,
        &mut values,
    );
    values
}

pub(crate) fn wrapper_type_name(pat: &Pat) -> Option<&'static str> {
    let Pat::TupleStruct(tuple) = pat else {
        return None;
    };
    wrapper_pattern_type(tuple).map(|kind| match kind {
        ValueWrapperKind::Option => OPTION_PATTERN_TYPE,
        ValueWrapperKind::Result => RESULT_PATTERN_TYPE,
    })
}

fn collect_typed_pattern_bindings(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &Pat,
    type_name: &str,
    wrapped_item_type_name: Option<&str>,
    values: &mut Vec<TypedLocalValue>,
) {
    match pat {
        Pat::Ident(ident) => values.push(TypedLocalValue {
            name: ident.ident.to_string(),
            type_name: type_name.to_string(),
        }),
        Pat::Reference(reference) => {
            collect_typed_pattern_bindings(
                document,
                workspace_index,
                &reference.pat,
                type_name,
                wrapped_item_type_name,
                values,
            );
        }
        Pat::Paren(paren) => {
            collect_typed_pattern_bindings(
                document,
                workspace_index,
                &paren.pat,
                type_name,
                wrapped_item_type_name,
                values,
            );
        }
        Pat::Type(typed) => {
            let type_name = super::local_value_type_name_from_type(&typed.ty)
                .unwrap_or_else(|| type_name.to_string());
            let wrapped_item_type_name = super::iterable_item_type_name_from_type(&typed.ty);
            collect_typed_pattern_bindings(
                document,
                workspace_index,
                &typed.pat,
                &type_name,
                wrapped_item_type_name.as_deref(),
                values,
            );
        }
        Pat::TupleStruct(tuple) if wrapper_tuple_struct_pattern_matches_type(tuple, type_name) => {
            let Some(inner_type) = wrapped_item_type_name else {
                return;
            };
            if let Some(inner) = tuple.elems.first() {
                collect_typed_pattern_bindings(
                    document,
                    workspace_index,
                    inner,
                    inner_type,
                    None,
                    values,
                );
            }
        }
        Pat::TupleStruct(tuple) if pre_unwrapped_option_pattern(tuple, type_name) => {
            if let Some(inner) = tuple.elems.first() {
                collect_typed_pattern_bindings(
                    document,
                    workspace_index,
                    inner,
                    type_name,
                    None,
                    values,
                );
            }
        }
        Pat::Struct(pat_struct) if struct_pattern_matches_type(pat_struct, type_name) => {
            collect_struct_pattern_bindings(
                document,
                workspace_index,
                pat_struct,
                type_name,
                values,
            );
        }
        Pat::Or(or) => {
            for case in &or.cases {
                collect_typed_pattern_bindings(
                    document,
                    workspace_index,
                    case,
                    type_name,
                    wrapped_item_type_name,
                    values,
                );
            }
        }
        _ => {}
    }
}

fn collect_struct_pattern_bindings(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat_struct: &syn::PatStruct,
    type_name: &str,
    values: &mut Vec<TypedLocalValue>,
) {
    for field in &pat_struct.fields {
        let Member::Named(member) = &field.member else {
            continue;
        };
        let Some(field_type) = account_members::resolved_struct_member_type_name(
            document,
            workspace_index,
            type_name,
            &member.to_string(),
        ) else {
            continue;
        };
        collect_typed_pattern_bindings(
            document,
            workspace_index,
            &field.pat,
            &field_type,
            None,
            values,
        );
    }
}

fn struct_pattern_matches_type(pat_struct: &syn::PatStruct, type_name: &str) -> bool {
    pat_struct
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == type_name)
}

fn wrapper_tuple_struct_pattern_matches_type(tuple: &syn::PatTupleStruct, type_name: &str) -> bool {
    tuple.elems.len() == 1
        && wrapper_pattern_type(tuple) == ValueWrapperKind::from_type_name(type_name)
}

fn pre_unwrapped_option_pattern(tuple: &syn::PatTupleStruct, type_name: &str) -> bool {
    tuple.elems.len() == 1
        && wrapper_pattern_type(tuple) == Some(ValueWrapperKind::Option)
        && ValueWrapperKind::from_type_name(type_name).is_none()
}

fn wrapper_pattern_type(tuple: &syn::PatTupleStruct) -> Option<ValueWrapperKind> {
    let constructor = tuple.path.segments.last()?.ident.to_string();
    match constructor.as_str() {
        OPTION_PATTERN_CONSTRUCTOR => Some(ValueWrapperKind::Option),
        RESULT_OK_PATTERN_CONSTRUCTOR => Some(ValueWrapperKind::Result),
        _ => None,
    }
}
