use {
    super::HandlerMemberVisitor,
    crate::lsp::local_types,
    syn::{visit::Visit, Expr, ExprMethodCall},
};

impl HandlerMemberVisitor<'_> {
    pub(super) fn visit_method_call_with_inferred_iterator_closure(
        &mut self,
        node: &ExprMethodCall,
    ) -> bool {
        let Some(item_type) = local_types::iterator_method_closure_item_type_name(
            self.document,
            self.workspace_index,
            node,
            &|name| self.scopes.get(name),
            &|name| self.scopes.get_context(name),
            &|name| self.scopes.get_iterable_item(name),
        ) else {
            return false;
        };
        let mut handled_closure = false;
        self.visit_expr(&node.receiver);
        for arg in &node.args {
            if let Expr::Closure(closure) = arg {
                handled_closure = true;
                self.visit_closure_with_inferred_input(closure, &item_type);
            } else {
                self.visit_expr(arg);
            }
        }
        handled_closure
    }

    fn visit_closure_with_inferred_input(&mut self, closure: &syn::ExprClosure, input_type: &str) {
        self.scopes.push();
        if let Some(input) = closure.inputs.iter().next() {
            self.scopes.declare_typed_pattern(
                self.document,
                self.workspace_index,
                input,
                input_type,
            );
        }
        for input in &closure.inputs {
            if let Some(item_type) = local_types::explicit_pattern_iterable_item_type_name(input) {
                self.scopes.declare_iterable_pat(input, item_type);
            }
            self.scopes
                .declare_explicit_typed_pattern(self.document, self.workspace_index, input);
        }
        self.visit_expr(&closure.body);
        self.scopes.pop();
    }
}
