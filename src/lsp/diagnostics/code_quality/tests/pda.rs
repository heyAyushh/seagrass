use super::*;

#[test]
fn reports_non_canonical_pda_bump() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn set_value(ctx: Context<SetValue>, key: u64, bump: u8) -> ProgramResult {
    let address = Pubkey::create_program_address(&[key.to_le_bytes().as_ref(), &[bump]], ctx.program_id)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("canonical PDA bump"))
        .expect("expected PDA bump canonicalization diagnostic");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("solana-code-quality".to_string()))
    );
    let data = diagnostic.data.as_ref().expect("expected diagnostic data");
    assert_eq!(data["attack"], "bump-seed-canonicalization");
    assert_eq!(data["corpusMode"], "legacy-invariant");
    assert_eq!(data["absorbedFrom"], "coral-xyz/sealevel-attacks");
}

#[test]
fn ignores_canonical_find_program_address() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn set_value(ctx: Context<SetValue>, key: u64) -> ProgramResult {
    let (_address, _bump) = Pubkey::find_program_address(&[key.to_le_bytes().as_ref()], ctx.program_id);
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}

#[test]
fn ignores_create_program_address_text_in_comments_strings_and_attributes() {
    let source = r#"
use anchor_lang::prelude::*;

// Pubkey::create_program_address accepts any bump; do not lint comments.
pub fn note() -> Result<()> {
    let _template = "Pubkey::create_program_address(&[seed], program_id)";
    Ok(())
}

#[derive(Accounts)]
pub struct SetValue<'info> {
    #[account(constraint = note == "create_program_address")]
    pub note: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "bump-seed-canonicalization");
}

#[test]
fn ignores_project_helper_named_create_program_address() {
    let source = r#"
use anchor_lang::prelude::*;

mod helpers {
    use anchor_lang::prelude::*;

    pub fn create_program_address(_seed: &[u8]) -> Pubkey {
        Pubkey::default()
    }
}

pub fn set_value(seed: &[u8]) -> Result<()> {
    let _address = helpers::create_program_address(seed);
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "bump-seed-canonicalization");
}
