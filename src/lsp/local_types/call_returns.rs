use {
    super::type_names,
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{
        visit::{self, Visit},
        Expr, ExprCall, ExprPath, ImplItem, ItemFn, ItemImpl, ItemMod, ReturnType, Type,
    },
};

#[derive(Clone, Copy)]
enum ReturnMode {
    Direct,
    TryUnwrap,
}

pub(super) fn call_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    call: &ExprCall,
) -> Option<String> {
    let function_path = called_function_path(&call.func)?;
    associated_function_return_type_name(
        document,
        workspace_index,
        function_path.as_slice(),
        ReturnMode::Direct,
    )
    .or_else(|| {
        free_function_return_type_name(
            document,
            workspace_index,
            function_path.as_slice(),
            ReturnMode::Direct,
        )
    })
}

pub(super) fn try_call_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
) -> Option<String> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let function_path = called_function_path(&call.func)?;
    associated_function_return_type_name(
        document,
        workspace_index,
        function_path.as_slice(),
        ReturnMode::TryUnwrap,
    )
    .or_else(|| {
        free_function_return_type_name(
            document,
            workspace_index,
            function_path.as_slice(),
            ReturnMode::TryUnwrap,
        )
    })
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

fn associated_function_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    function_path: &[String],
    mode: ReturnMode,
) -> Option<String> {
    let associated_path = AssociatedFunctionPath::from_segments(function_path)?;
    local_associated_function_return_type_name(
        document,
        associated_path.owner_type,
        associated_path.function_name,
        mode,
    )
    .or_else(|| {
        workspace_associated_function_return_type_name(
            workspace_index,
            associated_path.owner_type,
            associated_path.function_name,
            mode,
        )
    })
}

struct AssociatedFunctionPath<'a> {
    owner_type: &'a str,
    function_name: &'a str,
}

impl<'a> AssociatedFunctionPath<'a> {
    fn from_segments(segments: &'a [String]) -> Option<Self> {
        let [.., owner_type, function_name] = segments else {
            return None;
        };
        Some(Self {
            owner_type,
            function_name,
        })
    }
}

fn local_associated_function_return_type_name(
    document: &ParsedDocument,
    owner_type: &str,
    function_name: &str,
    mode: ReturnMode,
) -> Option<String> {
    let mut visitor = LocalAssociatedFunctionReturnVisitor {
        owner_type,
        function_name,
        mode,
        return_type_names: Vec::new(),
    };
    visitor.visit_file(document.syntax());
    visitor.unique_return_type_name()
}

fn local_function_return_type_name(
    document: &ParsedDocument,
    function_path: &[String],
    mode: ReturnMode,
) -> Option<String> {
    let mut visitor = LocalFunctionReturnVisitor {
        function_path: function_path.to_vec(),
        mode,
        module_path: Vec::new(),
        return_type_names: Vec::new(),
    };
    visitor.visit_file(document.syntax());
    visitor.unique_return_type_name()
}

fn workspace_function_return_type_name(
    workspace_index: Option<&WorkspaceIndex>,
    function_path: &[String],
    mode: ReturnMode,
) -> Option<String> {
    let function_name = function_path.last()?;
    let mut return_type_names = workspace_index?
        .function_return_type_displays(function_name)
        .into_iter()
        .filter_map(|display| match mode {
            ReturnMode::Direct => type_names::return_type_name_from_text(&display),
            ReturnMode::TryUnwrap => type_names::try_return_type_name_from_text(&display),
        })
        .collect::<Vec<_>>();
    return_type_names.sort();
    return_type_names.dedup();
    (return_type_names.len() == 1).then(|| return_type_names.remove(0))
}

fn workspace_associated_function_return_type_name(
    workspace_index: Option<&WorkspaceIndex>,
    owner_type: &str,
    function_name: &str,
    mode: ReturnMode,
) -> Option<String> {
    let mut return_type_names = workspace_index?
        .associated_values_in_container(owner_type)
        .into_iter()
        .filter(|value| value.kind == tower_lsp::lsp_types::SymbolKind::FUNCTION)
        .filter(|value| value.name == function_name)
        .filter_map(|value| value.type_display)
        .filter_map(|display| return_type_name_from_display(&display, owner_type, mode))
        .collect::<Vec<_>>();
    return_type_names.sort();
    return_type_names.dedup();
    (return_type_names.len() == 1).then(|| return_type_names.remove(0))
}

fn free_function_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    function_path: &[String],
    mode: ReturnMode,
) -> Option<String> {
    local_function_return_type_name(document, function_path, mode)
        .or_else(|| workspace_function_return_type_name(workspace_index, function_path, mode))
}

fn return_type_name_from_output(
    output: &ReturnType,
    self_type: &str,
    mode: ReturnMode,
) -> Option<String> {
    let type_name = match mode {
        ReturnMode::Direct => type_names::return_type_name(output),
        ReturnMode::TryUnwrap => type_names::try_return_type_name(output),
    }?;
    Some(resolve_self_type(type_name, self_type))
}

fn return_type_name_from_display(
    display: &str,
    self_type: &str,
    mode: ReturnMode,
) -> Option<String> {
    let type_name = match mode {
        ReturnMode::Direct => type_names::return_type_name_from_text(display),
        ReturnMode::TryUnwrap => type_names::try_return_type_name_from_text(display),
    }?;
    Some(resolve_self_type(type_name, self_type))
}

fn resolve_self_type(type_name: String, self_type: &str) -> String {
    if type_name == "Self" {
        self_type.to_string()
    } else {
        type_name
    }
}

fn impl_self_type_name(item_impl: &ItemImpl) -> Option<String> {
    let Type::Path(type_path) = item_impl.self_ty.as_ref() else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

struct LocalAssociatedFunctionReturnVisitor<'a> {
    owner_type: &'a str,
    function_name: &'a str,
    mode: ReturnMode,
    return_type_names: Vec<String>,
}

impl LocalAssociatedFunctionReturnVisitor<'_> {
    fn unique_return_type_name(mut self) -> Option<String> {
        self.return_type_names.sort();
        self.return_type_names.dedup();
        (self.return_type_names.len() == 1).then(|| self.return_type_names.remove(0))
    }

    fn push_return_type_name(&mut self, output: &ReturnType) {
        if let Some(type_name) = return_type_name_from_output(output, self.owner_type, self.mode) {
            self.return_type_names.push(type_name);
        }
    }
}

impl<'ast> Visit<'ast> for LocalAssociatedFunctionReturnVisitor<'_> {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if node.trait_.is_some() || impl_self_type_name(node).as_deref() != Some(self.owner_type) {
            visit::visit_item_impl(self, node);
            return;
        }
        for item in &node.items {
            let ImplItem::Fn(item_fn) = item else {
                continue;
            };
            if item_fn.sig.receiver().is_none() && item_fn.sig.ident == self.function_name {
                self.push_return_type_name(&item_fn.sig.output);
            }
        }
        visit::visit_item_impl(self, node);
    }
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
