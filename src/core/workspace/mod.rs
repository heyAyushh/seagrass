use {
    crate::{
        definition_bridge::{self, BridgeSymbol},
        document::{AccountConstraint, InstructionAttributeArgument, ParsedDocument},
        document_stub::DocumentStub,
    },
    std::{
        collections::{HashMap, HashSet},
        fs,
        sync::Arc,
    },
    tower_lsp::lsp_types::{Location, SymbolInformation, SymbolKind, Url},
};

mod associated_values;
mod files;
mod function_returns;
mod indexing;
mod instruction_arguments;
mod sorting;
mod type_names;

pub use associated_values::WorkspaceAssociatedValue;

use {
    files::anchor_rust_files,
    indexing::{
        document_indexed_references, document_indexed_symbols, indexed_accounts_structs,
        indexed_functions, IndexedAccountsStruct, IndexedDocument, IndexedFunction,
        IndexedFunctionEntry, IndexedReference, IndexedReferenceEntry, IndexedSymbol,
        IndexedSymbolEntry,
    },
    instruction_arguments::{
        instruction_argument_names_match, instruction_argument_ranges_in_constraint,
        push_unique_location,
    },
    type_names::primary_type_name,
};

#[cfg(test)]
use files::is_anchor_source_file;

/// Zero-cost shared key for symbol names (stacc performance + ownership).
/// Replaces `String` keys in hot index maps to eliminate per-request clones.
pub type SymbolName = Arc<str>;
const ACCOUNT_DATA_CONTAINER: &str = "#[account]";

#[derive(Debug, Default, Clone)]
pub struct WorkspaceIndex {
    documents: HashMap<Url, IndexedDocument>,
    symbols_by_name: HashMap<SymbolName, Vec<IndexedSymbolEntry>>,
    references_by_name: HashMap<SymbolName, Vec<IndexedReferenceEntry>>,
    functions_by_name: HashMap<SymbolName, Vec<IndexedFunctionEntry>>,
    functions_by_context: HashMap<SymbolName, Vec<IndexedFunctionEntry>>,
    accounts_by_name: HashMap<SymbolName, Vec<IndexedAccountsStruct>>,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceDocumentUpdate {
    uri: Url,
    is_open: bool,
    stub: DocumentStub,
    symbols: Vec<IndexedSymbol>,
    references: Vec<IndexedReference>,
    functions: Vec<IndexedFunction>,
    accounts_structs: Vec<IndexedAccountsStruct>,
}

impl WorkspaceDocumentUpdate {
    pub(crate) fn from_parsed_open_document(uri: Url, document: &ParsedDocument) -> Self {
        Self {
            is_open: true,
            stub: DocumentStub::from_parsed(document),
            symbols: document_indexed_symbols(document),
            references: document_indexed_references(document),
            functions: indexed_functions(document),
            accounts_structs: indexed_accounts_structs(document, &uri, true),
            uri,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceFieldInfo {
    pub location: Location,
    pub type_display: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceResolvedField {
    pub container_name: String,
    pub field_name: String,
    pub field_info: WorkspaceFieldInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceContextField {
    pub name: String,
    pub type_name: Option<String>,
    pub type_display: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceAccountsStruct {
    pub name: String,
    pub fields: Vec<WorkspaceAccountField>,
    pub instruction_arguments: Vec<InstructionAttributeArgument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceInstructionSummary {
    pub name: String,
    pub location: Location,
    pub is_open: bool,
}

#[derive(Debug, Clone)]
pub struct WorkspaceAccountField {
    pub name: String,
    pub type_name: Option<String>,
    pub generic_type_names: Vec<String>,
    pub is_optional: bool,
    pub account_constraints: Vec<AccountConstraint>,
}

macro_rules! prune_uri_entries {
    // Maps where entries carry `location.uri`
    ($self:expr, $map:ident, location, $uri:expr) => {
        $self
            .$map
            .values_mut()
            .for_each(|entries| entries.retain(|entry| entry.location.uri != *$uri));
        $self.$map.retain(|_, entries| !entries.is_empty());
    };
    // Maps where entries carry their own `uri` field
    ($self:expr, $map:ident, direct, $uri:expr) => {
        $self
            .$map
            .values_mut()
            .for_each(|entries| entries.retain(|entry| entry.uri != *$uri));
        $self.$map.retain(|_, entries| !entries.is_empty());
    };
}

impl WorkspaceIndex {
    pub fn build(roots: &[Url], open_documents: impl IntoIterator<Item = (Url, String)>) -> Self {
        let mut index = Self::default();

        // Root files: try wincode cache first (fresh via mtime); else parse+save.
        for root in roots {
            if let Ok(path) = root.to_file_path() {
                for file in anchor_rust_files(&path) {
                    let Ok(uri) = Url::from_file_path(&file) else {
                        continue;
                    };
                    let Ok(source) = fs::read_to_string(&file) else {
                        continue;
                    };
                    let document = ParsedDocument::parse_or_empty(source);
                    let stub = DocumentStub::from_parsed(&document);
                    let _ = stub.save_to_cache(&uri);
                    index.insert_parsed_document(uri, &document, false, stub);
                }
            }
        }

        for (uri, source) in open_documents {
            let document = ParsedDocument::parse_or_empty(source.clone());
            let stub = DocumentStub::from_parsed(&document);
            let _ = stub.save_to_cache(&uri);
            index.insert_parsed_document(uri, &document, true, stub);
        }

        index.insert_bridge_symbols(definition_bridge::collect(roots));

        index
    }

    #[cfg(test)]
    pub fn upsert_parsed_open_document(&mut self, uri: Url, document: &ParsedDocument) {
        self.upsert_open_document_update(WorkspaceDocumentUpdate::from_parsed_open_document(
            uri, document,
        ));
    }

    pub(crate) fn upsert_open_document_update(&mut self, update: WorkspaceDocumentUpdate) {
        // Keep per-keystroke updates in-memory only. Persisting on every edit
        // adds synchronous filesystem overhead to the completion/diagnostics path.
        self.insert_indexed_document(update);
    }

    pub fn indexed_file_count(&self) -> usize {
        self.documents.len()
    }

    #[allow(dead_code)]
    pub fn stub_for(&self, uri: &Url) -> Option<&DocumentStub> {
        self.documents.get(uri).map(|d| &d.stub)
    }

    pub fn symbol_locations(&self, name: &str) -> Vec<Location> {
        self.symbols_by_name
            .get(name)
            .map(|entries| entries.iter().map(|entry| entry.location.clone()).collect())
            .unwrap_or_default()
    }

    pub fn symbol_locations_with_kinds(&self, name: &str, kinds: &[SymbolKind]) -> Vec<Location> {
        self.symbols_by_name
            .get(name)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| kinds.contains(&entry.kind))
                    .map(|entry| entry.location.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn symbol_locations_in_container(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        container_name: &str,
    ) -> Vec<Location> {
        self.symbol_entries_in_container(name, kinds, container_name)
            .map(|entry| entry.location.clone())
            .collect()
    }

    pub fn field_info_in_container(
        &self,
        name: &str,
        container_name: &str,
    ) -> Option<WorkspaceFieldInfo> {
        self.symbol_entries_in_container(name, &[SymbolKind::FIELD], container_name)
            .next()
            .map(|entry| WorkspaceFieldInfo {
                location: entry.location.clone(),
                type_display: entry.type_display.clone(),
            })
    }

    pub fn resolve_account_field_path(
        &self,
        context_name: &str,
        segments: &[String],
        segment_index: usize,
    ) -> Option<WorkspaceResolvedField> {
        let mut container_name = context_name.to_string();
        for (index, segment) in segments.iter().enumerate() {
            let field_info = self.field_info_in_container(segment, &container_name)?;
            if index == segment_index {
                return Some(WorkspaceResolvedField {
                    container_name,
                    field_name: segment.clone(),
                    field_info,
                });
            }
            let next_container = self
                .accounts_struct(&container_name)?
                .fields
                .iter()
                .find(|field| field.name == *segment)?
                .type_name
                .clone()?;
            self.accounts_struct(&next_container)?;
            container_name = next_container;
        }
        None
    }

    pub fn field_names_in_container(&self, container_name: &str) -> Vec<String> {
        let mut names = self
            .symbols_by_name
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|entry| {
                entry.kind == SymbolKind::FIELD
                    && entry.container_name.as_deref() == Some(container_name)
            })
            .map(|entry| entry.name.to_string())
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }

    pub fn account_context_fields(&self, container_name: &str) -> Vec<WorkspaceContextField> {
        if !self.has_unambiguous_account_context(container_name) {
            return Vec::new();
        }

        let mut fields = self
            .symbols_by_name
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|entry| {
                entry.kind == SymbolKind::FIELD
                    && entry.container_name.as_deref() == Some(container_name)
            })
            .map(|entry| WorkspaceContextField {
                name: entry.name.to_string(),
                type_name: entry
                    .type_display
                    .as_deref()
                    .and_then(primary_type_name)
                    .map(str::to_string),
                type_display: entry.type_display.clone(),
            })
            .collect::<Vec<_>>();
        fields.sort_by(|left, right| left.name.cmp(&right.name));
        fields.dedup_by(|left, right| left.name == right.name);
        fields
    }

    fn has_unambiguous_account_context(&self, container_name: &str) -> bool {
        let Some(entries) = self.symbols_by_name.get(container_name) else {
            return true;
        };
        entries
            .iter()
            .filter(|entry| {
                entry.kind == SymbolKind::STRUCT
                    && entry.container_name.as_deref() == Some(ACCOUNT_DATA_CONTAINER)
            })
            .take(2)
            .count()
            <= 1
    }

    pub fn accounts_struct(&self, name: &str) -> Option<&WorkspaceAccountsStruct> {
        self.accounts_by_name
            .get(name)?
            .first()
            .map(|entry| &entry.accounts)
    }

    pub fn accounts_struct_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .accounts_by_name
            .keys()
            .map(|s| s.to_string())
            .collect();
        names.sort();
        names
    }

    pub fn account_data_struct_names(&self) -> Vec<String> {
        let mut names = self
            .symbols_by_name
            .iter()
            .filter(|(_, entries)| {
                entries.iter().any(|symbol| {
                    symbol.kind == SymbolKind::STRUCT
                        && symbol.container_name.as_deref() == Some("#[account]")
                })
            })
            .map(|(name, _)| name.to_string())
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    pub fn references_with_kinds(&self, name: &str, kinds: &[SymbolKind]) -> Vec<Location> {
        self.references_by_name
            .get(name)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| kinds.contains(&entry.kind))
                    .map(|entry| entry.location.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn references_in_container(
        &self,
        name: &str,
        kinds: &[SymbolKind],
        container_name: &str,
    ) -> Vec<Location> {
        self.references_by_name
            .get(name)
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| {
                        kinds.contains(&entry.kind)
                            && entry.container_name.as_deref() == Some(container_name)
                    })
                    .map(|entry| entry.location.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn anchor_type_references(&self, name: &str) -> Vec<Location> {
        let kinds = [SymbolKind::STRUCT];
        if self.symbol_locations_with_kinds(name, &kinds).is_empty() {
            return Vec::new();
        }

        self.references_with_kinds(name, &kinds)
    }

    pub fn function_references(&self, name: &str) -> Vec<Location> {
        let kinds = [SymbolKind::FUNCTION];
        if self.symbol_locations_with_kinds(name, &kinds).is_empty() {
            return Vec::new();
        }

        self.references_with_kinds(name, &kinds)
    }

    pub fn implementation_locations_for_context(&self, context_name: &str) -> Vec<Location> {
        self.functions_by_context
            .get(context_name)
            .into_iter()
            .flat_map(|functions| functions.iter())
            .filter(|function| function.is_program_instruction)
            .map(|function| function.location.clone())
            .collect()
    }

    pub fn program_instructions_for_context(
        &self,
        context_name: &str,
    ) -> Vec<WorkspaceInstructionSummary> {
        self.functions_by_context
            .get(context_name)
            .into_iter()
            .flat_map(|functions| functions.iter())
            .filter(|function| function.is_program_instruction)
            .map(|function| WorkspaceInstructionSummary {
                name: function.name.clone(),
                location: function.location.clone(),
                is_open: function.is_open,
            })
            .collect()
    }

    pub fn reachable_function_names_for_context(
        &self,
        context_name: &str,
    ) -> Option<HashSet<String>> {
        let functions = self.functions_by_context.get(context_name)?;
        let mut reachable = functions
            .iter()
            .filter(|function| function.is_program_instruction)
            .map(|function| function.name.clone())
            .collect::<HashSet<_>>();
        if reachable.is_empty() {
            return None;
        }

        let mut changed = true;
        while changed {
            changed = false;
            let called_names = functions
                .iter()
                .filter(|function| reachable.contains(&function.name))
                .flat_map(|function| function.calls.iter())
                .cloned()
                .collect::<HashSet<_>>();
            for function in functions {
                if !function.is_program_instruction
                    && called_names.contains(&function.name)
                    && reachable.insert(function.name.clone())
                {
                    changed = true;
                }
            }
        }

        Some(reachable)
    }

    pub fn reachable_cpi_program_usage_names_for_context(
        &self,
        context_name: &str,
    ) -> Option<HashSet<String>> {
        self.reachable_account_usage_names_for_context(context_name, |function| {
            &function.cpi_program_usages
        })
    }

    pub fn reachable_signer_usage_names_for_context(
        &self,
        context_name: &str,
    ) -> Option<HashSet<String>> {
        self.reachable_account_usage_names_for_context(context_name, |function| {
            &function.signer_usages
        })
    }

    pub fn reachable_signer_check_names_for_context(
        &self,
        context_name: &str,
    ) -> Option<HashSet<String>> {
        self.reachable_account_usage_names_for_context(context_name, |function| {
            &function.signer_checks
        })
    }

    fn reachable_account_usage_names_for_context(
        &self,
        context_name: &str,
        usages: impl Fn(&IndexedFunctionEntry) -> &[crate::document::AccountUsage],
    ) -> Option<HashSet<String>> {
        let reachable = self.reachable_function_names_for_context(context_name)?;
        let functions = self.functions_by_context.get(context_name)?;
        Some(
            functions
                .iter()
                .filter(|function| reachable.contains(&function.name))
                .flat_map(usages)
                .map(|usage| usage.name.clone())
                .collect(),
        )
    }

    pub fn has_program_instruction_for_context(&self, context_name: &str) -> bool {
        self.functions_by_context
            .get(context_name)
            .is_some_and(|functions| {
                functions
                    .iter()
                    .any(|function| function.is_program_instruction)
            })
    }

    pub fn instruction_argument_names_for_context(&self, context_name: &str) -> HashSet<String> {
        self.functions_by_context
            .get(context_name)
            .into_iter()
            .flat_map(|functions| functions.iter())
            .filter(|function| function.is_program_instruction)
            .flat_map(|function| {
                function
                    .arguments
                    .iter()
                    .map(|argument| argument.name.clone())
            })
            .collect()
    }

    pub fn instruction_argument_type_for_context(
        &self,
        context_name: &str,
        argument_name: &str,
    ) -> Option<String> {
        self.functions_by_context
            .get(context_name)?
            .iter()
            .filter(|function| function.is_program_instruction)
            .flat_map(|function| function.arguments.iter())
            .find(|argument| instruction_argument_names_match(&argument.name, argument_name))
            .and_then(|argument| argument.type_name.clone())
    }

    pub fn instruction_argument_references_for_context_with_source(
        &self,
        context_name: &str,
        argument_name: &str,
        source_for_uri: impl Fn(&Url) -> Option<String>,
    ) -> Vec<Location> {
        let mut locations = Vec::new();

        if let Some(functions) = self.functions_by_context.get(context_name) {
            for function in functions
                .iter()
                .filter(|function| function.is_program_instruction)
            {
                for argument in &function.arguments {
                    if instruction_argument_names_match(&argument.name, argument_name) {
                        push_unique_location(
                            &mut locations,
                            Location {
                                uri: function.uri.clone(),
                                range: argument.range,
                            },
                        );
                    }
                }
            }
        }

        if let Some(accounts_entries) = self.accounts_by_name.get(context_name) {
            for accounts_entry in accounts_entries {
                for argument in &accounts_entry.accounts.instruction_arguments {
                    if instruction_argument_names_match(&argument.name, argument_name) {
                        push_unique_location(
                            &mut locations,
                            Location {
                                uri: accounts_entry.uri.clone(),
                                range: argument.range,
                            },
                        );
                    }
                }

                if let Some(source) = source_for_uri(&accounts_entry.uri) {
                    for field in &accounts_entry.accounts.fields {
                        for constraint in &field.account_constraints {
                            for range in instruction_argument_ranges_in_constraint(
                                &source,
                                constraint,
                                argument_name,
                            ) {
                                push_unique_location(
                                    &mut locations,
                                    Location {
                                        uri: accounts_entry.uri.clone(),
                                        range,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        locations
    }

    pub fn workspace_symbols(&self, query: &str) -> Vec<SymbolInformation> {
        let query = query.to_lowercase();
        self.symbols_by_name
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|entry| query.is_empty() || entry.name.to_lowercase().contains(&query))
            .map(|entry| {
                #[allow(deprecated)]
                SymbolInformation {
                    name: entry.name.clone(),
                    kind: entry.kind,
                    tags: None,
                    deprecated: None,
                    location: entry.location.clone(),
                    container_name: entry.container_name.clone(),
                }
            })
            .collect()
    }

    fn insert_parsed_document(
        &mut self,
        uri: Url,
        document: &ParsedDocument,
        is_open: bool,
        stub: DocumentStub,
    ) {
        let accounts_structs = indexed_accounts_structs(document, &uri, is_open);
        let functions = indexed_functions(document);
        let references = document_indexed_references(document);
        let symbols = document_indexed_symbols(document);
        self.insert_indexed_document(WorkspaceDocumentUpdate {
            uri,
            is_open,
            stub,
            symbols,
            references,
            functions,
            accounts_structs,
        });
    }

    fn insert_indexed_document(&mut self, update: WorkspaceDocumentUpdate) {
        let WorkspaceDocumentUpdate {
            uri,
            is_open,
            stub,
            symbols,
            references,
            functions,
            accounts_structs,
        } = update;

        self.remove_index_entries_for_uri(&uri);
        self.index_document_parts(
            &uri,
            is_open,
            symbols,
            references,
            functions,
            accounts_structs,
        );
        self.documents.insert(uri.clone(), IndexedDocument { stub });
    }

    fn remove_index_entries_for_uri(&mut self, uri: &Url) {
        self.documents.remove(uri);
        prune_uri_entries!(self, symbols_by_name, location, uri);
        prune_uri_entries!(self, references_by_name, location, uri);
        prune_uri_entries!(self, functions_by_name, direct, uri);
        prune_uri_entries!(self, functions_by_context, direct, uri);
        prune_uri_entries!(self, accounts_by_name, direct, uri);
    }

    fn index_document_parts(
        &mut self,
        uri: &Url,
        is_open: bool,
        symbols: Vec<IndexedSymbol>,
        references: Vec<IndexedReference>,
        functions: Vec<IndexedFunction>,
        accounts_structs: Vec<IndexedAccountsStruct>,
    ) {
        // Pre-allocate based on typical sizes for better performance (Pass 4)
        self.symbols_by_name.reserve(symbols.len());
        self.references_by_name.reserve(references.len());
        self.functions_by_name.reserve(functions.len());
        self.functions_by_context.reserve(functions.len());
        self.accounts_by_name.reserve(accounts_structs.len());

        for symbol in &symbols {
            // Use Arc<str> key for zero-cost clones in hot lookup paths (Pass 4)
            self.symbols_by_name
                .entry(Arc::from(symbol.name.as_str()))
                .or_default()
                .push(IndexedSymbolEntry::from_symbol(uri, is_open, symbol));
        }
        for reference in &references {
            self.references_by_name
                .entry(Arc::from(reference.name.as_str()))
                .or_default()
                .push(IndexedReferenceEntry::from_reference(
                    uri, is_open, reference,
                ));
        }
        for function in &functions {
            let entry = IndexedFunctionEntry::from_function(uri, is_open, function);
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
        self.sort_open_entries_first();
    }

    fn insert_bridge_symbols(&mut self, symbols: Vec<BridgeSymbol>) {
        self.symbols_by_name.reserve(symbols.len());
        for symbol in symbols {
            self.symbols_by_name
                .entry(Arc::from(symbol.name.as_str()))
                .or_default()
                .push(IndexedSymbolEntry::from_bridge_symbol(symbol));
        }
        self.sort_open_entries_first();
    }

    fn symbol_entries_in_container<'a>(
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
}

#[cfg(test)]
mod tests;
