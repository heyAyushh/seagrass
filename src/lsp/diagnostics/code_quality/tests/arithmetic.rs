use super::*;

#[test]
fn reports_unchecked_balance_arithmetic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Use checked arithmetic")));
}

#[test]
fn ignores_deref_in_account_attribute() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Interface, TokenInterface};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(address = *token_mint_a.to_account_info().owner)]
    pub token_program_a: Interface<'info, TokenInterface>,
    #[account(address = *token_mint_b.to_account_info().owner)]
    pub token_program_b: Interface<'info, TokenInterface>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_unary_deref_in_executable_body() {
    let source = r#"
use solana_program::program_error::ProgramError;

fn read(ptr: &u64) -> Result<u64, ProgramError> {
    Ok(*ptr)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_token_word_inside_unrelated_identifier() {
    let source = r#"
use solana_program::program_error::ProgramError;

fn process(token_mint_a: u64, other: u64) -> Result<(), ProgramError> {
    let _difference = token_mint_a - other;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_arithmetic_text_in_comments_and_strings() {
    let source = r#"
use solana_program::program_error::ProgramError;

/// amount - fee is checked by caller.
fn process() -> Result<(), ProgramError> {
    let message = "token - fee";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn diagnostic_source_is_seagrass() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostic = collect(&document)
        .into_iter()
        .find(|diagnostic| diagnostic.message.contains("Use checked arithmetic"))
        .expect("expected unchecked arithmetic diagnostic");

    assert_eq!(diagnostic.source.as_deref(), Some("seagrass"));
    let data = diagnostic.data.as_ref().expect("expected diagnostic data");
    assert_eq!(data["rule"], "unchecked-arithmetic");
    assert_eq!(data["confidence"], "heuristic");
    assert_eq!(
        data["topic"],
        "seagrass/solana.code-quality.unchecked-arithmetic"
    );
    assert_eq!(data["applicability"], "Unspecified");
}

#[test]
fn resolved_framework_context_drives_code_quality_without_rescanning_imports() {
    let source = r#"
fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect_with_framework(
        &document,
        crate::solana::frameworks::FrameworkContext::new(
            crate::solana::frameworks::FrameworkId::Pinocchio,
        ),
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Use checked arithmetic"))
        .expect("expected unchecked arithmetic diagnostic from resolved framework context");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("programKind")),
        Some(&serde_json::json!("pinocchio"))
    );
}

#[test]
fn unchecked_arithmetic_declares_lint_contract() {
    assert_eq!(
        unchecked_arithmetic_scope(),
        &[
            crate::diagnostics::lint::Region::InstructionBody,
            crate::diagnostics::lint::Region::HelperFnBody,
        ]
    );
    assert_eq!(unchecked_arithmetic_confidence().as_str(), "heuristic");
    assert_eq!(unchecked_arithmetic_applicability().as_str(), "Unspecified");
    assert_eq!(
        unchecked_arithmetic_topic(),
        "seagrass/solana.code-quality.unchecked-arithmetic"
    );
}

#[test]
fn ignores_checked_balance_arithmetic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    amount.checked_sub(fee).ok_or(ProgramError::InvalidArgument)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}

fn assert_no_checked_arithmetic_diagnostic(document: &ParsedDocument) {
    assert!(!collect(document)
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Use checked arithmetic")));
}
