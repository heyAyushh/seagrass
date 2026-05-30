use {
    super::is_accounts_container_expr,
    crate::{document::NamedRange, range::range_from_span},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AccountFieldAlias {
    pub(super) alias: String,
    pub(super) account: String,
}

pub(super) struct AccountFieldAliasTarget {
    pub(super) account: String,
    pub(super) source: String,
}

pub(super) fn account_field_alias_target_for_expr(
    expr: &syn::Expr,
    account_field_aliases: &[AccountFieldAlias],
) -> Option<AccountFieldAliasTarget> {
    match expr {
        syn::Expr::Path(path) => account_field_aliases
            .iter()
            .find(|alias| path.path.is_ident(&alias.alias))
            .map(|alias| AccountFieldAliasTarget {
                account: alias.account.clone(),
                source: alias.alias.clone(),
            }),
        syn::Expr::Reference(reference) => {
            account_field_alias_target_for_expr(&reference.expr, account_field_aliases)
        }
        syn::Expr::Paren(paren) => {
            account_field_alias_target_for_expr(&paren.expr, account_field_aliases)
        }
        syn::Expr::Group(group) => {
            account_field_alias_target_for_expr(&group.expr, account_field_aliases)
        }
        syn::Expr::Unary(unary) => {
            account_field_alias_target_for_expr(&unary.expr, account_field_aliases)
        }
        _ => None,
    }
}

pub(super) fn account_field_alias_segment_for_expr(
    expr: &syn::Expr,
    account_field_aliases: &[AccountFieldAlias],
) -> Option<NamedRange> {
    match expr {
        syn::Expr::Path(path) => {
            let ident = path.path.get_ident()?;
            account_field_aliases
                .iter()
                .find(|alias| ident == &alias.alias)
                .map(|alias| NamedRange {
                    name: alias.account.clone(),
                    range: range_from_span(ident.span()),
                })
        }
        syn::Expr::Reference(reference) => {
            account_field_alias_segment_for_expr(&reference.expr, account_field_aliases)
        }
        syn::Expr::Paren(paren) => {
            account_field_alias_segment_for_expr(&paren.expr, account_field_aliases)
        }
        syn::Expr::Group(group) => {
            account_field_alias_segment_for_expr(&group.expr, account_field_aliases)
        }
        syn::Expr::Unary(unary) => {
            account_field_alias_segment_for_expr(&unary.expr, account_field_aliases)
        }
        _ => None,
    }
}

pub(super) fn direct_account_usage_from_expr(
    expr: &syn::Expr,
    context_names: &[String],
    accounts_aliases: &[String],
) -> Option<String> {
    match expr {
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
            Some(ident.to_string())
        }
        syn::Expr::Reference(reference) => {
            direct_account_usage_from_expr(&reference.expr, context_names, accounts_aliases)
        }
        syn::Expr::Paren(paren) => {
            direct_account_usage_from_expr(&paren.expr, context_names, accounts_aliases)
        }
        syn::Expr::Group(group) => {
            direct_account_usage_from_expr(&group.expr, context_names, accounts_aliases)
        }
        syn::Expr::Unary(unary) => {
            direct_account_usage_from_expr(&unary.expr, context_names, accounts_aliases)
        }
        _ => None,
    }
}
