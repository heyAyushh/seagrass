use {
    crate::{
        document::ParsedDocument,
        lsp::{local_types, scope::pattern_binding_name},
        workspace::WorkspaceIndex,
    },
    std::collections::HashMap,
};

#[derive(Default)]
pub(super) struct TypedScopeStack {
    scopes: Vec<HashMap<String, String>>,
    iterable_scopes: Vec<HashMap<String, String>>,
    context_scopes: Vec<HashMap<String, String>>,
}

impl TypedScopeStack {
    pub(super) fn push(&mut self) {
        self.scopes.push(HashMap::new());
        self.iterable_scopes.push(HashMap::new());
        self.context_scopes.push(HashMap::new());
    }

    pub(super) fn pop(&mut self) {
        self.scopes.pop();
        self.iterable_scopes.pop();
        self.context_scopes.pop();
    }

    pub(super) fn declare_context_pat(&mut self, pat: &syn::Pat, type_name: String) {
        let Some(name) = pattern_binding_name(pat) else {
            return;
        };
        if let Some(scope) = self.context_scopes.last_mut() {
            scope.insert(name, type_name);
        }
    }

    pub(super) fn declare_typed_pattern(
        &mut self,
        document: &ParsedDocument,
        workspace_index: Option<&WorkspaceIndex>,
        pat: &syn::Pat,
        type_name: &str,
    ) {
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        for value in local_types::typed_pattern_bindings(document, workspace_index, pat, type_name)
        {
            scope.insert(value.name, value.type_name);
        }
    }

    pub(super) fn declare_typed_name(&mut self, name: &str, type_name: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), type_name);
        }
    }

    pub(super) fn declare_iterable_pat(&mut self, pat: &syn::Pat, type_name: String) {
        let Some(name) = pattern_binding_name(pat) else {
            return;
        };
        self.declare_iterable_name(&name, type_name);
    }

    pub(super) fn declare_iterable_name(&mut self, name: &str, type_name: String) {
        if let Some(scope) = self.iterable_scopes.last_mut() {
            scope.insert(name.to_string(), type_name);
        }
    }

    pub(super) fn get(&self, name: &str) -> Option<String> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(super) fn get_context(&self, name: &str) -> Option<String> {
        self.context_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(super) fn get_iterable_item(&self, name: &str) -> Option<String> {
        self.iterable_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}
