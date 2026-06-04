use super::*;

#[test]
fn reports_native_raw_account_owner_and_type_invariants() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let account = &_accounts[0];
    let data = account.try_borrow_data()?;
    let state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("owner-checks"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("type-cosplay"))
    }));
}

#[test]
fn accepts_native_raw_account_with_owner_and_discriminator_checks() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let account = &_accounts[0];
    if account.owner != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = account.try_borrow_data()?;
    if &data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let state = State::try_from_slice(&data[8..])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(!diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.data.as_ref().and_then(|data| data.get("attack")),
            Some(value) if value == "owner-checks" || value == "type-cosplay"
        )
    }));
}

#[test]
fn reports_pinocchio_unchecked_borrow_without_owner_validation() {
    let source = r#"
use pinocchio::{account_info::AccountInfo, ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let _state = unsafe { load::<State>(account.borrow_data_unchecked())? };
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "owner-checks");
    assert_has_attack(&diagnostics, "type-cosplay");
}

#[test]
fn reports_pinocchio_destructured_account_borrow_in_impl_method() {
    let source = r#"
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};

pub struct Init<'a> {
    pub authority: &'a AccountInfo,
    pub config: &'a AccountInfo,
}

impl<'a> TryFrom<&'a [AccountInfo]> for Init<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountInfo]) -> Result<Self, Self::Error> {
        let [authority, config] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let _state = unsafe { load::<State>(config.borrow_data_unchecked())? };
        Ok(Self { authority, config })
    }
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    let owner = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!("owner-checks"))
        })
        .unwrap_or_else(|| panic!("missing owner-checks: {diagnostics:#?}"));
    assert_eq!(
        owner
            .data
            .as_ref()
            .and_then(|data| data.get("accountIndex")),
        Some(&serde_json::json!(1))
    );
    assert_has_attack(&diagnostics, "type-cosplay");
}

#[test]
fn accepts_pinocchio_is_owned_by_owner_validation() {
    let source = r#"
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    if !account.is_owned_by(&crate::ID) {
        return Err(ProgramError::InvalidAccountData);
    }
    let data = unsafe { account.borrow_data_unchecked() };
    if data[0] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let _state = State::try_from_slice(&data[1..])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_raw_account_text_in_comments_and_strings() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

/// account.try_borrow_data()? and State::try_from_slice(&data)? are docs.
pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let _template = "account.try_borrow_data()?; State::try_from_slice(&data)?";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_raw_account_text_inside_dead_macro() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

macro_rules! dead_account_template {
    () => {
        let data = account.try_borrow_data()?;
        let state = State::try_from_slice(&data)?;
    };
}

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_non_account_try_borrow_data_method() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

struct Buffer;

impl Buffer {
    fn try_borrow_data(&self) -> Result<&[u8], solana_program::program_error::ProgramError> {
        Ok(&[])
    }
}

pub fn process(buffer: Buffer) -> ProgramResult {
    let _data = buffer.try_borrow_data()?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_account_info_substring_inside_non_account_type() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

struct AccountInfoTemplate;

impl AccountInfoTemplate {
    fn try_borrow_data(&self) -> Result<&[u8], solana_program::program_error::ProgramError> {
        Ok(&[])
    }
}

pub fn process(template: AccountInfoTemplate) -> ProgramResult {
    let _data = template.try_borrow_data()?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_deserialize_text_without_raw_account_data_read() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

pub fn process(instruction_data: &[u8]) -> ProgramResult {
    let _state = State::try_from_slice(instruction_data)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn accepts_project_helper_owner_type_signer_program_and_bounds_checks() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    validate_owner(&accounts[0])?;
    validate_type(&accounts[0])?;
    validate_signer(&accounts[0])?;
    validate_program_id(program_id)?;
    validate_instruction_data(instruction_data)?;
    let tag = instruction_data[0];
    let account = &accounts[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    let metas = vec![AccountMeta::new(*account.key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![tag] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    for attack in [
        "owner-checks",
        "type-cosplay",
        "instruction-data-bounds",
        "signer-authorization",
        "arbitrary-cpi",
    ] {
        assert!(
            !diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "unexpected {attack}: {diagnostics:#?}"
        );
    }
}
