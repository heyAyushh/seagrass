use {
    crate::{diagnostics::FrameworkDocument, FrameworkKind},
    tower_lsp::lsp_types::Diagnostic,
};

mod common;
mod raw_account;
mod validation;

pub fn diagnostics(
    document: FrameworkDocument<'_>,
    framework_kind: FrameworkKind,
) -> Vec<Diagnostic> {
    if !matches!(
        framework_kind,
        FrameworkKind::Native | FrameworkKind::Pinocchio
    ) {
        return Vec::new();
    }

    let mut diagnostics = raw_account::diagnostics(document, framework_kind);
    diagnostics.extend(validation::diagnostics(document, framework_kind));
    diagnostics
}

#[cfg(test)]
mod tests {
    use {super::*, crate::diagnostics::FrameworkDocument};

    fn parse(source: &str) -> syn::File {
        syn::parse_file(source).expect("valid test rust")
    }

    fn collect(source: &str, framework_kind: FrameworkKind) -> Vec<Diagnostic> {
        let syntax = parse(source);
        diagnostics(FrameworkDocument::new(source, &syntax), framework_kind)
    }

    fn assert_has_attack(diagnostics: &[Diagnostic], attack: &str) {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "missing {attack}: {diagnostics:#?}"
        );
    }

    fn attack_count(diagnostics: &[Diagnostic], attack: &str) -> usize {
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            })
            .count()
    }

    #[test]
    fn skips_anchor_frameworks() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let data = accounts[0].try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::AnchorV1);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn emits_framework_diagnostic_contract() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let data = accounts[0].try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::Native);

        assert_has_attack(&diagnostics, "owner-checks");
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.source.as_deref() == Some(crate::diagnostics::SOURCE)
                && diagnostic.code_description.is_some()
        }));
    }

    #[test]
    fn reports_unvalidated_native_signer_when_different_account_was_checked() {
        let source = r#"
use {
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    if !accounts[0].is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let metas = vec![AccountMeta::new(*accounts[1].key, true)];
    let ix = Instruction { program_id: crate::ID, accounts: metas, data: vec![] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::Native);

        assert_eq!(attack_count(&diagnostics, "signer-authorization"), 1);
    }

    #[test]
    fn reports_unvalidated_native_raw_read_when_different_account_owner_was_checked() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    if accounts[0].owner != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = accounts[1].try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::Native);

        assert_eq!(attack_count(&diagnostics, "owner-checks"), 1);
    }

    #[test]
    fn reports_unvalidated_native_type_decode_when_different_account_type_was_checked() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let first_data = accounts[0].try_borrow_data()?;
    if &first_data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let second_data = accounts[1].try_borrow_data()?;
    let _state = State::try_from_slice(&second_data)?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::Native);

        assert_eq!(attack_count(&diagnostics, "type-cosplay"), 1);
    }

    #[test]
    fn reports_unvalidated_pinocchio_cpi_when_different_program_was_checked() {
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
    let instruction_accounts = [InstructionAccount::writable(accounts[0].key())];
    let instruction = InstructionView {
        program_id: external_program,
        accounts: &instruction_accounts,
        data: &[],
    };
    invoke(&instruction, &[&accounts[0]])?;
    Ok(())
}
"#;
        let diagnostics = collect(source, FrameworkKind::Pinocchio);

        assert_eq!(attack_count(&diagnostics, "arbitrary-cpi"), 1);
    }
}
