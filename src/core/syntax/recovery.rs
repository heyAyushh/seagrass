use {
    super::{byte_candidates, contains_point, RustSyntax},
    crate::range::{byte_offset_at, point_at},
    tower_lsp::lsp_types::{Position, Range},
    tree_sitter::Node,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveredContextType {
    pub prefix: String,
    pub range: Range,
}

impl RustSyntax {
    pub fn context_type_prefix_at_position(
        &self,
        source: &str,
        position: Position,
    ) -> Option<RecoveredContextType> {
        let byte_offset = byte_offset_at(source, position)?;
        if !has_anchor_framework_hint(source, byte_offset) {
            return None;
        }
        let point = point_at(source, position)?;
        let function_start = byte_candidates(byte_offset)
            .into_iter()
            .find_map(|candidate| {
                self.tree
                    .root_node()
                    .descendant_for_byte_range(candidate, candidate)
                    .and_then(function_item_ancestor)
                    .filter(|function| contains_point(*function, point))
                    .map(|function| function.start_byte())
            });
        let function_start =
            function_start.or_else(|| function_signature_start(source, byte_offset))?;
        let prefix = source.get(function_start..byte_offset)?;
        let context_open = prefix.rfind("Context<")? + "Context<".len();
        let typed_prefix = &prefix[context_open..];
        if typed_prefix.contains('>') || typed_prefix.contains(',') {
            return None;
        }
        if !typed_prefix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            return None;
        }

        let start = function_start + context_open;
        Some(RecoveredContextType {
            prefix: typed_prefix.to_string(),
            range: Range {
                start: position_at_byte_offset(source, start),
                end: position,
            },
        })
    }
}

fn function_item_ancestor(mut node: Node<'_>) -> Option<Node<'_>> {
    loop {
        if node.kind() == "function_item" {
            return Some(node);
        }
        node = node.parent()?;
    }
}

fn function_signature_start(source: &str, offset: usize) -> Option<usize> {
    let before_cursor = &source[..offset.min(source.len())];
    let function_start = before_cursor.rfind("fn ")?;
    let function_prefix = &before_cursor[function_start..];
    (function_prefix.contains('(') && !function_prefix.contains('{')).then_some(function_start)
}

fn position_at_byte_offset(source: &str, offset: usize) -> Position {
    let offset = offset.min(source.len());
    let (line, line_start) = source
        .char_indices()
        .take_while(|(idx, _)| *idx < offset)
        .filter(|(_, ch)| *ch == '\n')
        .fold((0u32, 0usize), |(line, _), (idx, ch)| {
            (line + 1, idx + ch.len_utf8())
        });
    Position {
        line,
        character: u32::try_from(source[line_start..offset].chars().count()).unwrap_or_default(),
    }
}

fn has_anchor_framework_hint(source: &str, offset: usize) -> bool {
    let before_cursor = &source[..offset.min(source.len())];
    crate::solana::frameworks::source_has_anchor_framework_hint(before_cursor)
}
