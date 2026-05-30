use syn::Pat;

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
