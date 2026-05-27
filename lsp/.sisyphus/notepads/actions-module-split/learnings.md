## Pattern for extracting action families from mod.rs

- Each extracted module follows: module-level docstring, `pub fn code_actions(...) -> Vec<CodeAction>` as single seam
- Private helper functions stay within the module
- `common` module provides shared utilities (`diagnostic_code`, `edit_distance`, `single_text_edit`, `single_document_edit`, etc.)
- Router in mod.rs calls each family's `code_actions()` with `uri.clone()` and `diagnostics`
- Import cleanup: remove imports only used by moved functions; add `std::collections::HashMap` back if still needed
- Pre-existing warnings (constraints.rs unused ConstraintsValueKind, etc.) should be left alone
