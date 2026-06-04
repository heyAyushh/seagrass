use {
    crate::{diagnostics, document::ParsedDocument, project, solana_project, syntax::RustSyntax},
    tower_lsp::lsp_types::Position,
};

pub fn document_parse(source: &str) {
    let _ = ParsedDocument::parse(source);
    let _ = ParsedDocument::parse_or_empty(source);
}

pub fn anchor_attr(tokens: &str) {
    let source = format!(
        r#"
#[derive(Accounts)]
pub struct Fuzz<'info> {{
    #[account({tokens})]
    pub account: AccountInfo<'info>,
}}
"#
    );

    if let Ok(file) = syn::parse_file(&source) {
        for item in &file.items {
            if let syn::Item::Struct(item_struct) = item {
                let _ = anchor_syn::parser::accounts::parse(item_struct);
            }
        }
    }

    let _ = ParsedDocument::parse_or_empty(&source);
    if let Some(syntax) = RustSyntax::parse(&source) {
        let _ = syntax.account_attribute_at_position(
            &source,
            Position {
                line: 3,
                character: 15,
            },
        );
    }
}

pub fn manifest_parse(cargo_toml: &str, anchor_toml: &str, source: &str) {
    let _ = project::parse_anchor_toml(anchor_toml);
    let _ = solana_project::classify_manifest_text(cargo_toml, source);
}

pub fn semantic_diagnostics(source: &str) {
    let document = ParsedDocument::parse_or_empty(source);
    let _ = diagnostics::collect_with_workspace(&document, None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzz_harness_document_parse_smoke() {
        document_parse("#[account(init]\n");
    }

    #[test]
    fn fuzz_harness_anchor_attr_smoke() {
        anchor_attr(r#"init, payer = user, seeds = [b"state","#);
    }

    #[test]
    fn fuzz_harness_manifest_parse_smoke() {
        manifest_parse(
            r#"
[package]
name = "demo"

[lib]
crate-type = ["cdylib"]

[dependencies]
solana-program = "1"
"#,
            r#"
[provider]
cluster = "localnet"

[programs.localnet]
demo = "11111111111111111111111111111111"
"#,
            "entrypoint!(process_instruction);",
        );
    }

    #[test]
    fn fuzz_harness_semantic_diagnostics_smoke() {
        semantic_diagnostics(
            r#"
pub fn process_instruction() {
    let account_data = account.try_borrow_data().unwrap();
    let signer = accounts[0].key();
}
"#,
        );
    }
}
