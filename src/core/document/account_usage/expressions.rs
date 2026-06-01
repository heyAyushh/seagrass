use {
    super::{
        aliases::{account_field_alias_segment_for_expr, AccountFieldAlias},
        AccountUsage, NamedRange,
    },
    crate::range::range_from_span,
};

fn is_ctx_accounts_base(expr: &syn::Expr, context_names: &[String]) -> bool {
    let syn::Expr::Field(accounts_field) = expr else {
        return false;
    };
    if !matches!(&accounts_field.member, syn::Member::Named(ident) if ident == "accounts") {
        return false;
    }
    matches!(
        accounts_field.base.as_ref(),
        syn::Expr::Path(path) if context_names.iter().any(|name| path.path.is_ident(name))
    )
}

fn is_accounts_alias(expr: &syn::Expr, accounts_aliases: &[String]) -> bool {
    matches!(
        expr,
        syn::Expr::Path(path) if accounts_aliases.iter().any(|alias| path.path.is_ident(alias))
    )
}

pub(super) fn is_accounts_container_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
) -> bool {
    match expr {
        expr if is_ctx_accounts_base(expr, context_names) => true,
        expr if is_accounts_alias(expr, accounts_aliases) => true,
        syn::Expr::Reference(reference) => {
            is_accounts_container_expr(reference.expr.as_ref(), context_names, accounts_aliases)
        }
        syn::Expr::Paren(paren) => {
            is_accounts_container_expr(paren.expr.as_ref(), context_names, accounts_aliases)
        }
        syn::Expr::Group(group) => {
            is_accounts_container_expr(group.expr.as_ref(), context_names, accounts_aliases)
        }
        syn::Expr::Unary(unary) => {
            is_accounts_container_expr(unary.expr.as_ref(), context_names, accounts_aliases)
        }
        _ => false,
    }
}

pub(super) fn account_usage_from_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
    account_field_aliases: &[AccountFieldAlias],
) -> Option<AccountUsage> {
    match expr {
        syn::Expr::Path(path) => {
            let ident = path.path.get_ident()?;
            account_field_aliases
                .iter()
                .find(|alias| ident == &alias.alias)
                .map(|alias| AccountUsage {
                    name: alias.account.clone(),
                    range: range_from_span(ident.span()),
                    mutable: false,
                })
        }
        syn::Expr::Field(expr_field)
            if is_accounts_container_expr(
                expr_field.base.as_ref(),
                context_names,
                accounts_aliases,
            ) =>
        {
            let syn::Member::Named(ident) = &expr_field.member else {
                return None;
            };
            Some(AccountUsage {
                name: ident.to_string(),
                range: range_from_span(ident.span()),
                mutable: false,
            })
        }
        syn::Expr::Field(expr_field) => account_usage_from_expr(
            expr_field.base.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::MethodCall(method_call) => account_usage_from_expr(
            method_call.receiver.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Reference(reference) => account_usage_from_expr(
            reference.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Paren(paren) => account_usage_from_expr(
            paren.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Group(group) => account_usage_from_expr(
            group.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Unary(unary) => account_usage_from_expr(
            unary.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        _ => None,
    }
}

pub(super) fn account_path_from_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
    account_field_aliases: &[AccountFieldAlias],
) -> Option<Vec<NamedRange>> {
    match expr {
        syn::Expr::Field(expr_field) => {
            let syn::Member::Named(ident) = &expr_field.member else {
                return None;
            };
            let segment = NamedRange {
                name: ident.to_string(),
                range: range_from_span(ident.span()),
            };
            if is_accounts_container_expr(expr_field.base.as_ref(), context_names, accounts_aliases)
            {
                return Some(vec![segment]);
            }
            if let Some(alias_segment) = account_field_alias_segment_for_expr(
                expr_field.base.as_ref(),
                account_field_aliases,
            ) {
                return Some(vec![alias_segment, segment]);
            }
            let mut path = account_path_from_expr(
                expr_field.base.as_ref(),
                context_names,
                accounts_aliases,
                account_field_aliases,
            )?;
            path.push(segment);
            Some(path)
        }
        syn::Expr::MethodCall(method_call) => account_path_from_expr(
            method_call.receiver.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Reference(reference) => account_path_from_expr(
            reference.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Paren(paren) => account_path_from_expr(
            paren.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Group(group) => account_path_from_expr(
            group.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Unary(unary) => account_path_from_expr(
            unary.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        _ => None,
    }
}

pub(super) fn account_key_from_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
    account_field_aliases: &[AccountFieldAlias],
) -> Option<String> {
    match expr {
        syn::Expr::MethodCall(method_call) if method_call.method == "key" => {
            account_usage_from_expr(
                method_call.receiver.as_ref(),
                context_names,
                accounts_aliases,
                account_field_aliases,
            )
            .map(|usage| usage.name)
        }
        syn::Expr::Reference(reference) => account_key_from_expr(
            reference.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Paren(paren) => account_key_from_expr(
            paren.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Group(group) => account_key_from_expr(
            group.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        syn::Expr::Unary(unary) => account_key_from_expr(
            unary.expr.as_ref(),
            context_names,
            accounts_aliases,
            account_field_aliases,
        ),
        _ => None,
    }
}
