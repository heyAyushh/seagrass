use {super::WorkspaceIndex, tower_lsp::lsp_types::SymbolKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAssociatedValue {
    pub name: String,
    pub kind: SymbolKind,
}

impl WorkspaceIndex {
    pub fn associated_values_in_container(
        &self,
        container_name: &str,
    ) -> Vec<WorkspaceAssociatedValue> {
        let mut values = self
            .symbols_by_name
            .values()
            .flat_map(|entries| entries.iter())
            .filter(|entry| {
                matches!(
                    entry.kind,
                    SymbolKind::CONSTANT | SymbolKind::METHOD | SymbolKind::FUNCTION
                ) && entry.container_name.as_deref() == Some(container_name)
            })
            .map(|entry| WorkspaceAssociatedValue {
                name: entry.name.to_string(),
                kind: entry.kind,
            })
            .collect::<Vec<_>>();
        values.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| symbol_kind_rank(left.kind).cmp(&symbol_kind_rank(right.kind)))
        });
        values.dedup_by(|left, right| left.name == right.name && left.kind == right.kind);
        values
    }
}

fn symbol_kind_rank(kind: SymbolKind) -> u8 {
    match kind {
        SymbolKind::CONSTANT => 0,
        SymbolKind::METHOD => 1,
        SymbolKind::FUNCTION => 2,
        _ => 3,
    }
}
