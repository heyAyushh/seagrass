use {
    super::{
        context_accounts_type, AccountDataFieldUsage, AccountKeyComparison, AccountKeyOperand,
        AccountPathUsage, AccountUsage, FunctionCall, NamedRange,
    },
    crate::{anchor::idioms, range::range_from_span, syntax::member_is_named},
    aliases::{
        account_field_alias_target_for_expr, direct_account_usage_from_expr, AccountFieldAlias,
    },
    syn::{
        parse::Parser,
        visit::{self, Visit},
        FnArg, ItemFn, Path,
    },
};

#[path = "account_usage/aliases.rs"]
mod aliases;
#[path = "account_usage/expressions.rs"]
mod expressions;

use expressions::{
    account_key_operand_from_expr, account_path_from_expr, account_usage_from_expr,
    is_accounts_container_expr,
};

pub(super) fn account_evidence(item_fn: &ItemFn) -> AccountUsageVisitor {
    let mut visitor = AccountUsageVisitor {
        context_names: context_argument_names(item_fn),
        ..AccountUsageVisitor::default()
    };
    visitor.visit_block(&item_fn.block);
    visitor
}

#[derive(Default)]
pub(super) struct AccountUsageVisitor {
    pub(super) context_names: Vec<String>,
    pub(super) accounts_aliases: Vec<String>,
    account_field_aliases: Vec<AccountFieldAlias>,
    pub(super) usages: Vec<AccountUsage>,
    pub(super) function_calls: Vec<FunctionCall>,
    pub(super) data_field_usages: Vec<AccountDataFieldUsage>,
    pub(super) account_path_usages: Vec<AccountPathUsage>,
    pub(super) cpi_program_usages: Vec<AccountUsage>,
    pub(super) signer_usages: Vec<AccountUsage>,
    pub(super) signer_checks: Vec<AccountUsage>,
    pub(super) account_key_comparisons: Vec<AccountKeyComparison>,
    pub(super) token_account_unpack_usages: Vec<AccountUsage>,
    pub(super) mutable_depth: usize,
}

impl AccountUsageVisitor {
    fn record_usage(&mut self, expr_field: &syn::ExprField) {
        if !is_accounts_container_expr(
            expr_field.base.as_ref(),
            &self.context_names,
            &self.accounts_aliases,
        ) {
            return;
        }
        let syn::Member::Named(ident) = &expr_field.member else {
            return;
        };

        let usage = AccountUsage {
            name: ident.to_string(),
            range: range_from_span(ident.span()),
            mutable: self.mutable_depth > 0,
        };
        if !self.usages.iter().any(|existing| {
            existing.name == usage.name
                && existing.range == usage.range
                && existing.mutable == usage.mutable
        }) {
            self.usages.push(usage);
        }
    }

    fn record_data_field_usage(&mut self, expr_field: &syn::ExprField) {
        let syn::Member::Named(field_ident) = &expr_field.member else {
            return;
        };
        if let Some(target) = account_field_alias_target_for_expr(
            expr_field.base.as_ref(),
            &self.account_field_aliases,
        ) {
            self.push_data_field_usage(AccountDataFieldUsage {
                account: target.account,
                source_account: target.source,
                field: field_ident.to_string(),
                range: range_from_span(field_ident.span()),
                mutable: self.mutable_depth > 0,
            });
            return;
        }

        let syn::Expr::Field(account_field) = expr_field.base.as_ref() else {
            return;
        };
        if !is_accounts_container_expr(
            account_field.base.as_ref(),
            &self.context_names,
            &self.accounts_aliases,
        ) {
            return;
        }
        let syn::Member::Named(account_ident) = &account_field.member else {
            return;
        };

        self.push_data_field_usage(AccountDataFieldUsage {
            account: account_ident.to_string(),
            source_account: account_ident.to_string(),
            field: field_ident.to_string(),
            range: range_from_span(field_ident.span()),
            mutable: self.mutable_depth > 0,
        });
    }

    fn push_data_field_usage(&mut self, usage: AccountDataFieldUsage) {
        if !self.data_field_usages.iter().any(|existing| {
            existing.account == usage.account
                && existing.source_account == usage.source_account
                && existing.field == usage.field
                && existing.range == usage.range
                && existing.mutable == usage.mutable
        }) {
            self.data_field_usages.push(usage);
        }
    }

    fn record_account_path_usage(&mut self, expr: &syn::Expr) {
        let Some(segments) = account_path_from_expr(
            expr,
            &self.context_names,
            &self.accounts_aliases,
            &self.account_field_aliases,
        ) else {
            return;
        };
        if segments.len() < 2 {
            return;
        }
        let usage = AccountPathUsage {
            segments,
            mutable: self.mutable_depth > 0,
        };
        if !self.account_path_usages.iter().any(|existing| {
            existing.mutable == usage.mutable && existing.segments == usage.segments
        }) {
            self.account_path_usages.push(usage);
        }
    }

    fn record_cpi_program_usage(&mut self, expr: &syn::Expr) {
        let Some(usage) = self.account_usage_from_expr(expr) else {
            return;
        };
        push_unique_account_usage(&mut self.cpi_program_usages, usage);
    }

    fn record_signer_usage(&mut self, expr: &syn::Expr) {
        let Some(usage) = self.account_usage_from_expr(expr) else {
            return;
        };
        push_unique_account_usage(&mut self.signer_usages, usage);
    }

    fn record_signer_check(&mut self, expr: &syn::Expr) {
        let Some(usage) = self.account_usage_from_expr(expr) else {
            return;
        };
        push_unique_account_usage(&mut self.signer_checks, usage);
    }

    fn record_account_key_comparison(&mut self, left: &syn::Expr, right: &syn::Expr) {
        let Some(left) = self.account_key_operand_from_expr(left) else {
            return;
        };
        let Some(right) = self.account_key_operand_from_expr(right) else {
            return;
        };
        let comparison = AccountKeyComparison { left, right };
        if !comparison.has_account_operand() || comparison.compares_account_to_itself() {
            return;
        }
        if !self
            .account_key_comparisons
            .iter()
            .any(|existing| existing.matches_operands(&comparison.left, &comparison.right))
        {
            self.account_key_comparisons.push(comparison);
        }
    }

    fn record_token_account_unpack_usage(&mut self, expr: &syn::Expr) {
        let Some(usage) = self.account_usage_from_expr(expr) else {
            return;
        };
        push_unique_account_usage(&mut self.token_account_unpack_usages, usage);
    }

    fn record_function_call(&mut self, expr: &syn::Expr) {
        let syn::Expr::Path(path) = expr else {
            return;
        };
        let Some(segment) = path.path.segments.last() else {
            return;
        };
        let call = FunctionCall {
            name: segment.ident.to_string(),
            range: range_from_span(segment.ident.span()),
        };
        if !self
            .function_calls
            .iter()
            .any(|existing| existing.name == call.name && existing.range == call.range)
        {
            self.function_calls.push(call);
        }
    }

    fn record_method_call(&mut self, method: &syn::Ident) {
        let call = FunctionCall {
            name: method.to_string(),
            range: range_from_span(method.span()),
        };
        if !self
            .function_calls
            .iter()
            .any(|existing| existing.name == call.name && existing.range == call.range)
        {
            self.function_calls.push(call);
        }
    }

    fn record_accounts_alias(&mut self, local: &syn::Local) {
        let syn::Pat::Ident(pat_ident) = &local.pat else {
            return;
        };
        let Some(init) = &local.init else {
            return;
        };
        if !is_accounts_container_expr(&init.expr, &self.context_names, &self.accounts_aliases) {
            return;
        }
        let alias = pat_ident.ident.to_string();
        if !self
            .accounts_aliases
            .iter()
            .any(|existing| existing == &alias)
        {
            self.accounts_aliases.push(alias);
        }
    }

    fn record_account_field_alias(&mut self, local: &syn::Local) {
        let syn::Pat::Ident(pat_ident) = &local.pat else {
            return;
        };
        let Some(init) = &local.init else {
            return;
        };
        let Some(account) =
            direct_account_usage_from_expr(&init.expr, &self.context_names, &self.accounts_aliases)
        else {
            return;
        };
        let alias = AccountFieldAlias {
            alias: pat_ident.ident.to_string(),
            account,
        };
        if !self
            .account_field_aliases
            .iter()
            .any(|existing| existing == &alias)
        {
            self.account_field_aliases.push(alias);
        }
    }

    fn with_mutable_context(&mut self, visit: impl FnOnce(&mut Self)) {
        self.mutable_depth += 1;
        visit(self);
        self.mutable_depth -= 1;
    }

    fn account_usage_from_expr(&self, expr: &syn::Expr) -> Option<AccountUsage> {
        account_usage_from_expr(
            expr,
            &self.context_names,
            &self.accounts_aliases,
            &self.account_field_aliases,
        )
    }

    fn account_key_operand_from_expr(&self, expr: &syn::Expr) -> Option<AccountKeyOperand> {
        account_key_operand_from_expr(
            expr,
            &self.context_names,
            &self.accounts_aliases,
            &self.account_field_aliases,
        )
    }
}

impl<'ast> Visit<'ast> for AccountUsageVisitor {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        self.record_accounts_alias(node);
        self.record_account_field_alias(node);
        visit::visit_local(self, node);
    }

    fn visit_expr_assign(&mut self, node: &'ast syn::ExprAssign) {
        self.with_mutable_context(|visitor| visitor.visit_expr(&node.left));
        self.visit_expr(&node.right);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if is_key_comparison_operator(&node.op) {
            self.record_account_key_comparison(&node.left, &node.right);
            visit::visit_expr_binary(self, node);
        } else if is_assignment_operator(&node.op) {
            self.with_mutable_context(|visitor| visitor.visit_expr(&node.left));
            self.visit_expr(&node.right);
        } else {
            visit::visit_expr_binary(self, node);
        }
    }

    fn visit_expr_reference(&mut self, node: &'ast syn::ExprReference) {
        if node.mutability.is_some() {
            self.with_mutable_context(|visitor| visitor.visit_expr(&node.expr));
        } else {
            visit::visit_expr_reference(self, node);
        }
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        self.record_method_call(&node.method);
        if is_mutating_account_method(&node.method.to_string()) {
            self.with_mutable_context(|visitor| visitor.visit_expr(&node.receiver));
            for arg in &node.args {
                self.visit_expr(arg);
            }
        } else {
            visit::visit_expr_method_call(self, node);
        }
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        self.record_function_call(node.func.as_ref());
        if is_cpi_context_constructor(node.func.as_ref())
            || is_instruction_constructor(node.func.as_ref())
        {
            if let Some(first_arg) = node.args.first() {
                self.record_cpi_program_usage(first_arg);
            }
        }
        if is_account_meta_constructor(node.func.as_ref())
            && node.args.len() >= 2
            && node.args.iter().nth(1).is_some_and(is_bool_true)
        {
            if let Some(first_arg) = node.args.first() {
                self.record_signer_usage(first_arg);
            }
        }
        if is_token_account_unpack_call(node.func.as_ref()) {
            if let Some(first_arg) = node.args.first() {
                self.record_token_account_unpack_usage(first_arg);
            }
        }
        visit::visit_expr_call(self, node);
    }

    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        self.record_data_field_usage(node);
        self.record_usage(node);
        self.record_account_path_usage(&syn::Expr::Field(node.clone()));
        if member_is_named(&node.member, "is_signer") {
            self.record_signer_check(node.base.as_ref());
        }
        visit::visit_expr_field(self, node);
    }

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if is_instruction_path(&node.path) {
            for field in &node.fields {
                if member_is_named(&field.member, "program_id") {
                    self.record_cpi_program_usage(&field.expr);
                }
            }
        } else if is_account_meta_path(&node.path) {
            let pubkey = node
                .fields
                .iter()
                .find(|field| member_is_named(&field.member, "pubkey"))
                .map(|field| &field.expr);
            let is_signer = node
                .fields
                .iter()
                .find(|field| member_is_named(&field.member, "is_signer"))
                .is_some_and(|field| is_bool_true(&field.expr));
            if is_signer {
                if let Some(pubkey) = pubkey {
                    self.record_signer_usage(pubkey);
                }
            }
        }
        visit::visit_expr_struct(self, node);
    }

    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !is_account_evidence_macro(node) {
            return;
        }
        if let Ok(expressions) =
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated
                .parse2(node.tokens.clone())
        {
            for expression in expressions {
                self.visit_expr(&expression);
            }
        }
        visit::visit_macro(self, node);
    }
}

fn is_account_evidence_macro(node: &syn::Macro) -> bool {
    node.path
        .segments
        .last()
        .is_some_and(|segment| is_account_evidence_macro_name(&segment.ident))
}

fn is_account_evidence_macro_name(ident: &syn::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "assert"
            | "assert_eq"
            | "assert_ne"
            | "require"
            | "require_eq"
            | "require_keys_eq"
            | "require_keys_neq"
            | "require_neq"
            | "vec"
    )
}

fn context_argument_names(item_fn: &ItemFn) -> Vec<String> {
    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| {
            let FnArg::Typed(pat_type) = arg else {
                return None;
            };
            context_accounts_type(pat_type)?;
            let syn::Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
                return None;
            };
            Some(pat_ident.ident.to_string())
        })
        .collect()
}

impl AccountKeyComparison {
    pub fn matches(&self, left: &str, right: &str) -> bool {
        self.matches_operands(
            &AccountKeyOperand::Account(left.to_string()),
            &AccountKeyOperand::Account(right.to_string()),
        )
    }

    pub fn matches_operands(&self, left: &AccountKeyOperand, right: &AccountKeyOperand) -> bool {
        (self.left == *left && self.right == *right) || (self.left == *right && self.right == *left)
    }

    pub fn compares_account_to_static_program_id(&self, account: &str) -> bool {
        matches!(
            (&self.left, &self.right),
            (AccountKeyOperand::Account(left), AccountKeyOperand::StaticProgramId)
                if left == account
        ) || matches!(
            (&self.left, &self.right),
            (AccountKeyOperand::StaticProgramId, AccountKeyOperand::Account(right))
                if right == account
        )
    }

    fn has_account_operand(&self) -> bool {
        matches!(self.left, AccountKeyOperand::Account(_))
            || matches!(self.right, AccountKeyOperand::Account(_))
    }

    fn compares_account_to_itself(&self) -> bool {
        matches!(
            (&self.left, &self.right),
            (AccountKeyOperand::Account(left), AccountKeyOperand::Account(right)) if left == right
        )
    }
}

fn push_unique_account_usage(usages: &mut Vec<AccountUsage>, usage: AccountUsage) {
    if !usages
        .iter()
        .any(|existing| existing.name == usage.name && existing.range == usage.range)
    {
        usages.push(usage);
    }
}

fn is_key_comparison_operator(op: &syn::BinOp) -> bool {
    matches!(op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_))
}

fn is_assignment_operator(op: &syn::BinOp) -> bool {
    matches!(
        op,
        syn::BinOp::AddAssign(_)
            | syn::BinOp::SubAssign(_)
            | syn::BinOp::MulAssign(_)
            | syn::BinOp::DivAssign(_)
            | syn::BinOp::RemAssign(_)
            | syn::BinOp::BitXorAssign(_)
            | syn::BinOp::BitAndAssign(_)
            | syn::BinOp::BitOrAssign(_)
            | syn::BinOp::ShlAssign(_)
            | syn::BinOp::ShrAssign(_)
    )
}

fn is_cpi_context_constructor(expr: &syn::Expr) -> bool {
    let Some(path) = expr_path(expr) else {
        return false;
    };
    path.segments.last().is_some_and(|segment| {
        segment.ident == idioms::CPI_CONTEXT_NEW_METHOD
            || segment.ident == idioms::CPI_CONTEXT_NEW_WITH_SIGNER_METHOD
    }) && path
        .segments
        .iter()
        .any(|segment| segment.ident == idioms::CPI_CONTEXT_TYPE)
}

fn is_instruction_constructor(expr: &syn::Expr) -> bool {
    let Some(path) = expr_path(expr) else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment.ident.to_string().starts_with("new"))
        && is_instruction_path(path)
}

fn is_account_meta_constructor(expr: &syn::Expr) -> bool {
    let Some(path) = expr_path(expr) else {
        return false;
    };
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "new" || segment.ident == "new_readonly")
        && is_account_meta_path(path)
}

fn is_token_account_unpack_call(expr: &syn::Expr) -> bool {
    let Some(path) = expr_path(expr) else {
        return false;
    };
    let Some(last) = path.segments.last() else {
        return false;
    };
    if !idioms::ident_is_any(&last.ident, idioms::TOKEN_ACCOUNT_UNPACK_METHODS) {
        return false;
    }
    path.segments
        .iter()
        .any(|segment| idioms::ident_is_any(&segment.ident, idioms::TOKEN_ACCOUNT_UNPACK_TYPES))
}

fn is_instruction_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "Instruction")
        || path
            .segments
            .iter()
            .any(|segment| segment.ident == "Instruction")
}

fn is_account_meta_path(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "AccountMeta")
        || path
            .segments
            .iter()
            .any(|segment| segment.ident == "AccountMeta")
}

fn expr_path(expr: &syn::Expr) -> Option<&Path> {
    match expr {
        syn::Expr::Path(path) => Some(&path.path),
        _ => None,
    }
}

fn is_bool_true(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Bool(value),
            ..
        }) if value.value
    )
}

fn is_mutating_account_method(method: &str) -> bool {
    method == idioms::ACCOUNT_RELOAD_METHOD
        || matches!(method, "set_inner" | "load_mut" | "close" | "realloc")
}
