use {
    super::{iterables, patterns, ContextLocalValue, TypedLocalValue},
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        BinOp, Expr, FnArg, ItemFn, Pat, Stmt,
    },
    tower_lsp::lsp_types::Position,
};

pub(super) fn typed_values_at(
    document: &ParsedDocument,
    position: Position,
    workspace_index: Option<&WorkspaceIndex>,
) -> Vec<TypedLocalValue> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleTypedValueCollector {
        document,
        workspace_index,
        source: document.source(),
        cursor_offset,
        values: Vec::new(),
        iterable_values: Vec::new(),
        context_values: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector.values
}

pub(super) fn context_values_at(
    document: &ParsedDocument,
    position: Position,
) -> Vec<ContextLocalValue> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleTypedValueCollector {
        document,
        workspace_index: None,
        source: document.source(),
        cursor_offset,
        values: Vec::new(),
        iterable_values: Vec::new(),
        context_values: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector
        .context_values
        .into_iter()
        .map(|value| ContextLocalValue {
            name: value.name,
            accounts_type_name: value.type_name,
        })
        .collect()
}

struct VisibleTypedValueCollector<'a> {
    document: &'a ParsedDocument,
    workspace_index: Option<&'a WorkspaceIndex>,
    source: &'a str,
    cursor_offset: usize,
    values: Vec<TypedLocalValue>,
    iterable_values: Vec<TypedLocalValue>,
    context_values: Vec<TypedLocalValue>,
    found_cursor_function: bool,
}

impl VisibleTypedValueCollector<'_> {
    fn collect_function(&mut self, item_fn: &ItemFn) {
        self.collect_function_inputs(item_fn);
        self.collect_block_bindings(&item_fn.block);
        self.found_cursor_function = true;
    }

    fn collect_function_inputs(&mut self, item_fn: &ItemFn) {
        for input in &item_fn.sig.inputs {
            let FnArg::Typed(pat_type) = input else {
                continue;
            };
            if let Some(context_name) = super::context_type_name_from_type(&pat_type.ty) {
                self.add_context_pattern_candidate(&pat_type.pat, context_name);
            }
            if let Some(item_type) = iterables::item_type_name_from_type(&pat_type.ty) {
                self.add_iterable_pattern_candidate(&pat_type.pat, item_type);
            }
            let Some(type_name) = super::local_value_type_name_from_type(&pat_type.ty) else {
                continue;
            };
            self.add_typed_pattern_candidates(&pat_type.pat, &type_name);
        }
    }

    fn collect_block_bindings(&mut self, block: &syn::Block) -> bool {
        for stmt in &block.stmts {
            let stmt_range = crate::range::range_from_span(stmt.span());
            let Some(stmt_start) = crate::range::byte_offset_at(self.source, stmt_range.start)
            else {
                continue;
            };
            let Some(stmt_end) = crate::range::byte_offset_at(self.source, stmt_range.end) else {
                continue;
            };

            if self.cursor_offset < stmt_start {
                return true;
            }
            if self.cursor_offset <= stmt_end {
                return self.collect_bindings_inside_statement(stmt);
            }
            if let Stmt::Local(local) = stmt {
                self.collect_local(local);
            }
        }
        false
    }

    fn collect_bindings_inside_statement(&mut self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Expr(expr, _) => self.collect_bindings_inside_expr(expr),
            Stmt::Local(local) => local.init.as_ref().is_none_or(|init| {
                !self.span_contains_cursor(init.expr.span())
                    || self.collect_bindings_inside_expr(&init.expr)
            }),
            Stmt::Item(_) | Stmt::Macro(_) => true,
        }
    }

    fn collect_bindings_inside_expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Block(block) => self.collect_block_bindings(&block.block),
            Expr::ForLoop(for_loop) if self.span_contains_cursor(for_loop.body.span()) => {
                if let Some(item_type) = self.expression_iterable_item_type_name(&for_loop.expr) {
                    self.add_typed_pattern_candidates(&for_loop.pat, &item_type);
                }
                self.collect_block_bindings(&for_loop.body)
            }
            Expr::If(if_expr) => {
                if self.span_contains_cursor(if_expr.then_branch.span()) {
                    self.collect_condition_pattern_candidates(&if_expr.cond);
                    return self.collect_block_bindings(&if_expr.then_branch);
                }
                if let Some((_, else_branch)) = &if_expr.else_branch {
                    return self.collect_bindings_inside_expr(else_branch);
                }
                true
            }
            Expr::Loop(loop_expr) if self.span_contains_cursor(loop_expr.body.span()) => {
                self.collect_block_bindings(&loop_expr.body)
            }
            Expr::Closure(closure) if self.span_contains_cursor(closure.body.span()) => {
                self.collect_closure_inputs(closure);
                self.collect_bindings_inside_expr(&closure.body)
            }
            Expr::While(while_expr) if self.span_contains_cursor(while_expr.body.span()) => {
                self.collect_condition_pattern_candidates(&while_expr.cond);
                self.collect_block_bindings(&while_expr.body)
            }
            Expr::Match(match_expr) => {
                let scrutinee_type = self.expression_type_name(&match_expr.expr);
                for arm in &match_expr.arms {
                    if arm
                        .guard
                        .as_ref()
                        .is_some_and(|(_, guard)| self.span_contains_cursor(guard.span()))
                    {
                        if let Some(type_name) = scrutinee_type.as_deref() {
                            self.add_typed_pattern_candidates(&arm.pat, type_name);
                        }
                        return true;
                    }
                    if self.span_contains_cursor(arm.body.span()) {
                        if let Some(type_name) = scrutinee_type.as_deref() {
                            self.add_typed_pattern_candidates(&arm.pat, type_name);
                        }
                        return self.collect_bindings_inside_expr(&arm.body);
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn collect_closure_inputs(&mut self, closure: &syn::ExprClosure) {
        for input in &closure.inputs {
            if let Some(item_type) = iterables::explicit_pattern_item_type_name(input) {
                self.add_iterable_pattern_candidate(input, item_type);
            }
            let Some(type_name) = super::explicit_pattern_type_name(input) else {
                continue;
            };
            self.add_typed_pattern_candidates(input, &type_name);
        }
    }

    fn collect_local(&mut self, local: &syn::Local) {
        if let Some(item_type) = super::local_iterable_item_type_name_with_scope(local, &|name| {
            self.visible_iterable_item_type_name(name)
        }) {
            self.add_iterable_pattern_candidate(&local.pat, item_type);
        }
        let Some(type_name) = super::local_type_name_with_item_scope(
            self.document,
            self.workspace_index,
            local,
            &|name| self.visible_type_name(name),
            &|name| self.visible_context_type_name(name),
            &|name| self.visible_iterable_item_type_name(name),
        ) else {
            return;
        };
        self.add_typed_pattern_candidates(&local.pat, &type_name);
    }

    fn collect_condition_pattern_candidates(&mut self, expr: &Expr) {
        match expr {
            Expr::Let(expr_let) => {
                if let Some(type_name) = self.expression_type_name(&expr_let.expr) {
                    self.add_typed_pattern_candidates(&expr_let.pat, &type_name);
                }
            }
            Expr::Binary(binary) if matches!(binary.op, BinOp::And(_)) => {
                self.collect_condition_pattern_candidates(&binary.left);
                self.collect_condition_pattern_candidates(&binary.right);
            }
            Expr::Group(group) => self.collect_condition_pattern_candidates(&group.expr),
            Expr::Paren(paren) => self.collect_condition_pattern_candidates(&paren.expr),
            _ => {}
        }
    }

    fn expression_type_name(&self, expr: &Expr) -> Option<String> {
        super::expression_type_name_with_item_scope(
            self.document,
            self.workspace_index,
            expr,
            &|name| self.visible_type_name(name),
            &|name| self.visible_context_type_name(name),
            &|name| self.visible_iterable_item_type_name(name),
        )
    }

    fn expression_iterable_item_type_name(&self, expr: &Expr) -> Option<String> {
        super::expression_iterable_item_type_name_with_scope(expr, &|name| {
            self.visible_iterable_item_type_name(name)
        })
    }

    fn add_typed_pattern_candidates(&mut self, pat: &Pat, type_name: &str) {
        self.values.extend(patterns::typed_pattern_bindings(
            self.document,
            self.workspace_index,
            pat,
            type_name,
        ));
    }

    fn add_context_pattern_candidate(&mut self, pat: &Pat, type_name: String) {
        let Some(name) = crate::lsp::scope::pattern_binding_name(pat) else {
            return;
        };
        self.context_values
            .push(TypedLocalValue { name, type_name });
    }

    fn add_iterable_pattern_candidate(&mut self, pat: &Pat, type_name: String) {
        let Some(name) = crate::lsp::scope::pattern_binding_name(pat) else {
            return;
        };
        self.iterable_values
            .push(TypedLocalValue { name, type_name });
    }

    fn span_contains_cursor(&self, span: proc_macro2::Span) -> bool {
        let range = crate::range::range_from_span(span);
        let Some(start) = crate::range::byte_offset_at(self.source, range.start) else {
            return false;
        };
        let Some(end) = crate::range::byte_offset_at(self.source, range.end) else {
            return false;
        };
        start <= self.cursor_offset && self.cursor_offset <= end
    }

    fn visible_type_name(&self, name: &str) -> Option<String> {
        self.values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    }

    fn visible_context_type_name(&self, name: &str) -> Option<String> {
        self.context_values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    }

    fn visible_iterable_item_type_name(&self, name: &str) -> Option<String> {
        self.iterable_values
            .iter()
            .rev()
            .find(|value| value.name == name)
            .map(|value| value.type_name.clone())
    }
}

impl<'ast> Visit<'ast> for VisibleTypedValueCollector<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.found_cursor_function || !self.span_contains_cursor(node.block.span()) {
            return;
        }
        self.collect_function(node);
    }

    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !self.found_cursor_function {
            visit::visit_item_mod(self, node);
        }
    }
}
