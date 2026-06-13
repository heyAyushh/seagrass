use {
    super::*,
    crate::{
        diagnostics::FrameworkDocument,
        semantic::{queries, CheckKind, PopulatedFields},
        FrameworkKind,
    },
};

fn model_for(source: &str) -> crate::semantic::SemanticModel {
    let syntax = syn::parse_file(source).expect("valid test rust");
    extract_native(
        FrameworkDocument::new(source, &syntax),
        FrameworkKind::Native,
    )
}

#[test]
fn native_entrypoint_extracts_next_account_info_fields_in_order() {
    let model = model_for(
        r#"
use solana_account_info::{next_account_info, AccountInfo};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let payer = next_account_info(account_iter)?;
    let vault = next_account_info(account_iter)?;
    Ok(())
}
"#,
    );

    let fields = &model.accounts_structs[0].inner.fields;
    let names = fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(names, ["payer", "vault"]);
}

#[test]
fn native_signer_check_is_populated() {
    let model = model_for(
        r#"
use solana_account_info::{next_account_info, AccountInfo};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let payer = next_account_info(account_iter)?;
    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}
"#,
    );
    let instruction = &model.instructions[0];

    assert!(instruction
        .populated_fields
        .has(PopulatedFields::SIGNER_CHECKS));
    assert!(instruction.inner.signer_checks.iter().any(|check| {
        check.kind == CheckKind::Signer && check.subject_ref.as_deref() == Some("payer")
    }));
}

#[test]
fn native_query_stays_silent_without_populated_signer_checks() {
    let model = model_for(
        r#"
use solana_account_info::{next_account_info, AccountInfo};

pub fn process(accounts: &[AccountInfo]) -> ProgramResult {
    let account_iter = &mut accounts.iter();
    let payer = next_account_info(account_iter)?;
    let _ = payer.key;
    Ok(())
}
"#,
    );

    let diagnostics = queries::missing_signer_diagnostics(&model);

    assert!(diagnostics.is_empty());
}
