use seagrass_framework::{
    diagnostics::{FrameworkDocument, LspDiagnostic},
    Framework, FrameworkKind, FrameworkMetadata, GeneratedCatalogStats, SourceDescriptor,
    SupportLevel,
};

pub struct NativeSolanaFramework;

const FRAMEWORK_ID: &str = "native";
const DISPLAY_NAME: &str = "Native Solana";

pub const SOURCES: &[SourceDescriptor] = &[SourceDescriptor {
    id: "cargo-meta",
    description: "Native Solana dependency and source detection",
    path_patterns: &["Cargo.toml", "src/**/*.rs"],
}];

impl Framework for NativeSolanaFramework {
    fn metadata(&self) -> FrameworkMetadata {
        FrameworkMetadata {
            id: FRAMEWORK_ID,
            display_name: DISPLAY_NAME,
            kind: FrameworkKind::Native,
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

pub fn framework() -> NativeSolanaFramework {
    NativeSolanaFramework
}

pub fn diagnostics(document: FrameworkDocument<'_>) -> Vec<LspDiagnostic> {
    seagrass_framework::native_rules::diagnostics(document, FrameworkKind::Native)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn assert_no_attack(diagnostics: &[LspDiagnostic], attack: &str) {
        assert!(
            !diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "unexpected {attack}: {diagnostics:#?}"
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
    fn native_metadata_marks_equivalent_support_stable() {
        let metadata = framework().metadata();

        assert_eq!(metadata.kind, FrameworkKind::Native);
        assert_eq!(metadata.support_level, SupportLevel::Stable);
    }

    #[test]
    fn native_framework_surfaces_raw_account_invariants() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
        let diagnostics = collect(source);

        assert_has_attack(&diagnostics, "owner-checks");
        assert_has_attack(&diagnostics, "type-cosplay");
    }

    #[test]
    fn native_framework_accepts_visible_owner_and_type_checks() {
        let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account = &accounts[0];
    if account.owner != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = account.try_borrow_data()?;
    if &data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let _state = State::try_from_slice(&data[8..])?;
    Ok(())
}
"#;
        let diagnostics = collect(source);

        assert_no_attack(&diagnostics, "owner-checks");
        assert_no_attack(&diagnostics, "type-cosplay");
    }

    #[test]
    fn native_framework_surfaces_cpi_and_signer_invariants() {
        let source = r#"
use {
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let metas = vec![AccountMeta::new(*accounts[0].key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
        let diagnostics = collect(source);

        assert_has_attack(&diagnostics, "signer-authorization");
        assert_has_attack(&diagnostics, "arbitrary-cpi");
    }

    #[test]
    fn native_framework_keeps_validation_account_scoped() {
        let source = r#"
use {
    solana_account_info::AccountInfo,
    solana_cpi::invoke,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
};

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    if !accounts[0].is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if accounts[0].owner != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let first_data = accounts[0].try_borrow_data()?;
    if &first_data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let second_data = accounts[1].try_borrow_data()?;
    let _state = State::try_from_slice(&second_data)?;
    let metas = vec![AccountMeta::new(*accounts[1].key, true)];
    let ix = Instruction { program_id: crate::ID, accounts: metas, data: vec![] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
        let diagnostics = collect(source);

        assert_eq!(attack_count(&diagnostics, "signer-authorization"), 1);
        assert_eq!(attack_count(&diagnostics, "owner-checks"), 1);
        assert_eq!(attack_count(&diagnostics, "type-cosplay"), 1);
    }
}
