use {
    super::type_names,
    crate::document::ParsedDocument,
    syn::{
        visit::{self, Visit},
        Expr, ExprCall, ExprPath, ItemFn, ItemMod, ReturnType,
    },
};

#[derive(Clone, Copy)]
enum ReturnMode {
    Direct,
    TryUnwrap,
}

pub(super) fn call_return_type_name(document: &ParsedDocument, call: &ExprCall) -> Option<String> {
    let function_path = called_function_path(&call.func)?;
    local_function_return_type_name(document, function_path, ReturnMode::Direct)
}

pub(super) fn try_call_return_type_name(document: &ParsedDocument, expr: &Expr) -> Option<String> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let function_path = called_function_path(&call.func)?;
    local_function_return_type_name(document, function_path, ReturnMode::TryUnwrap)
}

fn called_function_path(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::Path(path) => path_function_path(path),
        Expr::Paren(paren) => called_function_path(&paren.expr),
        Expr::Group(group) => called_function_path(&group.expr),
        _ => None,
    }
}

fn path_function_path(path: &ExprPath) -> Option<Vec<String>> {
    path.qself.is_none().then(|| {
        let mut segments = path
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        if matches!(segments.first().map(String::as_str), Some("crate" | "self")) {
            segments.remove(0);
        }
        (!segments.is_empty()).then_some(segments)
    })?
}

fn local_function_return_type_name(
    document: &ParsedDocument,
    function_path: Vec<String>,
    mode: ReturnMode,
) -> Option<String> {
    let mut visitor = LocalFunctionReturnVisitor {
        function_path,
        mode,
        module_path: Vec::new(),
        return_type_names: Vec::new(),
    };
    visitor.visit_file(document.syntax());
    visitor.unique_return_type_name()
}

struct LocalFunctionReturnVisitor {
    function_path: Vec<String>,
    mode: ReturnMode,
    module_path: Vec<String>,
    return_type_names: Vec<String>,
}

impl LocalFunctionReturnVisitor {
    fn unique_return_type_name(mut self) -> Option<String> {
        self.return_type_names.sort();
        self.return_type_names.dedup();
        (self.return_type_names.len() == 1).then(|| self.return_type_names.remove(0))
    }

    fn push_return_type_name(&mut self, output: &ReturnType) {
        let type_name = match self.mode {
            ReturnMode::Direct => type_names::return_type_name(output),
            ReturnMode::TryUnwrap => type_names::try_return_type_name(output),
        };
        if let Some(type_name) = type_name {
            self.return_type_names.push(type_name);
        }
    }

    fn current_function_matches(&self, item_fn: &ItemFn) -> bool {
        let Some(function_name) = self.function_path.last() else {
            return false;
        };
        if item_fn.sig.ident != function_name.as_str() {
            return false;
        }
        let qualifier = &self.function_path[..self.function_path.len().saturating_sub(1)];
        qualifier.is_empty() || qualifier == self.module_path.as_slice()
    }
}

impl<'ast> Visit<'ast> for LocalFunctionReturnVisitor {
    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        let Some((_, items)) = &node.content else {
            return;
        };
        self.module_path.push(node.ident.to_string());
        for item in items {
            self.visit_item(item);
        }
        self.module_path.pop();
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if self.current_function_matches(node) {
            self.push_return_type_name(&node.sig.output);
        }
        visit::visit_item_fn(self, node);
    }
}
