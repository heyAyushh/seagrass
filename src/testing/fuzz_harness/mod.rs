use {
    crate::{
        anchor_preflight::{
            AccountEvidence, InstructionEvidence, InvocationEvidence, ReallocEvidence,
        },
        diagnostics,
        document::ParsedDocument,
        project, solana_project,
        syntax::RustSyntax,
    },
    tower_lsp::lsp_types::Position,
};

const DISCRIMINATOR_BYTES: usize = 8;
const FUZZ_PROGRAM_ID: &str = "Fuzz1111111111111111111111111111111111111";
const FUZZ_OTHER_PROGRAM_ID: &str = "Other11111111111111111111111111111111111";
const MAX_FUZZ_ACCOUNTS: usize = 4;
const MAX_FUZZ_REALLOCS: usize = 4;

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

pub fn anchor_preflight(data: &[u8]) {
    let mut cursor = FuzzCursor::new(data);
    let expected_accounts = cursor.next_usize(MAX_FUZZ_ACCOUNTS);
    let provided_accounts = cursor.next_usize(MAX_FUZZ_ACCOUNTS);
    let instruction = InstructionEvidence {
        data_len: cursor.next_optional_usize(DISCRIMINATOR_BYTES * 2),
        deserializes: cursor.next_optional_bool(),
        discriminator_matches_known_instruction: cursor.next_optional_bool(),
        fallback_supported: cursor.next_optional_bool(),
        expected_program_id: cursor.next_optional_program_id(FUZZ_PROGRAM_ID),
        provided_program_id: cursor.next_optional_program_id(FUZZ_OTHER_PROGRAM_ID),
        expected_account_count: Some(expected_accounts),
        provided_account_count: Some(provided_accounts),
    };

    let account_count = cursor.next_usize(MAX_FUZZ_ACCOUNTS);
    let mut accounts = Vec::with_capacity(account_count);
    for _ in 0..account_count {
        accounts.push(AccountEvidence {
            data_len: cursor.next_optional_usize(DISCRIMINATOR_BYTES * 2),
            expected_discriminator: cursor.next_optional_discriminator(),
            actual_discriminator: cursor.next_optional_discriminator(),
            expected_owner: cursor.next_optional_program_id(FUZZ_PROGRAM_ID),
            actual_owner: cursor.next_optional_program_id(FUZZ_OTHER_PROGRAM_ID),
            initialized: cursor.next_optional_bool(),
            deserializes: cursor.next_optional_bool(),
            expected_zero_discriminator_for_init: cursor.next_optional_bool(),
        });
    }

    let realloc_count = cursor.next_usize(MAX_FUZZ_REALLOCS);
    let mut reallocs = Vec::with_capacity(realloc_count);
    for _ in 0..realloc_count {
        reallocs.push(ReallocEvidence {
            requested_data_increase: cursor.next_usize(16_384),
        });
    }

    let evidence = InvocationEvidence {
        instruction,
        accounts: &accounts,
        reallocs: &reallocs,
    };
    let _ = crate::anchor_preflight::detected_errors(&evidence);
}

struct FuzzCursor<'a> {
    data: &'a [u8],
    index: usize,
}

impl<'a> FuzzCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, index: 0 }
    }

    fn next_byte(&mut self) -> u8 {
        let byte = self.data.get(self.index).copied().unwrap_or_default();
        self.index = self.index.saturating_add(1);
        byte
    }

    fn next_bool(&mut self) -> bool {
        self.next_byte() & 1 == 1
    }

    fn next_optional_bool(&mut self) -> Option<bool> {
        self.next_bool().then(|| self.next_bool())
    }

    fn next_usize(&mut self, max_inclusive: usize) -> usize {
        usize::from(self.next_byte()) % max_inclusive.saturating_add(1)
    }

    fn next_optional_usize(&mut self, max_inclusive: usize) -> Option<usize> {
        self.next_bool().then(|| self.next_usize(max_inclusive))
    }

    fn next_optional_program_id(&mut self, program_id: &'static str) -> Option<&'static str> {
        self.next_bool().then_some(program_id)
    }

    fn next_optional_discriminator(&mut self) -> Option<[u8; DISCRIMINATOR_BYTES]> {
        if !self.next_bool() {
            return None;
        }
        let mut discriminator = [0u8; DISCRIMINATOR_BYTES];
        for byte in &mut discriminator {
            *byte = self.next_byte();
        }
        Some(discriminator)
    }
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

    #[test]
    fn fuzz_harness_anchor_preflight_smoke() {
        anchor_preflight(b"\x03\x02\x01\x08\x01\x00\x01\x01\x01\x01\x01\x02\x01abcdefgh");
    }
}
