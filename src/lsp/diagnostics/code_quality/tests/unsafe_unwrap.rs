use {
    super::*,
    tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString},
};

#[test]
fn reports_unsafe_unwrap_in_anchor_code() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn run(_ctx: Context<Run>) -> Result<()> {
        let value = maybe_value.unwrap();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Run<'info> {
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Avoid `.unwrap()`"))
        .expect("expected unsafe unwrap diagnostic");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("solana-code-quality".to_string()))
    );
}

#[test]
fn ignores_fixed_slice_try_into_unwrap() {
    let source = r#"
use solana_program::program_error::ProgramError;

fn parse(data: &[u8]) -> Result<[u8; 8], ProgramError> {
    Ok(data[0..8].try_into().unwrap())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}

#[test]
fn ignores_unwrap_text_in_comments_strings_and_attributes() {
    let source = r#"
use anchor_lang::prelude::*;

#[doc = "maybe_value.unwrap()"]
pub fn process() -> Result<()> {
    // maybe_value.unwrap()
    let _template = "maybe_value.expect(\"value\")";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}
