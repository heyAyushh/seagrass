use super::WorkspaceIndex;

impl WorkspaceIndex {
    pub(super) fn sort_open_entries_first(&mut self) {
        self.symbols_by_name
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
        self.references_by_name
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
        self.functions_by_context
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
        self.functions_by_name
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
        self.accounts_by_name
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
        self.account_data_by_name
            .values_mut()
            .for_each(|entries| entries.sort_by_key(|entry| !entry.is_open));
    }
}
