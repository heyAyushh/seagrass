use {
    super::{
        has_derive_accounts_attribute, has_previous_attribute, is_account_attribute_text,
        is_program_attribute_text, node_range, node_text, AnchorFieldSyntax, AnchorSyntaxReference,
        RustSyntax,
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{DocumentSymbol, SymbolKind},
    tree_sitter::Node,
};

const DECLARATION_LIST_NODE: &str = "declaration_list";
const FUNCTION_ITEM_NODE: &str = "function_item";
const IMPL_ITEM_NODE: &str = "impl_item";
const PARAMETERS_NODE: &str = "parameters";
const SELF_PARAMETER_NODE: &str = "self_parameter";
const STRUCT_ITEM_NODE: &str = "struct_item";
const TYPE_IDENTIFIER_NODE: &str = "type_identifier";

impl RustSyntax {
    pub fn struct_fields_named(&self, source: &str, struct_name: &str) -> Vec<AnchorFieldSyntax> {
        struct_item_nodes(self.tree.root_node())
            .into_iter()
            .find(|node| {
                node.child_by_field_name("name")
                    .and_then(|name| node_text(source, name))
                    .is_some_and(|name| name == struct_name)
            })
            .map(|node| struct_field_syntax(source, node))
            .unwrap_or_default()
    }

    pub fn inherent_method_names(&self, source: &str, owner_type: &str) -> Vec<String> {
        impl_item_nodes(self.tree.root_node())
            .into_iter()
            .filter(|node| impl_owner_type_name(source, *node).as_deref() == Some(owner_type))
            .flat_map(impl_method_nodes)
            .filter(|node| function_has_self_receiver(*node))
            .filter_map(|node| {
                node.child_by_field_name("name")
                    .and_then(|name| node_text(source, name))
                    .map(str::to_string)
            })
            .collect()
    }

    pub fn anchor_document_symbols(&self, source: &str) -> Vec<DocumentSymbol> {
        let mut symbols = Vec::new();
        let root = self.tree.root_node();

        for node in anchor_nodes(root, source) {
            match node.kind() {
                "mod_item" if has_previous_attribute(source, node, is_program_attribute_text) => {
                    if let Some(symbol) = program_symbol(source, node) {
                        symbols.push(symbol);
                    }
                }
                "struct_item" if has_derive_accounts_attribute(source, node) => {
                    if let Some(symbol) =
                        struct_symbol(source, node, Some("#[derive(Accounts)]".to_string()))
                    {
                        symbols.push(symbol);
                    }
                }
                "struct_item"
                    if has_previous_attribute(source, node, is_account_attribute_text) =>
                {
                    if let Some(symbol) =
                        struct_symbol(source, node, Some("#[account]".to_string()))
                    {
                        symbols.push(symbol);
                    }
                }
                _ => {}
            }
        }

        symbols
    }

    pub fn anchor_type_references(
        &self,
        source: &str,
        known_types: &HashSet<String>,
    ) -> Vec<AnchorSyntaxReference> {
        if known_types.is_empty() {
            return Vec::new();
        }

        type_identifier_nodes(self.tree.root_node())
            .into_iter()
            .filter_map(|node| {
                let name = node_text(source, node)?;
                known_types.contains(name).then(|| AnchorSyntaxReference {
                    name: name.to_string(),
                    range: node_range(node),
                })
            })
            .collect()
    }
}

fn anchor_nodes<'tree>(root: Node<'tree>, source: &str) -> Vec<Node<'tree>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut nodes = Vec::new();

    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "mod_item" | "struct_item")
            && (has_previous_attribute(source, node, is_program_attribute_text)
                || has_previous_attribute(source, node, is_account_attribute_text)
                || has_derive_accounts_attribute(source, node))
        {
            nodes.push(node);
        }

        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    nodes.sort_by_key(Node::start_byte);
    nodes
}

fn program_symbol(source: &str, node: Node<'_>) -> Option<DocumentSymbol> {
    let name = node.child_by_field_name("name")?;
    let children = function_children(source, node);
    Some(DocumentSymbol {
        name: node_text(source, name)?.to_string(),
        detail: Some("#[program]".to_string()),
        kind: SymbolKind::MODULE,
        tags: None,
        deprecated: None,
        range: node_range(node),
        selection_range: node_range(name),
        children: (!children.is_empty()).then_some(children),
    })
}

fn function_children(source: &str, node: Node<'_>) -> Vec<DocumentSymbol> {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut symbols = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "function_item" {
            if let Some(name) = child.child_by_field_name("name") {
                if let Some(name_text) = node_text(source, name) {
                    symbols.push(DocumentSymbol {
                        name: name_text.to_string(),
                        detail: Some("instruction".to_string()),
                        kind: SymbolKind::FUNCTION,
                        tags: None,
                        deprecated: None,
                        range: node_range(child),
                        selection_range: node_range(name),
                        children: None,
                    });
                }
            }
            continue;
        }

        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }

    symbols.sort_by_key(|symbol| {
        (
            symbol.selection_range.start.line,
            symbol.selection_range.start.character,
        )
    });
    symbols
}

fn struct_symbol(source: &str, node: Node<'_>, detail: Option<String>) -> Option<DocumentSymbol> {
    let name = node.child_by_field_name("name")?;
    let children = field_children(source, node);
    Some(DocumentSymbol {
        name: node_text(source, name)?.to_string(),
        detail,
        kind: SymbolKind::STRUCT,
        tags: None,
        deprecated: None,
        range: node_range(node),
        selection_range: node_range(name),
        children: (!children.is_empty()).then_some(children),
    })
}

fn field_children(source: &str, node: Node<'_>) -> Vec<DocumentSymbol> {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut symbols = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "field_declaration" {
            if let Some(name) = child.child_by_field_name("name") {
                if let Some(name_text) = node_text(source, name) {
                    symbols.push(DocumentSymbol {
                        name: name_text.to_string(),
                        detail: field_type_text(source, child),
                        kind: SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
                        range: node_range(child),
                        selection_range: node_range(name),
                        children: None,
                    });
                }
            }
            continue;
        }

        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }

    symbols.sort_by_key(|symbol| {
        (
            symbol.selection_range.start.line,
            symbol.selection_range.start.character,
        )
    });
    symbols
}

fn struct_field_syntax(source: &str, node: Node<'_>) -> Vec<AnchorFieldSyntax> {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut fields = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "field_declaration" {
            fields.push(AnchorFieldSyntax {
                name: child
                    .child_by_field_name("name")
                    .and_then(|name| node_text(source, name))
                    .map(str::to_string),
                range: node_range(child),
                type_text: field_type_text(source, child),
            });
            continue;
        }

        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }

    fields.sort_by_key(|field| (field.range.start.line, field.range.start.character));
    fields
}

fn field_type_text(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|ty| node_text(source, ty))
        .map(str::to_string)
}

fn impl_item_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    nodes_of_kind(root, IMPL_ITEM_NODE)
}

fn impl_method_nodes(node: Node<'_>) -> Vec<Node<'_>> {
    let Some(declarations) = direct_named_children(node)
        .into_iter()
        .find(|child| child.kind() == DECLARATION_LIST_NODE)
    else {
        return Vec::new();
    };

    direct_named_children(declarations)
        .into_iter()
        .filter(|child| child.kind() == FUNCTION_ITEM_NODE)
        .collect()
}

fn impl_owner_type_name(source: &str, node: Node<'_>) -> Option<String> {
    type_identifier_children_before_body(node)
        .into_iter()
        .filter_map(|child| node_text(source, child).map(str::to_string))
        .next_back()
}

fn type_identifier_children_before_body(node: Node<'_>) -> Vec<Node<'_>> {
    let mut type_identifiers = Vec::new();
    for child in direct_named_children(node) {
        if child.kind() == DECLARATION_LIST_NODE {
            break;
        }
        type_identifiers.extend(nodes_of_kind(child, TYPE_IDENTIFIER_NODE));
    }
    type_identifiers.sort_by_key(Node::start_byte);
    type_identifiers
}

fn function_has_self_receiver(node: Node<'_>) -> bool {
    let parameters = node.child_by_field_name(PARAMETERS_NODE).or_else(|| {
        direct_named_children(node)
            .into_iter()
            .find(|child| child.kind() == PARAMETERS_NODE)
    });
    parameters
        .and_then(|parameters| direct_named_children(parameters).into_iter().next())
        .is_some_and(|first_parameter| first_parameter.kind() == SELF_PARAMETER_NODE)
}

fn direct_named_children(node: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .filter(|child| child.is_named())
        .collect()
}

fn type_identifier_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    nodes_of_kind(root, TYPE_IDENTIFIER_NODE)
}

fn struct_item_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    nodes_of_kind(root, STRUCT_ITEM_NODE)
}

fn nodes_of_kind<'tree>(root: Node<'tree>, kind: &str) -> Vec<Node<'tree>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut nodes = Vec::new();

    while let Some(node) = stack.pop() {
        if node.kind() == kind {
            nodes.push(node);
            continue;
        }

        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    nodes.sort_by_key(Node::start_byte);
    nodes
}
