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

pub(crate) fn local_ident_name(local: &syn::Local) -> Option<String> {
    pat_ident_name(&local.pat)
}

pub(crate) fn pat_ident_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.to_string()),
        syn::Pat::Reference(reference) => pat_ident_name(&reference.pat),
        syn::Pat::Type(pat_type) => pat_ident_name(&pat_type.pat),
        _ => None,
    }
}

pub(crate) fn compact_token_text(tokens: &impl ToTokens) -> String {
    tokens
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect()
}

pub(super) const ACCOUNT_ITERATOR_METHODS: &[&str] = &["iter", "iter_mut"];
pub(super) const TRANSPARENT_ACCOUNT_ACCESS_METHODS: &[&str] = &[
    "ok_or",
    "ok_or_else",
    "unwrap",
    "expect",
    "as_ref",
    "as_mut",
];
pub(crate) const INITIAL_ITERATOR_ACCOUNT_INDEX: usize = 0;

#[derive(Clone, Debug)]
pub(crate) struct AccountIteratorOrigin {
    pub next_index: usize,
}

pub(super) fn account_index_from_expr(expr: &syn::Expr) -> Option<usize> {
    match expr {
        syn::Expr::Index(index) => unsigned_literal(&index.index),
        syn::Expr::MethodCall(method_call) if method_call.method == "get" => {
            method_call.args.first().and_then(unsigned_literal)
        }
        syn::Expr::MethodCall(method_call)
            if TRANSPARENT_ACCOUNT_ACCESS_METHODS
                .contains(&method_call.method.to_string().as_str()) =>
        {
            account_index_from_expr(&method_call.receiver)
        }
        syn::Expr::Field(field) => account_index_from_expr(&field.base),
        syn::Expr::MethodCall(method_call) => account_index_from_expr(&method_call.receiver),
        syn::Expr::Unary(unary) => account_index_from_expr(&unary.expr),
        syn::Expr::Reference(reference) => account_index_from_expr(&reference.expr),
        syn::Expr::Paren(paren) => account_index_from_expr(&paren.expr),
        syn::Expr::Group(group) => account_index_from_expr(&group.expr),
        syn::Expr::Try(expr_try) => account_index_from_expr(&expr_try.expr),
        _ => None,
    }
}

pub(super) fn account_alias_from_expr(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(expr_path) => expr_path.path.get_ident().map(ToString::to_string),
        syn::Expr::Field(field) => account_alias_from_expr(&field.base),
        syn::Expr::MethodCall(method_call) => account_alias_from_expr(&method_call.receiver),
        syn::Expr::Unary(unary) => account_alias_from_expr(&unary.expr),
        syn::Expr::Reference(reference) => account_alias_from_expr(&reference.expr),
        syn::Expr::Paren(paren) => account_alias_from_expr(&paren.expr),
        syn::Expr::Group(group) => account_alias_from_expr(&group.expr),
        _ => None,
    }
}

pub(crate) fn account_iterator_collection(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method_call)
            if ACCOUNT_ITERATOR_METHODS.contains(&method_call.method.to_string().as_str()) =>
        {
            account_alias_from_expr(&method_call.receiver)
        }
        syn::Expr::Reference(reference) => account_iterator_collection(&reference.expr),
        syn::Expr::Paren(paren) => account_iterator_collection(&paren.expr),
        syn::Expr::Group(group) => account_iterator_collection(&group.expr),
        _ => None,
    }
}

pub(crate) fn next_account_info_iterator_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Call(call) if called_ident(&call.func)? == "next_account_info" => {
            call.args.first().and_then(account_iterator_name)
        }
        syn::Expr::Try(expr_try) => next_account_info_iterator_name(&expr_try.expr),
        syn::Expr::Reference(reference) => next_account_info_iterator_name(&reference.expr),
        syn::Expr::Paren(paren) => next_account_info_iterator_name(&paren.expr),
        syn::Expr::Group(group) => next_account_info_iterator_name(&group.expr),
        _ => None,
    }
}

pub(super) fn account_iterator_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(_) => account_alias_from_expr(expr),
        syn::Expr::Reference(reference) => account_iterator_name(&reference.expr),
        syn::Expr::Paren(paren) => account_iterator_name(&paren.expr),
        syn::Expr::Group(group) => account_iterator_name(&group.expr),
        _ => None,
    }
}
