use {
    crate::document::ParsedDocument,
    syn::{
        spanned::Spanned,
        visit::{self, Visit},
        Expr, FnArg, GenericArgument, ItemFn, Pat, PathArguments, Stmt, Type,
    },
    tower_lsp::lsp_types::Position,
};

const TRANSPARENT_LOCAL_TYPE_WRAPPERS: &[&str] = &["Box"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypedLocalValue {
    pub(crate) name: String,
    pub(crate) type_name: String,
}

pub(crate) fn visible_typed_values_at(
    document: &ParsedDocument,
    position: Position,
) -> Vec<TypedLocalValue> {
    let Some(cursor_offset) = crate::range::byte_offset_at(document.source(), position) else {
        return Vec::new();
    };
    let mut collector = VisibleTypedValueCollector {
        source: document.source(),
        cursor_offset,
        values: Vec::new(),
        found_cursor_function: false,
    };
    collector.visit_file(document.syntax());
    collector.values
}

pub(crate) fn shallow_type_name(ty: &Type) -> Option<String> {
    let ty = match ty {
        Type::Reference(reference) => reference.elem.as_ref(),
        Type::Ptr(pointer) => pointer.elem.as_ref(),
        Type::Paren(paren) => paren.elem.as_ref(),
        Type::Group(group) => group.elem.as_ref(),
        _ => ty,
    };
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    let type_name = segment.ident.to_string();
    if !TRANSPARENT_LOCAL_TYPE_WRAPPERS.contains(&type_name.as_str()) {
        return Some(type_name);
    }

    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Some(type_name);
    };
    let Some(GenericArgument::Type(inner)) = args.args.first() else {
        return Some(type_name);
    };
    shallow_type_name(inner).or(Some(type_name))
}

pub(crate) fn constructed_type_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Struct(expr_struct) => expr_struct
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        Expr::Reference(reference) => constructed_type_name(&reference.expr),
        Expr::Paren(paren) => constructed_type_name(&paren.expr),
        Expr::Group(group) => constructed_type_name(&group.expr),
        _ => None,
    }
}

struct VisibleTypedValueCollector<'a> {
    source: &'a str,
    cursor_offset: usize,
    values: Vec<TypedLocalValue>,
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
            let Some(type_name) = shallow_type_name(&pat_type.ty) else {
                continue;
            };
            self.add_pattern_candidate(&pat_type.pat, type_name);
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
            Stmt::Local(_) | Stmt::Item(_) | Stmt::Macro(_) => true,
        }
    }

    fn collect_bindings_inside_expr(&mut self, expr: &Expr) -> bool {
        match expr {
            Expr::Block(block) => self.collect_block_bindings(&block.block),
            Expr::ForLoop(for_loop) if self.span_contains_cursor(for_loop.body.span()) => {
                self.collect_block_bindings(&for_loop.body)
            }
            Expr::If(if_expr) => {
                if self.span_contains_cursor(if_expr.then_branch.span()) {
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
            Expr::While(while_expr) if self.span_contains_cursor(while_expr.body.span()) => {
                self.collect_block_bindings(&while_expr.body)
            }
            Expr::Match(match_expr) => {
                for arm in &match_expr.arms {
                    if self.span_contains_cursor(arm.body.span()) {
                        return self.collect_bindings_inside_expr(&arm.body);
                    }
                }
                true
            }
            _ => true,
        }
    }

    fn collect_local(&mut self, local: &syn::Local) {
        let Some(type_name) = local_type_name(local) else {
            return;
        };
        self.add_pattern_candidate(&local.pat, type_name);
    }

    fn add_pattern_candidate(&mut self, pat: &Pat, type_name: String) {
        let Some(name) = crate::lsp::scope::pattern_binding_name(pat) else {
            return;
        };
        self.values.push(TypedLocalValue { name, type_name });
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

pub(crate) fn local_type_name(local: &syn::Local) -> Option<String> {
    explicit_pattern_type_name(&local.pat).or_else(|| {
        local
            .init
            .as_ref()
            .and_then(|init| constructed_type_name(&init.expr))
    })
}

fn explicit_pattern_type_name(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Type(typed) => shallow_type_name(&typed.ty),
        Pat::Reference(reference) => explicit_pattern_type_name(&reference.pat),
        Pat::Paren(paren) => explicit_pattern_type_name(&paren.pat),
        _ => None,
    }
}
