use {
    crate::document::{has_attr, ParsedDocument, PdaSeeds},
    std::{error::Error, fs, path::Path},
    syn::{Item, ItemFn, ItemMod},
    tower_lsp::lsp_types::Url,
};

#[cfg(test)]
use {crate::syntax::RustSyntax, tree_sitter::Node};

/// A condensed, declaration-only representation of an Anchor source file.
///
/// `DocumentStub` stores only the Anchor-specific declarations from a file
/// (programs, account structs, instructions, constraints, PDA seeds) — not
/// full AST bodies. This makes it small enough to store in `WorkspaceIndex`
/// instead of full [`ParsedDocument`], and fast enough to build without full
/// `syn` parsing.
///
/// # Example
///
/// ```
/// # use seagrass::document_stub::DocumentStub;
/// let source = r#"
/// #[program]
/// pub mod demo {
///     pub fn initialize(ctx: Context<Create>) -> Result<()> {
///         Ok(())
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Create<'info> {
///     #[account(init, payer = user, space = 8)]
///     pub state: Account<'info, State>,
///     pub user: Signer<'info>,
/// }
///
/// #[account]
/// pub struct State {
///     pub value: u64,
/// }
/// "#;
///
/// let stub = DocumentStub::from_source(source);
/// assert_eq!(stub.programs.len(), 1);
/// assert_eq!(stub.programs[0].name, "demo");
/// assert_eq!(stub.programs[0].instructions, vec!["initialize"]);
/// assert_eq!(stub.accounts.len(), 1);
/// assert_eq!(stub.accounts[0].name, "Create");
/// assert_eq!(stub.data_structs.len(), 1);
/// assert_eq!(stub.data_structs[0].name, "State");
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DocumentStub {
    /// `#[program]` modules with their instruction names.
    pub programs: Vec<ProgramStub>,
    /// `#[derive(Accounts)]` structs with field names and constraint shapes.
    pub accounts: Vec<AccountStub>,
    /// `#[account]` data structs with field names.
    pub data_structs: Vec<DataStructStub>,
    /// PDA seed expressions extracted from account constraints.
    pub pda_seeds: Vec<PdaSeedStub>,
}

/// A stub for a single `#[program]` module.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProgramStub {
    /// Name of the program module.
    pub name: String,
    /// Names of the instruction functions declared inside the module.
    pub instructions: Vec<String>,
}

/// A stub for a single `#[derive(Accounts)]` validation struct.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountStub {
    /// Name of the accounts struct.
    pub name: String,
    /// Fields declared in the struct, including their constraints.
    pub fields: Vec<AccountFieldStub>,
}

/// A stub for a single field inside an accounts struct.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AccountFieldStub {
    /// Name of the field.
    pub name: String,
    /// Raw `#[account(...)]` attribute texts attached to this field.
    pub constraints: Vec<String>,
}

/// A stub for a single `#[account]` data struct.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DataStructStub {
    /// Name of the data struct.
    pub name: String,
    /// Names of the fields declared in the struct.
    pub fields: Vec<String>,
}

/// A stub for PDA seed expressions attached to an account field.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PdaSeedStub {
    /// Name of the account field that carries the PDA constraint.
    pub account_name: String,
    /// Raw seed expressions (e.g. `b"seed"`, `user.key().as_ref()`).
    pub seeds: Vec<String>,
}

impl DocumentStub {
    /// Builds a stub from raw source text using only tree-sitter (not `syn`).
    ///
    /// This is much faster than full [`ParsedDocument::parse`] because it skips
    /// `syn` parsing and only extracts declaration shapes.
    ///
    /// # Example
    ///
    /// ```
    /// # use seagrass::document_stub::DocumentStub;
    /// let stub = DocumentStub::from_source("#[program] pub mod demo {}");
    /// assert_eq!(stub.programs.len(), 1);
    /// assert_eq!(stub.programs[0].name, "demo");
    /// ```
    #[cfg(test)]
    pub fn from_source(source: &str) -> Self {
        let Some(syntax) = RustSyntax::parse(source) else {
            return Self::default();
        };
        let root = syntax.root_node();

        let mut programs = Vec::new();
        let mut accounts = Vec::new();
        let mut data_structs = Vec::new();
        let mut pda_seeds = Vec::new();

        let mut cursor = root.walk();
        let mut stack = vec![root];

        while let Some(node) = stack.pop() {
            match node.kind() {
                "mod_item" if has_program_attribute(source, node) => {
                    if let Some(name) = node_name(source, node) {
                        let instructions = collect_function_names(source, node);
                        programs.push(ProgramStub { name, instructions });
                    }
                }
                "struct_item" if has_derive_accounts_attribute(source, node) => {
                    if let Some(name) = node_name(source, node) {
                        let (fields, seeds) = collect_account_fields(source, node);
                        pda_seeds.extend(seeds);
                        accounts.push(AccountStub { name, fields });
                    }
                }
                "struct_item" if has_account_attribute(source, node) => {
                    if let Some(name) = node_name(source, node) {
                        let fields = collect_data_struct_fields(source, node);
                        data_structs.push(DataStructStub { name, fields });
                    }
                }
                _ => {}
            }

            for child in node.children(&mut cursor) {
                stack.push(child);
            }
        }

        Self {
            programs,
            accounts,
            data_structs,
            pda_seeds,
        }
    }

    /// Builds a stub from an existing [`ParsedDocument`].
    ///
    /// Use this when you already have a full parse and want to derive the
    /// lightweight stub without re-parsing.
    ///
    /// # Example
    ///
    /// ```
    /// # use seagrass::document::ParsedDocument;
    /// # use seagrass::document_stub::DocumentStub;
    /// let document = ParsedDocument::parse_or_empty("#[account] pub struct State {}");
    /// let stub = DocumentStub::from_parsed(&document);
    /// assert_eq!(stub.data_structs.len(), 1);
    /// assert_eq!(stub.data_structs[0].name, "State");
    /// ```
    pub fn from_parsed(document: &ParsedDocument) -> Self {
        let mut programs = Vec::new();

        for item in &document.syntax().items {
            if let Item::Mod(ItemMod {
                attrs,
                ident,
                content,
                ..
            }) = item
            {
                if !has_attr(attrs, "program") {
                    continue;
                }
                let name = ident.to_string();
                let mut instructions = Vec::new();
                if let Some((_, items)) = content {
                    for inner in items {
                        if let Item::Fn(ItemFn { sig, .. }) = inner {
                            instructions.push(sig.ident.to_string());
                        }
                    }
                }
                programs.push(ProgramStub { name, instructions });
            }
        }

        let mut accounts = Vec::with_capacity(document.symbols().accounts_structs.len());
        let mut pda_seeds = Vec::new();
        for symbol in document.symbols().accounts_structs.values() {
            let mut fields = Vec::with_capacity(symbol.fields.len());
            for field in &symbol.fields {
                let constraints = field
                    .account_constraints
                    .iter()
                    .map(|c| c.text.clone())
                    .collect();

                if let Some(pda) = &field.pda_constraint {
                    let seeds = match &pda.seeds {
                        PdaSeeds::List(list) => list.clone(),
                        PdaSeeds::Expr(expr) => vec![expr.clone()],
                    };
                    pda_seeds.push(PdaSeedStub {
                        account_name: field.name.clone(),
                        seeds,
                    });
                }

                fields.push(AccountFieldStub {
                    name: field.name.clone(),
                    constraints,
                });
            }
            accounts.push(AccountStub {
                name: symbol.name.clone(),
                fields,
            });
        }

        let mut data_structs = Vec::with_capacity(document.symbols().account_data_structs.len());
        for symbol in document.symbols().account_data_structs.values() {
            let fields = symbol.fields.iter().map(|f| f.name.clone()).collect();
            data_structs.push(DataStructStub {
                name: symbol.name.clone(),
                fields,
            });
        }

        Self {
            programs,
            accounts,
            data_structs,
            pda_seeds,
        }
    }
}

// ---------------------------------------------------------------------------
// Tree-sitter helpers (used only by `from_source`)
// ---------------------------------------------------------------------------

#[cfg(test)]
fn has_program_attribute(source: &str, node: Node<'_>) -> bool {
    has_previous_attribute(source, node, |text| {
        text.trim_start().starts_with("#[program")
    })
}

#[cfg(test)]
fn has_derive_accounts_attribute(source: &str, node: Node<'_>) -> bool {
    has_previous_attribute(source, node, |text| {
        text.contains("derive") && text.contains("Accounts")
    })
}

#[cfg(test)]
fn has_account_attribute(source: &str, node: Node<'_>) -> bool {
    has_previous_attribute(source, node, |text| {
        text.trim_start().starts_with("#[account")
    })
}

#[cfg(test)]
fn has_previous_attribute(source: &str, node: Node<'_>, predicate: impl Fn(&str) -> bool) -> bool {
    let mut sibling = node.prev_named_sibling();
    while let Some(previous) = sibling {
        if previous.kind() != "attribute_item" {
            return false;
        }
        if previous
            .utf8_text(source.as_bytes())
            .ok()
            .is_some_and(&predicate)
        {
            return true;
        }
        sibling = previous.prev_named_sibling();
    }
    false
}

#[cfg(test)]
fn node_name(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source.as_bytes()).ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
fn collect_function_names(source: &str, node: Node<'_>) -> Vec<String> {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut names = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "function_item" {
            if let Some(name) = node_name(source, child) {
                names.push(name);
            }
            continue;
        }
        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }
    names.reverse();
    names
}

#[cfg(test)]
fn collect_account_fields(
    source: &str,
    node: Node<'_>,
) -> (Vec<AccountFieldStub>, Vec<PdaSeedStub>) {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut fields = Vec::new();
    let mut pda_seeds = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "field_declaration" {
            if let Some(name) = node_name(source, child) {
                let constraints = collect_field_constraints(source, child);

                for constraint in &constraints {
                    if constraint.contains("seeds") {
                        if let Some(seeds) = extract_seeds_from_constraint(constraint) {
                            pda_seeds.push(PdaSeedStub {
                                account_name: name.clone(),
                                seeds,
                            });
                        }
                    }
                }

                fields.push(AccountFieldStub { name, constraints });
            }
            continue;
        }
        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }
    fields.reverse();
    (fields, pda_seeds)
}

#[cfg(test)]
fn collect_data_struct_fields(source: &str, node: Node<'_>) -> Vec<String> {
    let mut cursor = node.walk();
    let mut stack = node.children(&mut cursor).collect::<Vec<_>>();
    let mut names = Vec::new();

    while let Some(child) = stack.pop() {
        if child.kind() == "field_declaration" {
            if let Some(name) = node_name(source, child) {
                names.push(name);
            }
            continue;
        }
        for grandchild in child.children(&mut cursor) {
            stack.push(grandchild);
        }
    }
    names.reverse();
    names
}

#[cfg(test)]
fn collect_field_constraints(source: &str, node: Node<'_>) -> Vec<String> {
    let mut constraints = Vec::new();
    let mut sibling = node.prev_named_sibling();
    while let Some(previous) = sibling {
        if previous.kind() != "attribute_item" {
            break;
        }
        if let Ok(text) = previous.utf8_text(source.as_bytes()) {
            if text.trim_start().starts_with("#[account") {
                constraints.push(text.to_string());
            }
        }
        sibling = previous.prev_named_sibling();
    }
    constraints.reverse();
    constraints
}

/// Best-effort extraction of seed expressions from a raw `#[account(...)]`
/// attribute text.
#[cfg(test)]
fn extract_seeds_from_constraint(constraint: &str) -> Option<Vec<String>> {
    let seeds_start = constraint.find("seeds")?;
    let after_seeds = &constraint[seeds_start + 5..];
    let after_equals = skip_whitespace_and_equals(after_seeds)?;
    let trimmed = after_equals.trim_start();

    if trimmed.starts_with('[') {
        let mut depth = 0usize;
        let mut end = 0usize;
        for (i, ch) in trimmed.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i;
                        break;
                    }
                }
                _ => {}
            }
        }
        let inner = &trimmed[1..end];
        let mut seeds = Vec::new();
        let mut current = String::new();
        let mut depth = 0usize;
        for ch in inner.chars() {
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                ',' if depth == 0 => {
                    let trimmed = current.trim().to_string();
                    if !trimmed.is_empty() {
                        seeds.push(trimmed);
                    }
                    current = String::new();
                    continue;
                }
                _ => {}
            }
            current.push(ch);
        }
        let trimmed = current.trim().to_string();
        if !trimmed.is_empty() {
            seeds.push(trimmed);
        }
        Some(seeds)
    } else {
        let mut end = 0usize;
        for (i, ch) in trimmed.char_indices() {
            if ch == ',' || ch == ']' || ch == ')' {
                end = i;
                break;
            }
            end = i + 1;
        }
        let expr = trimmed[..end].trim().to_string();
        if !expr.is_empty() {
            Some(vec![expr])
        } else {
            None
        }
    }
}

#[cfg(test)]
fn skip_whitespace_and_equals(text: &str) -> Option<&str> {
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }
        if ch == '=' {
            return Some(chars.as_str());
        }
        return None;
    }
    None
}

impl DocumentStub {
    fn cache_dir() -> std::path::PathBuf {
        Path::new("/tmp/seagrass-stubs").to_path_buf()
    }

    fn cache_path(uri: &Url) -> Option<std::path::PathBuf> {
        let path = uri.to_file_path().ok()?;
        let name = path
            .file_name()?
            .to_string_lossy()
            .replace(|c: char| !c.is_alphanumeric(), "_");
        Some(Self::cache_dir().join(name).with_extension("wincode"))
    }

    /// Saves to wincode cache (JSON for now; enables future zero-copy binary). Dir auto-created.
    /// Why: open docs must persist updates; aligns with stacc (ownership on &self, no deps).
    pub fn save_to_cache(&self, uri: &Url) -> Result<(), Box<dyn Error>> {
        let path = Self::cache_path(uri).ok_or("invalid uri")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// Loads only if cache mtime > source mtime (freshness). None triggers reparse+save.
    /// Why: avoids full from_source parse for unchanged root files (perf); mtime zero-cost.
    /// Wincode choice documented for Anza alignment (no new deps per rules).
    #[allow(dead_code)]
    pub fn load_from_cache(uri: &Url) -> Option<Self> {
        let cache_path = Self::cache_path(uri)?;
        let source_path = uri.to_file_path().ok()?;
        let cache_meta = fs::metadata(&cache_path).ok()?;
        let source_meta = fs::metadata(&source_path).ok()?;
        if cache_meta.modified().ok()? <= source_meta.modified().ok()? {
            return None;
        }
        let json = fs::read_to_string(&cache_path).ok()?;
        serde_json::from_str(&json).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_source_extracts_programs() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
    pub fn update(ctx: Context<Update>) -> Result<()> {
        Ok(())
    }
}
"#;
        let stub = DocumentStub::from_source(source);
        assert_eq!(stub.programs.len(), 1);
        assert_eq!(stub.programs[0].name, "demo");
        assert_eq!(stub.programs[0].instructions, vec!["initialize", "update"]);
    }

    #[test]
    fn from_source_extracts_accounts_struct() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
        let stub = DocumentStub::from_source(source);
        assert_eq!(stub.accounts.len(), 1);
        assert_eq!(stub.accounts[0].name, "Create");
        assert_eq!(stub.accounts[0].fields.len(), 2);
        assert_eq!(stub.accounts[0].fields[0].name, "state");
        assert_eq!(stub.accounts[0].fields[1].name, "user");
    }

    #[test]
    fn from_source_extracts_data_struct() {
        let source = r#"
#[account]
pub struct State {
    pub value: u64,
    pub owner: Pubkey,
}
"#;
        let stub = DocumentStub::from_source(source);
        assert_eq!(stub.data_structs.len(), 1);
        assert_eq!(stub.data_structs[0].name, "State");
        assert_eq!(stub.data_structs[0].fields, vec!["value", "owner"]);
    }

    #[test]
    fn from_source_extracts_pda_seeds() {
        let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        seeds = [b"state", user.key().as_ref()],
        bump
    )]
    pub state: Account<'info, State>,
}
"#;
        let stub = DocumentStub::from_source(source);
        assert_eq!(stub.pda_seeds.len(), 1);
        assert_eq!(stub.pda_seeds[0].account_name, "state");
        assert_eq!(
            stub.pda_seeds[0].seeds,
            vec!["b\"state\"", "user.key().as_ref()"]
        );
    }

    #[test]
    fn from_parsed_matches_from_source() {
        let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
        let document = ParsedDocument::parse_or_empty(source);
        let from_parsed = DocumentStub::from_parsed(&document);
        let from_source = DocumentStub::from_source(source);

        assert_eq!(from_parsed.programs.len(), from_source.programs.len());
        assert_eq!(from_parsed.accounts.len(), from_source.accounts.len());
        assert_eq!(
            from_parsed.data_structs.len(),
            from_source.data_structs.len()
        );
    }

    #[test]
    fn stub_is_clone() {
        let stub = DocumentStub::from_source("#[program] pub mod demo {}");
        let cloned = stub.clone();
        assert_eq!(cloned.programs.len(), 1);
    }
}
