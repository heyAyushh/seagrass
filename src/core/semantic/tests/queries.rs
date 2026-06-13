use {
    super::*,
    std::collections::{HashMap, HashSet},
    tower_lsp::lsp_types::{NumberOrString, Range},
};

#[test]
fn missing_signer_query_reports_when_checks_populated() {
    let model = model_with_signer_checks(PopulatedFields(PopulatedFields::SIGNER_CHECKS));

    let diagnostics = crate::semantic::queries::missing_signer_diagnostics(&model);

    assert!(diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.code.as_ref(),
        Some(NumberOrString::String(code)) if code == "anchor-security-signer"
    )));
}

#[test]
fn missing_signer_query_stays_silent_without_checks_populated() {
    let model = model_with_signer_checks(PopulatedFields::default());

    let diagnostics = crate::semantic::queries::missing_signer_diagnostics(&model);

    assert!(diagnostics.is_empty());
}

#[test]
fn missing_signer_query_uses_reachable_checks() {
    let model = model_with_signer_checks(PopulatedFields(PopulatedFields::SIGNER_CHECKS));
    let reachable_checks =
        HashMap::from([("process".to_string(), HashSet::from(["payer".to_string()]))]);

    let diagnostics = crate::semantic::queries::missing_signer_diagnostics_with_reachability(
        &model,
        &reachable_checks,
    );

    assert!(diagnostics.is_empty());
}

fn model_with_signer_checks(instruction_fields: PopulatedFields) -> SemanticModel {
    SemanticModel {
        instructions: vec![Populated::with_fields(
            Instruction {
                name: "process".to_string(),
                context_type: Some("Process".to_string()),
                parameters: Vec::new(),
                cpi_calls: Vec::new(),
                signer_checks: Vec::new(),
                owner_checks: Vec::new(),
                discriminator_checks: Vec::new(),
            },
            ExtractionConfidence::MacroAnnounced,
            instruction_fields,
        )],
        accounts_structs: vec![Populated::new(
            AccountsStruct {
                name: "Process".to_string(),
                fields: vec![AccountField {
                    name: "payer".to_string(),
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
