use {super::TypedLocalValue, syn::Pat};

const TRANSPARENT_PATTERN_WRAPPERS: &[&str] = &["Some", "Ok"];

pub(crate) fn typed_pattern_bindings(pat: &Pat, type_name: &str) -> Vec<TypedLocalValue> {
    let mut values = Vec::new();
    collect_typed_pattern_bindings(pat, type_name, &mut values);
    values
}

fn collect_typed_pattern_bindings(pat: &Pat, type_name: &str, values: &mut Vec<TypedLocalValue>) {
    match pat {
        Pat::Ident(ident) => values.push(TypedLocalValue {
            name: ident.ident.to_string(),
            type_name: type_name.to_string(),
        }),
        Pat::Reference(reference) => {
            collect_typed_pattern_bindings(&reference.pat, type_name, values);
        }
        Pat::Paren(paren) => collect_typed_pattern_bindings(&paren.pat, type_name, values),
        Pat::Type(typed) => {
            let type_name = super::local_value_type_name_from_type(&typed.ty)
                .unwrap_or_else(|| type_name.to_string());
            collect_typed_pattern_bindings(&typed.pat, &type_name, values);
        }
        Pat::TupleStruct(tuple) if transparent_tuple_struct_pattern(tuple) => {
            if let Some(inner) = tuple.elems.first() {
                collect_typed_pattern_bindings(inner, type_name, values);
            }
        }
        Pat::Or(or) => {
            for case in &or.cases {
                collect_typed_pattern_bindings(case, type_name, values);
            }
        }
        _ => {}
    }
}

fn transparent_tuple_struct_pattern(tuple: &syn::PatTupleStruct) -> bool {
    tuple.elems.len() == 1
        && tuple.path.segments.last().is_some_and(|segment| {
            TRANSPARENT_PATTERN_WRAPPERS.contains(&segment.ident.to_string().as_str())
        })
}
