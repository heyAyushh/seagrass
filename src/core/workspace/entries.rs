use super::*;

impl WorkspaceIndex {
    pub(super) fn insert_parsed_document(
        &mut self,
        uri: Url,
        document: &ParsedDocument,
        is_open: bool,
    ) {
        let accounts_structs = indexed_accounts_structs(document, &uri, is_open);
        let account_data_structs = indexed_account_data_structs(document, &uri, is_open);
        let functions = indexed_functions(document);
        let references = document_indexed_references(document);
        let symbols = document_indexed_symbols(document);
        self.insert_indexed_document(WorkspaceDocumentUpdate {
            uri,
            is_open,
            symbols,
            references,
            functions,
            accounts_structs,
            account_data_structs,
        });
    }

    pub(super) fn insert_indexed_document(&mut self, update: WorkspaceDocumentUpdate) {
        let uri = update.uri.clone();
        self.remove_index_entries_for_uri(&uri);
        self.record_module_path_for_root(&uri);
        self.index_document_parts(update);
        self.documents.insert(uri);
    }

    pub(super) fn remove_index_entries_for_uri(&mut self, uri: &Url) {
        self.documents.remove(uri);
        self.remove_module_path_for_root(uri);
        prune_uri_entries!(self, symbols_by_name, location, uri);
        prune_uri_entries!(self, references_by_name, location, uri);
        prune_uri_entries!(self, functions_by_name, direct, uri);
        prune_uri_entries!(self, functions_by_context, direct, uri);
        prune_uri_entries!(self, accounts_by_name, direct, uri);
        prune_uri_entries!(self, account_data_by_name, direct, uri);
        self.rebuild_call_graph();
    }

    pub(super) fn index_document_parts(&mut self, update: WorkspaceDocumentUpdate) {
        let WorkspaceDocumentUpdate {
            uri,
            is_open,
            symbols,
            references,
            functions,
            accounts_structs,
            account_data_structs,
        } = update;
        // Pre-allocate based on typical sizes for better performance (Pass 4)
        self.symbols_by_name.reserve(symbols.len());
        self.references_by_name.reserve(references.len());
        self.functions_by_name.reserve(functions.len());
        self.functions_by_context.reserve(functions.len());
        self.accounts_by_name.reserve(accounts_structs.len());
        self.account_data_by_name
            .reserve(account_data_structs.len());

        for symbol in &symbols {
            // Use Arc<str> key for zero-cost clones in hot lookup paths (Pass 4)
            self.symbols_by_name
                .entry(Arc::from(symbol.name.as_str()))
                .or_default()
                .push(IndexedSymbolEntry::from_symbol(&uri, is_open, symbol));
        }
        for reference in &references {
            self.references_by_name
                .entry(Arc::from(reference.name.as_str()))
                .or_default()
                .push(IndexedReferenceEntry::from_reference(
                    &uri, is_open, reference,
                ));
        }
        for function in &functions {
            let entry = IndexedFunctionEntry::from_function(&uri, is_open, function);
            self.functions_by_name
                .entry(Arc::from(function.name.as_str()))
                .or_default()
                .push(entry.clone());
            let Some(context_name) = function.context_name.clone() else {
                continue;
            };
            self.functions_by_context
                .entry(Arc::from(context_name.as_str()))
                .or_default()
                .push(entry);
        }
        for accounts in &accounts_structs {
            self.accounts_by_name
                .entry(Arc::from(accounts.accounts.name.as_str()))
                .or_default()
                .push(accounts.clone());
        }
        for account_data in &account_data_structs {
            self.account_data_by_name
                .entry(Arc::from(account_data.symbol.name.as_str()))
                .or_default()
                .push(account_data.clone());
        }
        self.sort_open_entries_first();
        self.rebuild_call_graph();
    }

    pub(super) fn insert_bridge_symbols(&mut self, symbols: Vec<BridgeSymbol>) {
        self.symbols_by_name.reserve(symbols.len());
        for symbol in symbols {
            self.symbols_by_name
                .entry(Arc::from(symbol.name.as_str()))
                .or_default()
                .push(IndexedSymbolEntry::from_bridge_symbol(symbol));
        }
        self.sort_open_entries_first();
    }

    pub(super) fn symbol_entries_in_container<'a>(
        &'a self,
        name: &str,
        kinds: &'a [SymbolKind],
        container_name: &'a str,
    ) -> impl Iterator<Item = &'a IndexedSymbolEntry> {
        self.symbols_by_name
            .get(name)
            .into_iter()
            .flat_map(|entries| entries.iter())
            .filter(move |entry| {
                kinds.contains(&entry.kind)
                    && entry.container_name.as_deref() == Some(container_name)
            })
    }

    pub(super) fn function_entries_for_reachable_names(
        &self,
        names: &HashSet<String>,
    ) -> Vec<&IndexedFunctionEntry> {
        let mut entries = names
            .iter()
            .flat_map(|name| self.functions_by_name.get(name.as_str()))
            .flat_map(|entries| entries.iter())
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.uri.as_str().cmp(right.uri.as_str()))
                .then_with(|| {
                    left.location
                        .range
                        .start
                        .line
                        .cmp(&right.location.range.start.line)
                })
                .then_with(|| {
                    left.location
                        .range
                        .start
                        .character
                        .cmp(&right.location.range.start.character)
                })
        });
        entries
    }

    pub(super) fn rebuild_call_graph(&mut self) {
        self.call_graph = CallGraph::build(
            self.functions_by_name
                .values()
                .flat_map(|entries| entries.iter()),
        );
    }
}
