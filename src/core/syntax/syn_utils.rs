pub(crate) fn expr_path_last_ident(expr: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = expr else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

pub(crate) fn expr_path_ends_with(expr: &syn::Expr, expected: &[&str]) -> bool {
    match expr {
        syn::Expr::Path(expr_path) => path_ends_with(&expr_path.path, expected),
        syn::Expr::Call(expr_call) => expr_path_ends_with(&expr_call.func, expected),
        syn::Expr::Group(expr_group) => expr_path_ends_with(&expr_group.expr, expected),
        syn::Expr::Paren(expr_paren) => expr_path_ends_with(&expr_paren.expr, expected),
        syn::Expr::Reference(expr_reference) => expr_path_ends_with(&expr_reference.expr, expected),
        _ => false,
    }
}

fn path_ends_with(path: &syn::Path, expected: &[&str]) -> bool {
    let segment_count = path.segments.len();
    segment_count >= expected.len()
        && path
            .segments
            .iter()
            .skip(segment_count - expected.len())
            .map(|segment| segment.ident.to_string())
            .eq(expected.iter().copied())
}

pub(crate) fn member_name(member: &syn::Member) -> Option<String> {
    match member {
        syn::Member::Named(ident) => Some(ident.to_string()),
        syn::Member::Unnamed(_) => None,
    }
}

pub(crate) fn member_is_named(member: &syn::Member, expected: &str) -> bool {
    matches!(member, syn::Member::Named(ident) if ident == expected)
}
