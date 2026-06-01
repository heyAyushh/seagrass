use syn::parse::Parser;

// Only assertion-style macros here: parsing every macro would false-positive on
// format-style inputs such as `msg!("value={value}")`.
const RUNTIME_ASSERTION_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "require",
    "require_eq",
    "require_gt",
    "require_gte",
    "require_keys_eq",
    "require_keys_neq",
    "require_neq",
];

const DEBUG_ASSERTION_MACROS: &[&str] = &["debug_assert", "debug_assert_eq", "debug_assert_ne"];

pub(super) fn assertion_macro_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    if !is_expression_assertion_macro(node) {
        return Vec::new();
    }
    parse_expression_arguments(node)
}

pub(super) fn runtime_assertion_macro_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    if !is_runtime_assertion_macro(node) {
        return Vec::new();
    }
    parse_expression_arguments(node)
}

fn parse_expression_arguments(node: &syn::Macro) -> Vec<syn::Expr> {
    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
        .parse2(node.tokens.clone())
        .map(|expressions| expressions.into_iter().collect())
        .unwrap_or_default()
}

fn is_expression_assertion_macro(node: &syn::Macro) -> bool {
    node.path.segments.last().is_some_and(|segment| {
        let macro_name = segment.ident.to_string();
        RUNTIME_ASSERTION_MACROS.contains(&macro_name.as_str())
            || DEBUG_ASSERTION_MACROS.contains(&macro_name.as_str())
    })
}

fn is_runtime_assertion_macro(node: &syn::Macro) -> bool {
    node.path.segments.last().is_some_and(|segment| {
        RUNTIME_ASSERTION_MACROS.contains(&segment.ident.to_string().as_str())
    })
}
