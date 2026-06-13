use {
    super::HandlerMemberVisitor,
    crate::lsp::local_types,
    syn::{visit::Visit, BinOp, Expr, FnArg, ImplItemFn, ItemFn},
};

impl HandlerMemberVisitor<'_> {
    pub(super) fn declare_function_inputs(&mut self, item_fn: &ItemFn) {
        self.declare_signature_typed_inputs(&item_fn.sig.inputs);
    }

    /// Declare the typed parameters of an impl-block method into the current
    /// scope, mirroring `declare_function_inputs` for free functions.
    /// `self` receivers have no type annotation that we can resolve here, so
    /// they are skipped — the open-world principle: suppress rather than guess.
    pub(super) fn declare_impl_method_inputs(&mut self, method: &ImplItemFn) {
        self.declare_signature_typed_inputs(&method.sig.inputs);
    }

    fn declare_signature_typed_inputs(
        &mut self,
        inputs: &syn::punctuated::Punctuated<FnArg, syn::token::Comma>,
    ) {
        for input in inputs {
            let FnArg::Typed(pat_type) = input else {
                // `self` receiver: no resolvable type annotation available here.
                continue;
            };
            if let Some(context_name) = local_types::context_type_name_from_type(&pat_type.ty) {
                self.scopes.declare_context_pat(&pat_type.pat, context_name);
            }
            let item_type = local_types::iterable_item_type_name_from_type(&pat_type.ty);
            if let Some(item_type) = item_type.as_deref() {
                self.scopes
                    .declare_iterable_pat(&pat_type.pat, item_type.to_string());
            }
            self.scopes.declare_typed_pattern_from_type(
                self.document,
                self.workspace_index,
                &pat_type.pat,
                &pat_type.ty,
            );
        }
    }

    pub(super) fn visit_condition_with_typed_pattern_scope(&mut self, expr: &Expr) {
        match expr {
            Expr::Let(expr_let) => {
                self.visit_expr(&expr_let.expr);
                let wrapped_item_type = self.expression_optional_item_type_name(&expr_let.expr);
                let type_name = self.expression_type_name(&expr_let.expr).or_else(|| {
                    local_types::wrapper_pattern_type_name(&expr_let.pat).map(str::to_string)
                });
                if let Some(type_name) = type_name {
                    self.scopes.declare_typed_pattern_with_wrapped_item(
                        self.document,
                        self.workspace_index,
                        &expr_let.pat,
                        &type_name,
                        wrapped_item_type.as_deref(),
                    );
                }
            }
            Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
                self.visit_condition_with_typed_pattern_scope(&binary.left);
                self.visit_condition_with_typed_pattern_scope(&binary.right);
            }
            Expr::Group(group) => self.visit_condition_with_typed_pattern_scope(&group.expr),
            Expr::Paren(paren) => self.visit_condition_with_typed_pattern_scope(&paren.expr),
            _ => self.visit_expr(expr),
        }
    }

    pub(super) fn expression_optional_item_type_name(&self, expr: &Expr) -> Option<String> {
        local_types::expression_optional_item_type_name_with_item_scope(
            self.document,
            self.workspace_index,
            expr,
            &|name| self.scopes.get(name),
            &|name| self.scopes.get_context(name),
            &|name| self.scopes.get_iterable_item(name),
        )
    }
}
