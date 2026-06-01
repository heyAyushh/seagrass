use {
    super::{
        account_field_ancestor, has_derive_accounts_attribute, has_previous_attribute,
        is_account_attribute_text, is_program_attribute_text, node_range, node_text,
        AnchorQueryCapture, AnchorQueryKind, RustSyntax,
    },
    crate::constraint_ranges::constraint_key_ranges,
    tree_sitter::{Node, Query, QueryCursor, StreamingIterator},
};

const ANCHOR_OVERLAY_QUERY: &str = r#"
(attribute_item) @attribute
(mod_item name: (identifier) @mod.name) @mod
(function_item name: (identifier) @function.name) @function
(struct_item name: (type_identifier) @struct.name) @struct
(field_declaration name: (_) @field.name type: (_) @field.type) @field
"#;

impl RustSyntax {
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
