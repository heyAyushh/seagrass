use super::WorkspaceIndex;

impl WorkspaceIndex {
    pub fn function_return_type_displays(&self, function_name: &str) -> Vec<String> {
        self.functions_by_name
            .get(function_name)
            .into_iter()
            .flat_map(|entries| entries.iter())
            .filter_map(|entry| entry.return_type_display.clone())
            .collect()
    }
}
