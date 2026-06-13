#![allow(deprecated)]

use {
    crate::{
        document::AccountAttributeCursor,
        range::{byte_offset_at, point_at},
    },
    tower_lsp::lsp_types::{Position, Range},
    tree_sitter::{Node, Parser, Tree},
};

#[cfg(test)]
use tree_sitter::{InputEdit, Point};

mod identifiers;
mod query;
mod recovery;
mod symbols;
mod syn_utils;

pub use identifiers::{
    is_ascii_identifier, is_ascii_identifier_byte, is_ascii_identifier_char,
    is_ascii_identifier_path, is_ascii_identifier_prefix, is_ascii_identifier_start,
    is_ascii_type_identifier, is_ascii_type_path,
};
pub use recovery::RecoveredContextType;
pub(crate) use syn_utils::{
    expr_path_ends_with, expr_path_last_ident, member_is_named, member_name,
};

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
    #[cfg(test)]
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
    #[cfg(test)]
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
