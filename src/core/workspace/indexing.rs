use {
    super::{WorkspaceAccountField, WorkspaceAccountsStruct},
    crate::{
        anchor::idioms,
        definition_bridge::BridgeSymbol,
        document::{
            document_symbols, AccountUsage, AssociatedValueKind, InstructionSymbol, ParsedDocument,
        },
        navigation::{account_field_path_definition_target_for_position, AccountPathPosition},
        range::range_from_span,
    },
    quote::ToTokens,
    std::collections::HashSet,
    syn::{visit::Visit, ItemFn},
    tower_lsp::lsp_types::{DocumentSymbol, Location, Range, SymbolKind, Url},
};

pub(super) fn document_indexed_symbols(document: &ParsedDocument) -> Vec<IndexedSymbol> {
    let symbols = indexed_symbols(document);
    if symbols.is_empty() {
        indexed_document_symbols(&document_symbols(document))
    } else {
        symbols
    }
}

pub(super) fn document_indexed_references(document: &ParsedDocument) -> Vec<IndexedReference> {
    let references = indexed_references(document);
    if references.is_empty() {
        indexed_tree_sitter_references(document, &document_indexed_symbols(document))
    } else {
        references
    }
}

#[derive(Debug, Clone)]
pub(super) struct IndexedAccountsStruct {
    pub(super) uri: Url,
    pub(super) is_open: bool,
    pub(super) accounts: WorkspaceAccountsStruct,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedSymbol {
    pub(super) name: String,
    pub(super) kind: SymbolKind,
    pub(super) selection_range: Range,
    pub(super) container_name: Option<String>,
    pub(super) type_display: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedReference {
    pub(super) name: String,
    pub(super) kind: SymbolKind,
    pub(super) range: Range,
    pub(super) container_name: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedSymbolEntry {
    pub(super) name: String,
    pub(super) kind: SymbolKind,
    pub(super) container_name: Option<String>,
    pub(super) type_display: Option<String>,
    pub(super) location: Location,
    pub(super) is_open: bool,
}

impl IndexedSymbolEntry {
    pub(super) fn from_symbol(uri: &Url, is_open: bool, symbol: &IndexedSymbol) -> Self {
        Self {
            name: symbol.name.clone(),
            kind: symbol.kind,
            container_name: symbol.container_name.clone(),
            type_display: symbol.type_display.clone(),
            location: Location {
                uri: uri.clone(),
                range: symbol.selection_range,
            },
            is_open,
        }
    }

    pub(super) fn from_bridge_symbol(symbol: BridgeSymbol) -> Self {
        Self {
            name: symbol.name,
            kind: symbol.kind,
            container_name: symbol.container_name,
            type_display: symbol.type_display,
            location: symbol.location,
            is_open: false,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct IndexedReferenceEntry {
    pub(super) kind: SymbolKind,
    pub(super) container_name: Option<String>,
    pub(super) location: Location,
    pub(super) is_open: bool,
}

impl IndexedReferenceEntry {
    pub(super) fn from_reference(uri: &Url, is_open: bool, reference: &IndexedReference) -> Self {
        Self {
            kind: reference.kind,
            container_name: reference.container_name.clone(),
            location: Location {
                uri: uri.clone(),
                range: reference.range,
            },
            is_open,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct IndexedFunction {
    pub(super) name: String,
    pub(super) context_name: Option<String>,
    pub(super) return_type_display: Option<String>,
    pub(super) selection_range: Range,
    pub(super) is_program_instruction: bool,
    pub(super) calls: Vec<String>,
    pub(super) cpi_program_usages: Vec<AccountUsage>,
    pub(super) signer_usages: Vec<AccountUsage>,
    pub(super) signer_checks: Vec<AccountUsage>,
    pub(super) arguments: Vec<IndexedFunctionArgument>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedFunctionArgument {
    pub(super) name: String,
    pub(super) range: Range,
    pub(super) type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct IndexedFunctionEntry {
    pub(super) uri: Url,
    pub(super) is_open: bool,
    pub(super) name: String,
    pub(super) return_type_display: Option<String>,
    pub(super) location: Location,
    pub(super) is_program_instruction: bool,
    pub(super) calls: Vec<String>,
    pub(super) cpi_program_usages: Vec<AccountUsage>,
    pub(super) signer_usages: Vec<AccountUsage>,
    pub(super) signer_checks: Vec<AccountUsage>,
    pub(super) arguments: Vec<IndexedFunctionArgument>,
}

impl IndexedFunctionEntry {
    pub(super) fn from_function(uri: &Url, is_open: bool, function: &IndexedFunction) -> Self {
        Self {
            uri: uri.clone(),
            is_open,
            name: function.name.clone(),
            return_type_display: function.return_type_display.clone(),
            location: Location {
                uri: uri.clone(),
                range: function.selection_range,
            },
            is_program_instruction: function.is_program_instruction,
            calls: function.calls.clone(),
            cpi_program_usages: function.cpi_program_usages.clone(),
            signer_usages: function.signer_usages.clone(),
            signer_checks: function.signer_checks.clone(),
            arguments: function.arguments.clone(),
        }
    }
}

pub(super) fn indexed_functions(document: &ParsedDocument) -> Vec<IndexedFunction> {
    document
        .symbols()
        .instructions
        .iter()
        .map(|instruction| indexed_function(document, instruction, true))
        .chain(
            document
                .symbols()
                .functions
                .iter()
                .map(|function| indexed_function(document, function, false)),
        )
        .collect()
}

fn indexed_function(
    document: &ParsedDocument,
    function: &InstructionSymbol,
    is_program_instruction: bool,
) -> IndexedFunction {
    IndexedFunction {
        name: function.name.clone(),
        context_name: function
            .context
            .as_ref()
            .map(|context| context.name.clone()),
        return_type_display: function_return_type_display(document, function.name.as_str()),
        selection_range: function.selection_range,
        is_program_instruction,
        calls: function
            .function_calls
            .iter()
            .map(|call| call.name.clone())
            .collect(),
        cpi_program_usages: function.cpi_program_usages.clone(),
        signer_usages: function.signer_usages.clone(),
        signer_checks: function.signer_checks.clone(),
        arguments: function
            .arguments
            .iter()
            .map(|argument| IndexedFunctionArgument {
                name: argument.name.clone(),
                range: argument.range,
                type_name: argument.type_name.clone(),
            })
            .collect(),
    }
}

fn function_return_type_display(document: &ParsedDocument, function_name: &str) -> Option<String> {
    let mut visitor = FunctionReturnTypeVisitor {
        function_name,
        return_type_displays: Vec::new(),
    };
    visitor.visit_file(document.syntax());
    visitor.unique_return_type_display()
}

struct FunctionReturnTypeVisitor<'a> {
    function_name: &'a str,
    return_type_displays: Vec<String>,
}

impl FunctionReturnTypeVisitor<'_> {
    fn unique_return_type_display(mut self) -> Option<String> {
        self.return_type_displays.sort();
        self.return_type_displays.dedup();
        (self.return_type_displays.len() == 1).then(|| self.return_type_displays.remove(0))
    }
}

impl<'ast> Visit<'ast> for FunctionReturnTypeVisitor<'_> {
    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        if node.sig.ident == self.function_name {
            if let syn::ReturnType::Type(_, ty) = &node.sig.output {
                self.return_type_displays
                    .push(ty.to_token_stream().to_string());
            }
        }
        syn::visit::visit_item_fn(self, node);
    }
}

pub(super) fn indexed_accounts_structs(
    document: &ParsedDocument,
    uri: &Url,
    is_open: bool,
) -> Vec<IndexedAccountsStruct> {
    document
        .symbols()
        .accounts_structs
        .values()
        .map(|accounts| IndexedAccountsStruct {
            uri: uri.clone(),
            is_open,
            accounts: WorkspaceAccountsStruct {
                name: accounts.name.clone(),
                fields: accounts
                    .fields
                    .iter()
                    .map(|field| WorkspaceAccountField {
                        name: field.name.clone(),
                        type_name: field.type_name.clone(),
                        generic_type_names: field.generic_type_names.clone(),
                        is_optional: field.is_optional,
                        account_constraints: field.account_constraints.clone(),
                    })
                    .collect(),
                instruction_arguments: accounts.instruction_arguments.clone(),
            },
        })
        .collect()
}

fn indexed_symbols(document: &ParsedDocument) -> Vec<IndexedSymbol> {
    let mut symbols = Vec::with_capacity(32); // Pre-allocate for typical Anchor program (Pass 4)

    if let Some(declared) = document.symbols().declared_program_id.as_ref() {
        symbols.push(IndexedSymbol {
            name: idioms::DECLARE_ID_MACRO_INVOCATION.to_string(),
            kind: SymbolKind::CONSTANT,
            selection_range: declared.range,
            container_name: Some(declared.value.clone()),
            type_display: None,
        });
    }

    symbols.extend(
        document
            .symbols()
            .instructions
            .iter()
            .flat_map(|instruction| {
                std::iter::once(IndexedSymbol {
                    name: instruction.name.clone(),
                    kind: SymbolKind::FUNCTION,
                    selection_range: instruction.selection_range,
                    container_name: Some("#[program]".to_string()),
                    type_display: None,
                })
                .chain(instruction.arguments.iter().map(|argument| {
                    IndexedSymbol {
                        name: argument.name.clone(),
                        kind: SymbolKind::VARIABLE,
                        selection_range: argument.range,
                        container_name: Some(format!("{} instruction", instruction.name)),
                        type_display: argument.type_name.clone(),
                    }
                }))
            }),
    );
    symbols.extend(document.symbols().functions.iter().flat_map(|function| {
        std::iter::once(IndexedSymbol {
            name: function.name.clone(),
            kind: SymbolKind::FUNCTION,
            selection_range: function.selection_range,
            container_name: function
                .context
                .as_ref()
                .map(|context| format!("Context<{}>", context.name))
                .or_else(|| Some("Anchor helper function".to_string())),
            type_display: None,
        })
        .chain(function.arguments.iter().map(|argument| IndexedSymbol {
            name: argument.name.clone(),
            kind: SymbolKind::VARIABLE,
            selection_range: argument.range,
            container_name: Some(format!("{} function", function.name)),
            type_display: argument.type_name.clone(),
        }))
    }));
    symbols.extend(
        document
            .symbols()
            .accounts_structs
            .values()
            .flat_map(|accounts| {
                std::iter::once(IndexedSymbol {
                    name: accounts.name.clone(),
                    kind: SymbolKind::STRUCT,
                    selection_range: accounts.selection_range,
                    container_name: Some("#[derive(Accounts)]".to_string()),
                    type_display: None,
                })
                .chain(accounts.fields.iter().map(|field| IndexedSymbol {
                    name: field.name.clone(),
                    kind: SymbolKind::FIELD,
                    selection_range: field.selection_range,
                    container_name: Some(accounts.name.clone()),
                    type_display: field_type_display(field),
                }))
            }),
    );
    symbols.extend(
        document
            .symbols()
            .account_data_structs
            .values()
            .flat_map(|account| {
                std::iter::once(IndexedSymbol {
                    name: account.name.clone(),
                    kind: SymbolKind::STRUCT,
                    selection_range: account.selection_range,
                    container_name: Some("#[account]".to_string()),
                    type_display: None,
                })
                .chain(account.fields.iter().map(|field| IndexedSymbol {
                    name: field.name.clone(),
                    kind: SymbolKind::FIELD,
                    selection_range: field.selection_range,
                    container_name: Some(account.name.clone()),
                    type_display: field_type_display(field),
                }))
            }),
    );
    symbols.extend(
        document
            .symbols()
            .all_structs
            .values()
            .filter(|symbol| {
                !document
                    .symbols()
                    .accounts_structs
                    .contains_key(&symbol.name)
                    && !document
                        .symbols()
                        .account_data_structs
                        .contains_key(&symbol.name)
            })
            .flat_map(|symbol| {
                std::iter::once(IndexedSymbol {
                    name: symbol.name.clone(),
                    kind: SymbolKind::STRUCT,
                    selection_range: symbol.selection_range,
                    container_name: Some("struct".to_string()),
                    type_display: None,
                })
                .chain(symbol.fields.iter().map(|field| IndexedSymbol {
                    name: field.name.clone(),
                    kind: SymbolKind::FIELD,
                    selection_range: field.selection_range,
                    container_name: Some(symbol.name.clone()),
                    type_display: field_type_display(field),
                }))
            }),
    );
    symbols.extend(document.symbols().associated_value_items.iter().flat_map(
        |(container_name, items)| {
            items.iter().map(move |item| IndexedSymbol {
                name: item.name.clone(),
                kind: associated_value_symbol_kind(item.kind),
                selection_range: item.range,
                container_name: Some(container_name.clone()),
                type_display: item.type_display.clone(),
            })
        },
    ));

    symbols
}

fn associated_value_symbol_kind(kind: AssociatedValueKind) -> SymbolKind {
    match kind {
        AssociatedValueKind::Constant => SymbolKind::CONSTANT,
        AssociatedValueKind::Function => SymbolKind::FUNCTION,
        AssociatedValueKind::Method => SymbolKind::METHOD,
    }
}

fn indexed_document_symbols(symbols: &[DocumentSymbol]) -> Vec<IndexedSymbol> {
    let mut indexed = Vec::new();
    for symbol in symbols {
        flatten_document_symbol(symbol, None, &mut indexed);
    }
    indexed
}

fn flatten_document_symbol(
    symbol: &DocumentSymbol,
    container_name: Option<String>,
    indexed: &mut Vec<IndexedSymbol>,
) {
    indexed.push(IndexedSymbol {
        name: symbol.name.clone(),
        kind: symbol.kind,
        selection_range: symbol.selection_range,
        container_name: container_name.clone().or_else(|| symbol.detail.clone()),
        type_display: symbol.detail.clone(),
    });

    if let Some(children) = &symbol.children {
        let child_container = Some(symbol.name.clone());
        for child in children {
            flatten_document_symbol(child, child_container.clone(), indexed);
        }
    }
}

fn field_type_display(field: &crate::document::SymbolRange) -> Option<String> {
    let type_name = field.type_name.as_ref()?;
    if field.generic_type_names.is_empty() {
        Some(type_name.clone())
    } else {
        Some(format!(
            "{}<{}>",
            type_name,
            field.generic_type_names.join(", ")
        ))
    }
}

fn indexed_references(document: &ParsedDocument) -> Vec<IndexedReference> {
    let mut references = Vec::new();

    references.extend(
        document
            .symbols()
            .all_structs
            .values()
            .map(|symbol| IndexedReference {
                name: symbol.name.clone(),
                kind: SymbolKind::STRUCT,
                range: symbol.selection_range,
                container_name: None,
            }),
    );
    references.extend(
        document
            .symbols()
            .context_references
            .iter()
            .map(|reference| IndexedReference {
                name: reference.name.clone(),
                kind: SymbolKind::STRUCT,
                range: reference.range,
                container_name: None,
            }),
    );
    references.extend(
        document
            .symbols()
            .accounts_structs
            .values()
            .chain(document.symbols().account_data_structs.values())
            .flat_map(|symbol| symbol.fields.iter())
            .flat_map(|field| {
                field
                    .type_name
                    .iter()
                    .zip(field.type_range)
                    .map(|(name, range)| IndexedReference {
                        name: name.clone(),
                        kind: SymbolKind::STRUCT,
                        range,
                        container_name: None,
                    })
                    .chain(
                        field
                            .generic_type_ranges
                            .iter()
                            .map(|generic| IndexedReference {
                                name: generic.name.clone(),
                                kind: SymbolKind::STRUCT,
                                range: generic.range,
                                container_name: None,
                            }),
                    )
            }),
    );
    references.extend(document.symbols().callable_functions().map(|function| {
        IndexedReference {
            name: function.name.clone(),
            kind: SymbolKind::FUNCTION,
            range: function.selection_range,
            container_name: function
                .context
                .as_ref()
                .map(|context| format!("Context<{}>", context.name)),
        }
    }));
    references.extend(indexed_function_call_references(document));
    references.extend(indexed_account_path_field_references(document));
    references.extend(indexed_account_data_field_references(document));

    references
}

fn indexed_function_call_references(document: &ParsedDocument) -> Vec<IndexedReference> {
    let mut visitor = FunctionCallReferenceVisitor::default();
    visitor.visit_file(document.syntax());
    visitor.references
}

#[derive(Default)]
struct FunctionCallReferenceVisitor {
    references: Vec<IndexedReference>,
}

impl<'ast> Visit<'ast> for FunctionCallReferenceVisitor {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            if let Some(segment) = path.path.segments.last() {
                self.references.push(IndexedReference {
                    name: segment.ident.to_string(),
                    kind: SymbolKind::FUNCTION,
                    range: range_from_span(segment.ident.span()),
                    container_name: path
                        .path
                        .segments
                        .iter()
                        .rev()
                        .nth(1)
                        .map(|segment| segment.ident.to_string()),
                });
            }
        }
        syn::visit::visit_expr_call(self, node);
    }
}

fn indexed_account_data_field_references(document: &ParsedDocument) -> Vec<IndexedReference> {
    document
        .symbols()
        .callable_functions()
        .flat_map(|instruction| {
            let accounts = instruction
                .context
                .as_ref()
                .and_then(|context| document.symbols().accounts_structs.get(&context.name));
            instruction
                .account_data_field_usages
                .iter()
                .filter_map(move |usage| {
                    let account_data_type = accounts
                        .and_then(|accounts| {
                            accounts
                                .fields
                                .iter()
                                .find(|field| field.name == usage.account)
                        })
                        .and_then(|field| field.generic_type_names.last())?;
                    Some(IndexedReference {
                        name: usage.field.clone(),
                        kind: SymbolKind::FIELD,
                        range: usage.range,
                        container_name: Some(account_data_type.clone()),
                    })
                })
        })
        .chain(
            document
                .symbols()
                .account_data_structs
                .values()
                .flat_map(|account| {
                    account.fields.iter().map(|field| IndexedReference {
                        name: field.name.clone(),
                        kind: SymbolKind::FIELD,
                        range: field.selection_range,
                        container_name: Some(account.name.clone()),
                    })
                }),
        )
        .collect()
}

fn indexed_account_path_field_references(document: &ParsedDocument) -> Vec<IndexedReference> {
    document
        .symbols()
        .callable_functions()
        .filter_map(|instruction| {
            instruction
                .context
                .as_ref()
                .map(|context| (instruction, context))
        })
        .flat_map(|(instruction, context)| {
            instruction
                .account_path_usages
                .iter()
                .flat_map(move |usage| {
                    usage
                        .segments
                        .iter()
                        .enumerate()
                        .filter_map(move |(segment_index, segment)| {
                            let path = AccountPathPosition {
                                context: context.name.clone(),
                                segments: usage
                                    .segments
                                    .iter()
                                    .map(|segment| segment.name.clone())
                                    .collect(),
                                segment_index,
                                field: segment.name.clone(),
                            };
                            let target =
                                account_field_path_definition_target_for_position(document, &path)?;
                            Some(IndexedReference {
                                name: target.field,
                                kind: SymbolKind::FIELD,
                                range: segment.range,
                                container_name: Some(target.container),
                            })
                        })
                })
        })
        .collect()
}

fn indexed_tree_sitter_references(
    document: &ParsedDocument,
    symbols: &[IndexedSymbol],
) -> Vec<IndexedReference> {
    let known_types = symbols
        .iter()
        .filter(|symbol| symbol.kind == SymbolKind::STRUCT)
        .map(|symbol| symbol.name.clone())
        .collect::<HashSet<_>>();

    document
        .tree_sitter()
        .map(|syntax| {
            syntax
                .anchor_type_references(document.source(), &known_types)
                .into_iter()
                .map(|reference| IndexedReference {
                    name: reference.name,
                    kind: SymbolKind::STRUCT,
                    range: reference.range,
                    container_name: None,
                })
                .collect()
        })
        .unwrap_or_default()
}
