use super::*;

#[test]
fn ignores_native_account_validation_text_in_comments_and_strings() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

/// AccountMeta::new(*accounts[0].key, true); invoke(&ix, accounts); program_id
pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let _template = "AccountMeta::new(*accounts[0].key, true); Instruction { program_id }; invoke";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "signer-authorization");
    assert_no_attack(&diagnostics, "arbitrary-cpi");
}

#[test]
fn reports_native_signer_even_with_unrelated_signer_helper() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::AccountMeta,
};

pub fn helper(account: &AccountInfo) -> ProgramResult {
    validate_signer(account)?;
    Ok(())
}

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let _metas = vec![AccountMeta::new(*accounts[0].key, true)];
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!("signer-authorization"))
        })
        .unwrap_or_else(|| panic!("missing signer-authorization: {diagnostics:#?}"));
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("accountIndex")),
        Some(&serde_json::json!(0))
    );
}

#[test]
fn reports_native_arbitrary_cpi_even_with_unrelated_program_id_helper() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

pub fn helper(program_id: &Pubkey) -> ProgramResult {
    validate_program_id(program_id)?;
    Ok(())
}

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let metas = vec![AccountMeta::new_readonly(*accounts[0].key, false)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "arbitrary-cpi");
}

#[test]
fn accepts_native_account_validation_in_same_function() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    validate_signer(&accounts[0])?;
    validate_program_id(program_id)?;
    let metas = vec![AccountMeta::new(*accounts[0].key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "signer-authorization");
    assert_no_attack(&diagnostics, "arbitrary-cpi");
}

#[test]
fn ignores_account_meta_false_when_true_exists_elsewhere() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::AccountMeta,
};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let enabled = true;
    let _metas = vec![AccountMeta::new(*accounts[0].key, false)];
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "signer-authorization");
}

#[test]
fn ignores_pda_signer_account_meta_with_invoke_signed() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let pda = &accounts[0];
    let metas = vec![AccountMeta::new(*pda.key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![] };
    invoke_signed(&ix, accounts, &[&[b"vault", &[255]]])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "signer-authorization");
}

#[test]
fn ignores_known_program_id_instruction() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let metas = vec![AccountMeta::new_readonly(*accounts[0].key, false)];
    let _ix = Instruction { program_id: system_program::ID, accounts: metas, data: vec![] };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "arbitrary-cpi");
}

#[test]
fn ignores_sdk_built_instruction_when_program_id_param_exists() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::invoke,
    pubkey::Pubkey,
    system_instruction,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let _ = program_id;
    invoke(&system_instruction::transfer(accounts[0].key, accounts[1].key, 1), accounts)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "arbitrary-cpi");
}
