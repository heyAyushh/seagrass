use seagrass_framework::{
    diagnostics::{FrameworkDocument, LspDiagnostic},
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct PinocchioFramework;

const FRAMEWORK_ID: &str = "pinocchio";
const DISPLAY_NAME: &str = "Pinocchio";

pub const SOURCES: &[SourceDescriptor] = &[SourceDescriptor {
    id: "cargo-meta",
    description: "Pinocchio dependency and source detection",
    path_patterns: &["Cargo.toml", "src/**/*.rs"],
}];

impl Framework for PinocchioFramework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::Pinocchio,
            support_level: SupportLevel::Stable,
            generated: GeneratedCatalogStats {
                constraints: 0,
                field_completions: 0,
                errors: 0,
            },
        }
    }

    fn sources(&self) -> &'static [SourceDescriptor] {
        SOURCES
    }
}

pub fn framework() -> PinocchioFramework {
    PinocchioFramework
}

pub fn diagnostics(document: FrameworkDocument<'_>) -> Vec<LspDiagnostic> {
    seagrass_framework::native_rules::diagnostics(document, FrameworkKind::Pinocchio)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        seagrass_framework::diagnostics::{SOLANA_CODE_QUALITY_CODE, SOURCE as DIAGNOSTIC_SOURCE},
        tower_lsp::lsp_types::NumberOrString,
    };

    fn collect(source: &str) -> Vec<LspDiagnostic> {
        let syntax = syn::parse_file(source).expect("valid test rust");
        diagnostics(FrameworkDocument::new(source, &syntax))
    }

    fn assert_has_attack(diagnostics: &[LspDiagnostic], attack: &str) {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "missing {attack}: {diagnostics:#?}"
        );
    }

    fn attack_count(diagnostics: &[LspDiagnostic], attack: &str) -> usize {
        diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            })
            .count()
    }

    #[test]
    fn pinocchio_metadata_marks_equivalent_support_stable() {
        let metadata = framework().metadata();

        assert_eq!(metadata.kind, FrameworkKind::Pinocchio);
        assert_eq!(metadata.support_level, SupportLevel::Stable);
    }

    #[test]
    fn pinocchio_framework_surfaces_raw_account_invariants() {
        let source = r#"
use pinocchio::{account_info::AccountInfo, ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let _state = unsafe { load::<State>(account.borrow_data_unchecked())? };
    Ok(())
}
"#;
        let diagnostics = collect(source);

        assert_has_attack(&diagnostics, "owner-checks");
        assert_has_attack(&diagnostics, "type-cosplay");
        assert!(diagnostics.iter().all(|diagnostic| {
            diagnostic.source.as_deref() == Some(DIAGNOSTIC_SOURCE)
                && diagnostic.code
                    == Some(NumberOrString::String(SOLANA_CODE_QUALITY_CODE.to_string()))
                && diagnostic.code_description.is_some()
        }));
    }

    #[test]
    fn pinocchio_framework_surfaces_cpi_and_signer_invariants() {
        let source = r#"
use pinocchio::{
    cpi::invoke,
    instruction::{InstructionAccount, InstructionView},
    AccountView,
    Address,
    ProgramResult,
};

pub struct Transfer<'a> {
    pub source: &'a AccountView,
    pub token_program: &'a Address,
}

impl<'a> Transfer<'a> {
    pub fn invoke(&self) -> ProgramResult {
        let instruction_accounts = [InstructionAccount::writable_signer(self.source.address())];
        let instruction = InstructionView {
            program_id: self.token_program,
            accounts: &instruction_accounts,
            data: &[],
        };
        invoke(&instruction, &[self.source])?;
        Ok(())
    }
}
"#;
        let diagnostics = collect(source);

        assert_has_attack(&diagnostics, "signer-authorization");
        assert_has_attack(&diagnostics, "arbitrary-cpi");
    }

    #[test]
    fn pinocchio_framework_keeps_validation_account_scoped() {
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
        let diagnostics = collect(source);

        assert_eq!(attack_count(&diagnostics, "signer-authorization"), 1);
        assert_eq!(attack_count(&diagnostics, "owner-checks"), 1);
        assert_eq!(attack_count(&diagnostics, "type-cosplay"), 1);
        assert_eq!(attack_count(&diagnostics, "arbitrary-cpi"), 1);
    }
}
