#![allow(deprecated)]

use {
    crate::{
        constraint_ranges::constraint_key_ranges,
        document::AccountAttributeCursor,
        range::{byte_offset_at, point_at},
    },
    std::collections::HashSet,
    tower_lsp::lsp_types::{DocumentSymbol, Position, Range, SymbolKind},
    tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree},
};

mod recovery;

pub use recovery::RecoveredContextType;

const ANCHOR_OVERLAY_QUERY: &str = r#"
(attribute_item) @attribute
(mod_item name: (identifier) @mod.name) @mod
(function_item name: (identifier) @function.name) @function
(struct_item name: (type_identifier) @struct.name) @struct
(field_declaration name: (_) @field.name type: (_) @field.type) @field
"#;

#[derive(Debug)]
pub struct RustSyntax {
    tree: Tree,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorSyntaxReference {
    pub name: String,
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorFieldSyntax {
    pub name: Option<String>,
    pub range: Range,
    pub type_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorQueryKind {
    ProgramAttribute,
    ProgramModule,
    ProgramInstruction,
    DeriveAccountsAttribute,
    AccountsStruct,
    AccountAttribute,
    AccountConstraintKey,
    AccountDataStruct,
    AccountField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorQueryCapture {
    pub kind: AnchorQueryKind,
    pub name: Option<String>,
    pub range: Range,
}

impl RustSyntax {
    pub fn parse(source: &str) -> Option<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;
        parser.parse(source, None).map(|tree| Self { tree })
    }

    /// Parses source code, optionally reusing a previous tree for **incremental reparsing**.
    ///
    /// This is the core performance primitive for Phase 2. When a small edit arrives from the
    /// editor (or AI agent), we can pass the old `Tree` and tree-sitter will only re-parse the
    /// changed region instead of the entire file.
    ///
    /// # Example
    ///
    /// ```no_run
    /// let old = RustSyntax::parse("fn old() {}").unwrap();
    /// let new = RustSyntax::parse_edited("fn new() {}", Some(&old.tree));
    /// assert!(new.is_some());
    /// ```
    #[allow(dead_code)]
    pub fn parse_edited(source: &str, old_tree: Option<&Tree>) -> Option<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;
        parser.parse(source, old_tree).map(|tree| Self { tree })
    }

    /// Computes a tree-sitter `InputEdit` from an LSP change event.
    ///
    /// This bridges LSP `DidChangeTextDocumentParams.content_changes` to tree-sitter's
    /// incremental API. Supports both full document replacement and range-based edits.
    #[allow(dead_code)]
    pub fn edit_for_lsp_change(
        old_source: &str,
        change: &tower_lsp::lsp_types::TextDocumentContentChangeEvent,
    ) -> Option<InputEdit> {
        let new_text = &change.text;
        let old_bytes = old_source.as_bytes();
        let new_bytes = new_text.as_bytes();

        let (start_byte, old_end_byte, new_end_byte) = if let Some(range) = change.range {
            let start = byte_offset_at(old_source, range.start)?;
            let old_end = byte_offset_at(old_source, range.end)?;
            let new_end = start + new_bytes.len();
            (start, old_end, new_end)
        } else {
            // Full document replacement
            (0, old_bytes.len(), new_bytes.len())
        };

        let start_position = point_at(
            old_source,
            change.range.map(|r| r.start).unwrap_or_default(),
        )
        .unwrap_or(Point::new(0, 0));
        let old_end_position =
            point_at(old_source, change.range.map(|r| r.end).unwrap_or_default())
                .unwrap_or(Point::new(0, 0));
        let new_end_position =
            point_at(new_text, Position::new(0, new_text.lines().count() as u32))
                .unwrap_or(Point::new(0, 0));

        Some(InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        })
    }

    #[cfg(test)]
    pub fn root_kind(&self) -> &'static str {
        self.tree.root_node().kind()
    }

    #[cfg(test)]
    pub fn has_error(&self) -> bool {
        self.tree.root_node().has_error()
    }

    /// Returns the root node of the parsed tree.
    #[cfg(test)]
    pub fn root_node(&self) -> Node<'_> {
        self.tree.root_node()
    }

    pub fn account_attribute_at_position(&self, source: &str, position: Position) -> bool {
        self.account_attribute_range_at_position(source, position)
            .is_some()
    }

    pub fn account_attribute_cursor_at_position(
        &self,
        source: &str,
        position: Position,
    ) -> Option<AccountAttributeCursor> {
        self.account_attribute_range_at_position(source, position)
            .and_then(|range| AccountAttributeCursor::from_attribute_range(source, range, position))
    }

    pub fn account_attribute_range_at_position(
        &self,
        source: &str,
        position: Position,
    ) -> Option<Range> {
        let byte_offset = byte_offset_at(source, position)?;
        let point = point_at(source, position)?;

        let mut candidates = Vec::with_capacity(2);
        candidates.push(byte_offset);
        if byte_offset > 0 {
            candidates.push(byte_offset - 1);
        }

        candidates.into_iter().find_map(|candidate| {
            self.tree
                .root_node()
                .descendant_for_byte_range(candidate, candidate)
                .and_then(|node| account_attribute_ancestor(source, node))
                .filter(|node| {
                    contains_point(*node, point)
                        && node
                            .utf8_text(source.as_bytes())
                            .is_ok_and(is_account_attribute_text)
                })
                .map(node_range)
        })
    }

    pub fn accounts_struct_at_position(&self, source: &str, position: Position) -> bool {
        let Some(byte_offset) = byte_offset_at(source, position) else {
            return false;
        };
        let Some(point) = point_at(source, position) else {
            return false;
        };

        let mut candidates = Vec::with_capacity(2);
        candidates.push(byte_offset);
        if byte_offset > 0 {
            candidates.push(byte_offset - 1);
        }

        candidates.into_iter().any(|candidate| {
            self.tree
                .root_node()
                .descendant_for_byte_range(candidate, candidate)
                .and_then(|node| accounts_struct_ancestor(source, node))
                .is_some_and(|node| contains_point(node, point))
        }) || account_struct_nodes(self.tree.root_node(), source)
            .into_iter()
            .any(|node| contains_point(node, point))
    }

    pub fn account_field_at_position(
        &self,
        source: &str,
        position: Position,
    ) -> Option<AnchorFieldSyntax> {
        let byte_offset = byte_offset_at(source, position)?;
        let point = point_at(source, position)?;
        let candidates = byte_candidates(byte_offset);

        candidates.into_iter().find_map(|candidate| {
            self.tree
                .root_node()
                .descendant_for_byte_range(candidate, candidate)
                .and_then(|node| account_field_ancestor(source, node))
                .filter(|node| contains_point(*node, point))
                .map(|node| AnchorFieldSyntax {
                    name: node
                        .child_by_field_name("name")
                        .and_then(|name| node_text(source, name))
                        .map(str::to_string),
                    range: node_range(node),
                    type_text: node
                        .child_by_field_name("type")
                        .and_then(|ty| node_text(source, ty).map(|text| text.trim().to_string())),
                })
        })
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

    pub fn anchor_query_captures(&self, source: &str) -> Vec<AnchorQueryCapture> {
        let Ok(query) = Query::new(&tree_sitter_rust::LANGUAGE.into(), ANCHOR_OVERLAY_QUERY) else {
            return Vec::new();
        };
        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, self.tree.root_node(), source.as_bytes());
        let mut captures = Vec::new();

        while let Some(query_match) = {
            matches.advance();
            matches.get()
        } {
            if let Some(capture) = anchor_query_capture(source, &query, query_match) {
                captures.push(capture);
            }
        }
        let account_attribute_ranges = captures
            .iter()
            .filter(|capture| capture.kind == AnchorQueryKind::AccountAttribute)
            .map(|capture| capture.range)
            .collect::<Vec<_>>();
        captures.extend(
            account_attribute_ranges
                .into_iter()
                .flat_map(|range| constraint_key_ranges(source, range))
                .map(|key_range| AnchorQueryCapture {
                    kind: AnchorQueryKind::AccountConstraintKey,
                    name: Some(key_range.key.to_string()),
                    range: key_range.range,
                }),
        );

        captures.sort_by_key(|capture| {
            (
                capture.range.start.line,
                capture.range.start.character,
                capture.range.end.line,
                capture.range.end.character,
                anchor_query_kind_order(capture.kind),
            )
        });
        captures.dedup_by(|left, right| left.kind == right.kind && left.range == right.range);
        captures
    }
}

fn anchor_query_capture(
    source: &str,
    query: &Query,
    query_match: &tree_sitter::QueryMatch<'_, '_>,
) -> Option<AnchorQueryCapture> {
    let capture_names = query.capture_names();
    let node = capture_node(query_match, capture_names, "attribute");
    if let Some(node) = node {
        let text = node_text(source, node)?;
        let kind = if is_program_attribute_text(text) {
            AnchorQueryKind::ProgramAttribute
        } else if text.contains("derive") && text.contains("Accounts") {
            AnchorQueryKind::DeriveAccountsAttribute
        } else if is_account_attribute_text(text) {
            AnchorQueryKind::AccountAttribute
        } else {
            return None;
        };
        return Some(AnchorQueryCapture {
            kind,
            name: None,
            range: node_range(node),
        });
    }

    if let Some(node) = capture_node(query_match, capture_names, "mod") {
        if has_previous_attribute(source, node, is_program_attribute_text) {
            let name = capture_node(query_match, capture_names, "mod.name")?;
            return Some(AnchorQueryCapture {
                kind: AnchorQueryKind::ProgramModule,
                name: node_text(source, name).map(str::to_string),
                range: node_range(name),
            });
        }
    }

    if let Some(node) = capture_node(query_match, capture_names, "function") {
        if is_inside_program_module(source, node) {
            let name = capture_node(query_match, capture_names, "function.name")?;
            return Some(AnchorQueryCapture {
                kind: AnchorQueryKind::ProgramInstruction,
                name: node_text(source, name).map(str::to_string),
                range: node_range(name),
            });
        }
    }

    if let Some(node) = capture_node(query_match, capture_names, "struct") {
        let kind = if has_derive_accounts_attribute(source, node) {
            AnchorQueryKind::AccountsStruct
        } else if has_previous_attribute(source, node, is_account_attribute_text) {
            AnchorQueryKind::AccountDataStruct
        } else {
            return None;
        };
        let name = capture_node(query_match, capture_names, "struct.name")?;
        return Some(AnchorQueryCapture {
            kind,
            name: node_text(source, name).map(str::to_string),
            range: node_range(name),
        });
    }

    if let Some(node) = capture_node(query_match, capture_names, "field") {
        if account_field_ancestor(source, node).is_some() {
            let name = capture_node(query_match, capture_names, "field.name")?;
            return Some(AnchorQueryCapture {
                kind: AnchorQueryKind::AccountField,
                name: node_text(source, name).map(str::to_string),
                range: node_range(name),
            });
        }
    }

    None
}

fn capture_node<'tree>(
    query_match: &tree_sitter::QueryMatch<'_, 'tree>,
    capture_names: &[&str],
    name: &str,
) -> Option<Node<'tree>> {
    query_match.captures.iter().find_map(|capture| {
        (capture_names
            .get(usize::try_from(capture.index).ok()?)
            .is_some_and(|capture_name| *capture_name == name))
        .then_some(capture.node)
    })
}

fn is_inside_program_module(source: &str, mut node: Node<'_>) -> bool {
    loop {
        if node.kind() == "mod_item" {
            return has_previous_attribute(source, node, is_program_attribute_text);
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        node = parent;
    }
}

fn anchor_query_kind_order(kind: AnchorQueryKind) -> u8 {
    match kind {
        AnchorQueryKind::ProgramAttribute => 0,
        AnchorQueryKind::ProgramModule => 1,
        AnchorQueryKind::ProgramInstruction => 2,
        AnchorQueryKind::DeriveAccountsAttribute => 3,
        AnchorQueryKind::AccountsStruct => 4,
        AnchorQueryKind::AccountAttribute => 5,
        AnchorQueryKind::AccountConstraintKey => 6,
        AnchorQueryKind::AccountDataStruct => 7,
        AnchorQueryKind::AccountField => 8,
    }
}

fn account_attribute_ancestor<'tree>(source: &str, mut node: Node<'tree>) -> Option<Node<'tree>> {
    loop {
        if matches!(node.kind(), "attribute_item" | "attribute")
            && node
                .utf8_text(source.as_bytes())
                .is_ok_and(is_account_attribute_text)
        {
            return Some(node);
        }

        node = node.parent()?;
    }
}

fn is_account_attribute_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("#[account")
        || trimmed.starts_with("#[cfg_attr") && trimmed.contains("account(")
}

fn is_program_attribute_text(text: &str) -> bool {
    text.trim_start().starts_with("#[program")
}

fn accounts_struct_ancestor<'tree>(source: &str, mut node: Node<'tree>) -> Option<Node<'tree>> {
    loop {
        if node.kind() == "struct_item" && is_accounts_struct_node(source, node) {
            return Some(node);
        }

        node = node.parent()?;
    }
}

fn account_field_ancestor<'tree>(source: &str, mut node: Node<'tree>) -> Option<Node<'tree>> {
    let mut field = None;
    loop {
        if node.kind() == "field_declaration" {
            field = Some(node);
        }
        if node.kind() == "struct_item" {
            return is_accounts_struct_node(source, node).then_some(field?);
        }

        node = node.parent()?;
    }
}

fn is_accounts_struct_node(source: &str, node: Node<'_>) -> bool {
    is_accounts_struct_text(source, node) || has_derive_accounts_attribute(source, node)
}

fn is_accounts_struct_text(source: &str, node: Node<'_>) -> bool {
    node.utf8_text(source.as_bytes()).is_ok_and(|text| {
        let header = text.split_once('{').map_or(text, |(header, _)| header);
        header.contains("#[derive(Accounts")
    })
}

fn has_derive_accounts_attribute(source: &str, node: Node<'_>) -> bool {
    has_previous_attribute(source, node, |text| {
        text.contains("derive") && text.contains("Accounts")
    })
}

fn has_previous_attribute(source: &str, node: Node<'_>, matches: impl Fn(&str) -> bool) -> bool {
    let mut sibling = node.prev_named_sibling();
    while let Some(previous) = sibling {
        if previous.kind() != "attribute_item" {
            return false;
        }
        if previous.utf8_text(source.as_bytes()).is_ok_and(&matches) {
            return true;
        }
        sibling = previous.prev_named_sibling();
    }
    false
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

fn field_type_text(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|ty| node_text(source, ty))
        .map(str::to_string)
}

fn type_identifier_nodes(root: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut nodes = Vec::new();

    while let Some(node) = stack.pop() {
        if node.kind() == "type_identifier" {
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

fn node_text<'a>(source: &'a str, node: Node<'_>) -> Option<&'a str> {
    node.utf8_text(source.as_bytes()).ok()
}

fn node_range(node: Node<'_>) -> Range {
    Range {
        start: position_from_point(node.start_position()),
        end: position_from_point(node.end_position()),
    }
}

fn position_from_point(point: tree_sitter::Point) -> Position {
    Position {
        line: u32::try_from(point.row).unwrap_or_default(),
        character: u32::try_from(point.column).unwrap_or_default(),
    }
}

fn account_struct_nodes<'tree>(root: Node<'tree>, source: &str) -> Vec<Node<'tree>> {
    let mut cursor = root.walk();
    let mut stack = vec![root];
    let mut nodes = Vec::new();

    while let Some(node) = stack.pop() {
        if node.kind() == "struct_item" && is_accounts_struct_node(source, node) {
            nodes.push(node);
        }

        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    nodes
}

fn contains_point(node: Node<'_>, point: tree_sitter::Point) -> bool {
    let start = node.start_position();
    let end = node.end_position();
    (point.row > start.row || point.row == start.row && point.column >= start.column)
        && (point.row < end.row || point.row == end.row && point.column <= end.column)
}

fn byte_candidates(byte_offset: usize) -> Vec<usize> {
    let mut candidates = Vec::with_capacity(2);
    candidates.push(byte_offset);
    if byte_offset > 0 {
        candidates.push(byte_offset - 1);
    }
    candidates
}

#[cfg(test)]
mod tests;
