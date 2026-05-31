use {
    super::TypedLocalValue,
    crate::{account_members, document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{Member, Pat},
};

const TRANSPARENT_PATTERN_WRAPPERS: &[&str] = &["Some", "Ok"];

pub(crate) fn typed_pattern_bindings(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &Pat,
    type_name: &str,
) -> Vec<TypedLocalValue> {
    let mut values = Vec::new();
    collect_typed_pattern_bindings(document, workspace_index, pat, type_name, &mut values);
    values
}

fn collect_typed_pattern_bindings(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    pat: &Pat,
    type_name: &str,
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
                values,
            );
        }
        Pat::Paren(paren) => {
            collect_typed_pattern_bindings(
                document,
                workspace_index,
                &paren.pat,
                type_name,
                values,
            );
        }
        Pat::Type(typed) => {
            let type_name = super::local_value_type_name_from_type(&typed.ty)
                .unwrap_or_else(|| type_name.to_string());
            collect_typed_pattern_bindings(
                document,
                workspace_index,
                &typed.pat,
                &type_name,
                values,
            );
        }
        Pat::TupleStruct(tuple) if transparent_tuple_struct_pattern(tuple) => {
            if let Some(inner) = tuple.elems.first() {
                collect_typed_pattern_bindings(document, workspace_index, inner, type_name, values);
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
                collect_typed_pattern_bindings(document, workspace_index, case, type_name, values);
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
        collect_typed_pattern_bindings(document, workspace_index, &field.pat, &field_type, values);
    }
}

fn struct_pattern_matches_type(pat_struct: &syn::PatStruct, type_name: &str) -> bool {
    pat_struct
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == type_name)
}

fn transparent_tuple_struct_pattern(tuple: &syn::PatTupleStruct) -> bool {
    tuple.elems.len() == 1
        && tuple.path.segments.last().is_some_and(|segment| {
            TRANSPARENT_PATTERN_WRAPPERS.contains(&segment.ident.to_string().as_str())
        })
}
