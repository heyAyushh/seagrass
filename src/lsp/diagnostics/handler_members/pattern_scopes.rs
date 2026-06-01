use {
    super::HandlerMemberVisitor,
    crate::lsp::local_types,
    syn::{visit::Visit, BinOp, Expr, FnArg, ItemFn},
};

impl HandlerMemberVisitor<'_> {
    pub(super) fn declare_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            let FnArg::Typed(pat_type) = input else {
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
