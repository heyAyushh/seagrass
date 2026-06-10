use {
    super::{
        derives_accounts, has_attr, is_anchor_helper_function, path_last_is_any_ident,
        InstructionSymbol, ParsedDocument, SymbolRange,
    },
    crate::{
        anchor::idioms, constraint_catalog, constraint_ranges::constraint_key_ranges,
        range::range_from_span,
    },
    syn::{spanned::Spanned, Item},
    tower_lsp::lsp_types::{DocumentSymbol, SymbolKind},
};

pub fn document_symbols(document: &ParsedDocument) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();

    for item in &document.syntax().items {
        match item {
            Item::Macro(item_macro)
                if path_last_is_any_ident(
                    &item_macro.mac.path,
                    idioms::PROGRAM_DECLARATION_MACROS,
                ) =>
            {
                let Some(declared) = document.symbols().declared_program_id.as_ref() else {
                    continue;
                };
                symbols.push(DocumentSymbol {
                    name: macro_symbol_name(item_macro),
                    detail: Some(declared.value.clone()),
                    kind: SymbolKind::CONSTANT,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(item_macro.span()),
                    selection_range: declared.range,
                    children: None,
                });
            }
            Item::Mod(item_mod) if has_attr(&item_mod.attrs, "program") => {
                let children = document
                    .symbols()
                    .instructions
                    .iter()
                    .map(instruction_document_symbol)
                    .collect::<Vec<_>>();

                symbols.push(DocumentSymbol {
                    name: item_mod.ident.to_string(),
                    detail: Some("#[program]".to_string()),
                    kind: SymbolKind::MODULE,
                    tags: None,
                    deprecated: None,
                    range: range_from_span(item_mod.span()),
                    selection_range: range_from_span(item_mod.ident.span()),
                    children: (!children.is_empty()).then_some(children),
                });
            }
            Item::Struct(item_struct) if derives_accounts(&item_struct.attrs) => {
                let Some(symbol) = document
                    .symbols()
                    .accounts_structs
                    .get(&item_struct.ident.to_string())
                else {
                    continue;
                };
                symbols.push(struct_document_symbol(
                    document.source(),
                    symbol,
                    Some("#[derive(Accounts)]".to_string()),
                ));
            }
            Item::Struct(item_struct) if has_attr(&item_struct.attrs, "account") => {
                let Some(symbol) = document
                    .symbols()
                    .account_data_structs
                    .get(&item_struct.ident.to_string())
                else {
                    continue;
                };
                symbols.push(struct_document_symbol(
                    document.source(),
                    symbol,
                    Some("#[account]".to_string()),
                ));
            }
            Item::Fn(item_fn) if is_anchor_helper_function(item_fn) => {
                let Some(function) = document
                    .symbols()
                    .functions
                    .iter()
                    .find(|function| item_fn.sig.ident == function.name.as_str())
                else {
                    continue;
                };
                symbols.push(function_document_symbol(
                    function,
                    Some("Anchor helper function".to_string()),
                ));
            }
            _ => {}
        }
    }

    if symbols.is_empty() {
        symbols = document
            .tree_sitter()
            .map(|syntax| syntax.anchor_document_symbols(document.source()))
            .unwrap_or_default();
    }

    symbols
}

fn macro_symbol_name(item_macro: &syn::ItemMacro) -> String {
    item_macro
        .mac
        .path
        .segments
        .last()
        .map(|segment| format!("{}!", segment.ident))
        .unwrap_or_default()
}

fn instruction_document_symbol(instruction: &InstructionSymbol) -> DocumentSymbol {
    function_document_symbol(instruction, None)
}

fn function_document_symbol(
    instruction: &InstructionSymbol,
    fallback_detail: Option<String>,
) -> DocumentSymbol {
    let children = instruction
        .arguments
        .iter()
        .map(|argument| DocumentSymbol {
            name: argument.name.clone(),
            detail: argument.type_name.clone(),
            kind: SymbolKind::VARIABLE,
            tags: None,
            deprecated: None,
            range: argument.range,
            selection_range: argument.range,
            children: None,
        })
        .collect::<Vec<_>>();

    DocumentSymbol {
        name: instruction.name.clone(),
        detail: instruction
            .context
            .as_ref()
            .map(|context| format!("instruction Context<{}>", context.name))
            .or(fallback_detail)
            .or_else(|| Some("instruction".to_string())),
        kind: SymbolKind::FUNCTION,
        tags: None,
        deprecated: None,
        range: instruction.range,
        selection_range: instruction.selection_range,
        children: (!children.is_empty()).then_some(children),
    }
}

fn struct_document_symbol(
    source: &str,
    symbol: &SymbolRange,
    detail: Option<String>,
) -> DocumentSymbol {
    let children = symbol
        .fields
        .iter()
        .map(|field| field_document_symbol(source, field))
        .collect::<Vec<_>>();

    DocumentSymbol {
        name: symbol.name.clone(),
        detail,
        kind: SymbolKind::STRUCT,
        tags: None,
        deprecated: None,
        range: symbol.range,
        selection_range: symbol.selection_range,
        children: (!children.is_empty()).then_some(children),
    }
}

fn field_document_symbol(source: &str, field: &SymbolRange) -> DocumentSymbol {
    let constraint_children = constraint_document_symbols(source, field);
    DocumentSymbol {
        name: field.name.clone(),
        detail: field_detail(field),
        kind: SymbolKind::FIELD,
        tags: None,
        deprecated: None,
        range: field.range,
        selection_range: field.selection_range,
        children: (!constraint_children.is_empty()).then_some(constraint_children),
    }
}

fn constraint_document_symbols(source: &str, field: &SymbolRange) -> Vec<DocumentSymbol> {
    let mut symbols = Vec::new();
    for attribute in &field.account_constraints {
        for key_range in constraint_key_ranges(source, attribute.range) {
            let Some(spec) = constraint_catalog::by_key(key_range.key) else {
                continue;
            };
            symbols.push(DocumentSymbol {
                name: constraint_catalog::key(spec.label).to_string(),
                detail: Some(format!("{:?} constraint", spec.family)),
                kind: SymbolKind::PROPERTY,
                tags: None,
                deprecated: None,
                range: key_range.range,
                selection_range: key_range.range,
                children: None,
            });
        }
    }
    symbols.sort_by_key(|symbol| (symbol.range.start.line, symbol.range.start.character));
    symbols.dedup_by_key(|symbol| {
        (
            symbol.name.clone(),
            symbol.range.start.line,
            symbol.range.start.character,
            symbol.range.end.line,
            symbol.range.end.character,
        )
    });
    symbols
}

fn field_detail(field: &SymbolRange) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(type_name) = &field.type_name {
        parts.push(type_name.clone());
    }
    if !field.generic_type_names.is_empty() {
        parts.push(format!("<{}>", field.generic_type_names.join(", ")));
    }
    if !field.account_constraints.is_empty() {
        parts.push(format!(
            "{} constraint{}",
            field.account_constraints.len(),
            if field.account_constraints.len() == 1 {
                ""
            } else {
                "s"
            }
        ));
    }
    (!parts.is_empty()).then(|| parts.join(" "))
}
