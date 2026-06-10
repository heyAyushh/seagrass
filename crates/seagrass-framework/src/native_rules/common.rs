use quote::ToTokens;

pub(super) fn unsigned_literal(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Int(lit) => lit.base10_parse().ok(),
            _ => None,
        },
        syn::Expr::Group(group) => unsigned_literal(&group.expr),
        syn::Expr::Paren(paren) => unsigned_literal(&paren.expr),
        _ => None,
    }
}

pub(super) fn ident_matches_any(ident: &syn::Ident, candidates: &[&str]) -> bool {
    candidates.iter().any(|candidate| ident == *candidate)
}

pub(super) fn member_is_named(member: &syn::Member, name: &str) -> bool {
    matches!(member, syn::Member::Named(ident) if ident == name)
}

pub(super) fn called_ident(func: &syn::Expr) -> Option<&syn::Ident> {
    let syn::Expr::Path(expr_path) = func else {
        return None;
    };
    expr_path.path.segments.last().map(|segment| &segment.ident)
}

pub(super) fn local_ident_name(local: &syn::Local) -> Option<String> {
    pat_ident_name(&local.pat)
}

pub(super) fn pat_ident_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        syn::Pat::Reference(reference) => pat_ident_name(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident_name(&pat_type.pat),
        _ => None,
    }
}

pub(super) fn compact_token_text(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}
