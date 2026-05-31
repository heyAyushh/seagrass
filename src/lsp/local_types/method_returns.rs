use {
    super::{expression_type_name_with_context_scope, type_names},
    crate::{document::ParsedDocument, workspace::WorkspaceIndex},
    syn::{
        visit::{self, Visit},
        Expr, ExprMethodCall, ImplItem, ItemImpl, ReturnType, Type,
    },
};

#[derive(Clone, Copy)]
enum ReturnMode {
    Direct,
    TryUnwrap,
}

pub(super) fn method_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    method_call: &ExprMethodCall,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let receiver_type = expression_type_name_with_context_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
    )?;
    local_method_return_type_name(
        document,
        &receiver_type,
        &method_call.method.to_string(),
        ReturnMode::Direct,
    )
    .or_else(|| {
        workspace_method_return_type_name(
            workspace_index,
            &receiver_type,
            &method_call.method.to_string(),
            ReturnMode::Direct,
        )
    })
}

pub(super) fn method_return_type_name_for_receiver_type(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    method_name: &str,
) -> Option<String> {
    local_method_return_type_name(document, receiver_type, method_name, ReturnMode::Direct).or_else(
        || {
            workspace_method_return_type_name(
                workspace_index,
                receiver_type,
                method_name,
                ReturnMode::Direct,
            )
        },
    )
}

pub(super) fn try_method_return_type_name(
    document: &ParsedDocument,
    workspace_index: Option<&WorkspaceIndex>,
    expr: &Expr,
    scope_type_name: &impl Fn(&str) -> Option<String>,
    context_type_name: &impl Fn(&str) -> Option<String>,
) -> Option<String> {
    let Expr::MethodCall(method_call) = expr else {
        return None;
    };
    let receiver_type = expression_type_name_with_context_scope(
        document,
        workspace_index,
        &method_call.receiver,
        scope_type_name,
        context_type_name,
    )?;
    local_method_return_type_name(
        document,
        &receiver_type,
        &method_call.method.to_string(),
        ReturnMode::TryUnwrap,
    )
    .or_else(|| {
        workspace_method_return_type_name(
            workspace_index,
            &receiver_type,
            &method_call.method.to_string(),
            ReturnMode::TryUnwrap,
        )
    })
}

fn local_method_return_type_name(
    document: &ParsedDocument,
    receiver_type: &str,
    method_name: &str,
    mode: ReturnMode,
) -> Option<String> {
    let mut visitor = LocalMethodReturnVisitor {
        receiver_type,
        method_name,
        mode,
        return_type_names: Vec::new(),
    };
    visitor.visit_file(document.syntax());
    visitor.unique_return_type_name()
}

fn workspace_method_return_type_name(
    workspace_index: Option<&WorkspaceIndex>,
    receiver_type: &str,
    method_name: &str,
    mode: ReturnMode,
) -> Option<String> {
    let mut return_type_names = workspace_index?
        .associated_values_in_container(receiver_type)
        .into_iter()
        .filter(|value| value.kind == tower_lsp::lsp_types::SymbolKind::METHOD)
        .filter(|value| value.name == method_name)
        .filter_map(|value| value.type_display)
        .filter_map(|display| match mode {
            ReturnMode::Direct => type_names::return_type_name_from_text(&display),
            ReturnMode::TryUnwrap => type_names::try_return_type_name_from_text(&display),
        })
        .map(|type_name| resolve_self_type(type_name, receiver_type))
        .collect::<Vec<_>>();
    return_type_names.sort();
    return_type_names.dedup();
    (return_type_names.len() == 1).then(|| return_type_names.remove(0))
}

struct LocalMethodReturnVisitor<'a> {
    receiver_type: &'a str,
    method_name: &'a str,
    mode: ReturnMode,
    return_type_names: Vec<String>,
}

impl LocalMethodReturnVisitor<'_> {
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
            self.return_type_names
                .push(resolve_self_type(type_name, self.receiver_type));
        }
    }
}

impl<'ast> Visit<'ast> for LocalMethodReturnVisitor<'_> {
    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        if node.trait_.is_some() || impl_self_type_name(node).as_deref() != Some(self.receiver_type)
        {
            visit::visit_item_impl(self, node);
            return;
        }
        for item in &node.items {
            let ImplItem::Fn(item_fn) = item else {
                continue;
            };
            if item_fn.sig.receiver().is_some() && item_fn.sig.ident == self.method_name {
                self.push_return_type_name(&item_fn.sig.output);
            }
        }
        visit::visit_item_impl(self, node);
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

fn resolve_self_type(type_name: String, self_type: &str) -> String {
    if type_name == "Self" {
        self_type.to_string()
    } else {
        type_name
    }
}
