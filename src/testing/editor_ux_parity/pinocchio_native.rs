use super::*;

#[test]
fn editor_ux_surfaces_pinocchio_native_security_metadata() {
    let source = r#"
use {
    core::slice::from_raw_parts,
    pinocchio::{
        account_info::AccountInfo,
        cpi::invoke,
        instruction::{InstructionAccount, InstructionView},
        pubkey::Pubkey,
        ProgramResult,
    },
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let data = unsafe { account.borrow_data_unchecked() };
    let _state = State::try_from_slice(data)?;
    let instruction_accounts = [InstructionAccount::writable_signer(account.key())];
    let instruction = InstructionView {
        program_id,
        accounts: &instruction_accounts,
        data: unsafe { from_raw_parts([0].as_ptr(), 1) },
    };
    invoke(&instruction, &[account])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert_editor_diagnostic_metadata(
        &diagnostics,
        "owner-checks",
        "seagrass/security.owner-check",
        "add-owner-check",
        Some("pinocchio"),
    );
    assert_editor_diagnostic_metadata(
        &diagnostics,
        "type-cosplay",
        "seagrass/security.type-cosplay",
        "add-discriminator-check",
        Some("pinocchio"),
    );
    assert_editor_diagnostic_metadata(
        &diagnostics,
        "arbitrary-cpi",
        "seagrass/security.cpi.program",
        "add-program-id-check",
        Some("pinocchio"),
    );
    assert_editor_diagnostic_metadata(
        &diagnostics,
        "writable-account",
        "seagrass/security.writable-account",
        "add-writable-check",
        Some("pinocchio"),
    );
    assert_editor_diagnostic_metadata(
        &diagnostics,
        "signer-authorization",
        "seagrass/security.signer.authorization",
        "add-signer-check",
        Some("pinocchio"),
    );
}

#[test]
fn editor_ux_keeps_pinocchio_security_validation_account_scoped() {
    let source = r#"
use pinocchio::{
    account_info::AccountInfo,
    cpi::invoke,
    instruction::{InstructionAccount, InstructionView},
    pubkey::Pubkey,
    ProgramResult,
};

pub fn process(program_id: &Pubkey, external_program: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    if program_id != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    if !accounts[0].is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if !accounts[0].is_owned_by(&crate::ID) {
        return Err(ProgramError::IncorrectProgramId);
    }
    let first_data = unsafe { accounts[0].borrow_data_unchecked() };
    if &first_data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let second_data = unsafe { accounts[1].borrow_data_unchecked() };
    let _state = State::try_from_slice(second_data)?;
    let instruction_accounts = [InstructionAccount::writable_signer(accounts[1].key())];
    let instruction = InstructionView {
        program_id: external_program,
        accounts: &instruction_accounts,
        data: &[],
    };
    invoke(&instruction, &[&accounts[1]])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = diagnostics::collect(&document);

    assert_eq!(
        diagnostic_attack_count(&diagnostics, "signer-authorization"),
        1
    );
    assert_eq!(diagnostic_attack_count(&diagnostics, "owner-checks"), 1);
    assert_eq!(diagnostic_attack_count(&diagnostics, "type-cosplay"), 1);
    assert_eq!(diagnostic_attack_count(&diagnostics, "arbitrary-cpi"), 1);
    assert_editor_diagnostic_metadata(
        &diagnostics,
        "type-cosplay",
        "seagrass/security.type-cosplay",
        "add-discriminator-check",
        Some("pinocchio"),
    );
}

fn assert_editor_diagnostic_metadata(
    diagnostics: &[Diagnostic],
    attack: &str,
    topic: &str,
    quickfix: &str,
    program_kind: Option<&str>,
) {
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic_data_str(diagnostic, "attack") == Some(attack))
        .unwrap_or_else(|| panic!("missing {attack}: {diagnostics:#?}"));
    assert_eq!(diagnostic_data_str(diagnostic, "topic"), Some(topic));
    assert_eq!(diagnostic_data_str(diagnostic, "quickfix"), Some(quickfix));
    if let Some(expected_program_kind) = program_kind {
        assert_eq!(
            diagnostic_data_str(diagnostic, "programKind"),
            Some(expected_program_kind)
        );
    }
    assert!(
        diagnostic.code_description.is_some(),
        "expected lint doc URL for {attack}: {diagnostic:#?}"
    );
}

fn diagnostic_attack_count(diagnostics: &[Diagnostic], attack: &str) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic_data_str(diagnostic, "attack") == Some(attack))
        .count()
}
