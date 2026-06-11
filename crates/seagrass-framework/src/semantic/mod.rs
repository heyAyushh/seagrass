pub mod nodes;
pub mod populated;
pub mod queries;

use std::collections::HashSet;

pub use {
    nodes::*,
    populated::{ExtractionConfidence, NodeKind, Populated, PopulatedFields},
};

impl SemanticModel {
    pub fn all_fields_for_struct(&self, struct_name: &str) -> Vec<&AccountField> {
        let Some(accounts_struct) = self.accounts_struct(struct_name) else {
            return Vec::new();
        };
        let mut fields = Vec::new();
        let mut visited = HashSet::new();
        self.collect_fields(accounts_struct, &mut visited, &mut fields);
        fields
    }

    pub fn accounts_struct(&self, struct_name: &str) -> Option<&AccountsStruct> {
        self.accounts_structs
            .iter()
            .map(|accounts_struct| &accounts_struct.inner)
            .find(|accounts_struct| accounts_struct.name == struct_name)
    }

    fn collect_fields<'a>(
        &'a self,
        accounts_struct: &'a AccountsStruct,
        visited: &mut HashSet<String>,
        fields: &mut Vec<&'a AccountField>,
    ) {
        if !visited.insert(accounts_struct.name.clone()) {
            return;
        }
        fields.extend(accounts_struct.fields.iter());
        for composite_ref in &accounts_struct.composite_refs {
            if let Some(resolved) = composite_ref.resolved.as_deref() {
                self.collect_fields(resolved, visited, fields);
            } else if let Some(target) = self.accounts_struct(&composite_ref.target_struct_name) {
                self.collect_fields(target, visited, fields);
            }
        }
    }
}
