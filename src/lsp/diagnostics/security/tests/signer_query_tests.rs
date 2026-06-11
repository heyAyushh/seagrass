use {
    super::*,
    crate::{
        anchor::extractor,
        diagnostics::registry::ANCHOR_SECURITY_SIGNER_CODE,
        semantic::{
            AccountField, AccountType, AccountsStruct, Check, CheckKind, ExtractionConfidence,
            Instruction, Populated, PopulatedFields, SemanticModel,
        },
    },
    tower_lsp::lsp_types::Range,
};

#[test]
fn model_with_signer_checks_populated_reports_missing_signer() {
    let model = signer_model(PopulatedFields(PopulatedFields::SIGNER_CHECKS), Vec::new());

    let diagnostics = super::super::signer_query::missing_signer_diagnostics(&model);

    assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn model_without_signer_checks_populated_stays_silent() {
    let model = signer_model(PopulatedFields::default(), Vec::new());

    let diagnostics = super::super::signer_query::missing_signer_diagnostics(&model);

    assert!(diagnostics.is_empty());
}

#[test]
fn model_with_runtime_signer_check_stays_silent() {
    let model = signer_model(
        PopulatedFields(PopulatedFields::SIGNER_CHECKS),
        vec![signer_check("authority")],
    );

    let diagnostics = super::super::signer_query::missing_signer_diagnostics(&model);

    assert!(diagnostics.is_empty());
}

#[test]
fn anchor_source_with_explicit_signer_constraint_stays_silent() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn authorize(ctx: Context<Authorize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Authorize<'info> {
    #[account(constraint = authority.is_signer)]
    pub authority: UncheckedAccount<'info>,
}
"#,
    )
    .unwrap();
    let model = extractor::extract(&document);

    let diagnostics = super::super::signer_query::missing_signer_diagnostics(&model);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
}

fn signer_model(instruction_fields: PopulatedFields, signer_checks: Vec<Check>) -> SemanticModel {
    SemanticModel {
        instructions: vec![Populated::with_fields(
            Instruction {
                name: "authorize".to_string(),
                context_type: Some("Authorize".to_string()),
                parameters: Vec::new(),
                cpi_calls: Vec::new(),
                signer_checks,
                owner_checks: Vec::new(),
                discriminator_checks: Vec::new(),
            },
            ExtractionConfidence::MacroAnnounced,
            instruction_fields,
        )],
        accounts_structs: vec![Populated::new(
            AccountsStruct {
                name: "Authorize".to_string(),
                fields: vec![AccountField {
                    name: "authority".to_string(),
                    source_range: Range::default(),
                    account_type: AccountType::UncheckedAccount,
                    constraints: Vec::new(),
                    token_interface_candidate: false,
                }],
                composite_refs: Vec::new(),
            },
            ExtractionConfidence::MacroAnnounced,
        )],
        ..SemanticModel::default()
    }
}

fn signer_check(name: &str) -> Check {
    Check {
        kind: CheckKind::Signer,
        subject_ref: Some(name.to_string()),
        source_range: Range::default(),
    }
}
