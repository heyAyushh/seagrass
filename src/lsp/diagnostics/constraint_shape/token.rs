use {
    crate::{
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        evidence::{AccountSetEvidence, ConstraintEvidence, FieldEvidence},
    },
    tower_lsp::lsp_types::Diagnostic,
};

const TOKEN_PROGRAM_EXPECTED_TYPE: &str = "Program<'info, Token>, Program<'info, Token2022>, Program<'info, TokenInterface>, or Interface<'info, TokenInterface>";
const DEFAULT_TOKEN_PROGRAM_QUICKFIX_TYPE: &str = "Program<'info, Token>";
const INTERFACE_TOKEN_PROGRAM_QUICKFIX_TYPE: &str = "Interface<'info, TokenInterface>";

pub(super) fn constraint_diagnostics(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(spl_init_space_diagnostic(field, constraint));
    diagnostics.extend(mint_init_diagnostics(field, constraint));
    diagnostics.extend(token_account_init_diagnostics(field, constraint));
    diagnostics.extend(token_interface_mint_reference_diagnostic(
        accounts, field, constraint,
    ));
    diagnostics.extend(token_program_init_diagnostics(accounts, field, constraint));
    diagnostics
}
fn mint_init_diagnostics(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    if !constraint.has_init_like_constraint() || !field.has_generic_type("Mint") {
        return Vec::new();
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    let mut diagnostics = Vec::new();
    for missing in ["mint::decimals", "mint::authority"] {
        if !constraint.has_key(missing) {
            diagnostics.push(diagnostic_from_range(
                constraint.range(),
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` initializes a token mint but is missing `{missing} = ...`.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": init_key,
                    "missing": missing,
                })),
            ));
        }
    }
    diagnostics
}
fn spl_init_space_diagnostic(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if !constraint.has_init_like_constraint()
        || !constraint.has_key("space")
        || !initializes_spl_account_with_anchor_space(constraint)
    {
        return None;
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    Some(diagnostic_from_range(
        constraint.range(),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `{init_key}` with SPL token initialization constraints; Anchor derives SPL account space from the token program, so `space` is not required.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "constraint": "space",
            "requiredBy": init_key,
            "quickfix": "remove-conflicting-constraints",
            "remove": ["space"],
            "parserRule": {
                "kind": "parser",
                "message": "space is not required for initializing an spl account",
                "sourceMethod": "build",
            },
        })),
    ))
}
fn initializes_spl_account_with_anchor_space(constraint: &ConstraintEvidence<'_>) -> bool {
    constraint.has_key("token::mint")
        || constraint.has_key("token::authority")
        || constraint.has_key("associated_token::authority")
        || constraint.has_key("mint::authority")
}
fn token_account_init_diagnostics(
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    if !constraint.has_init_like_constraint() || !field.has_generic_type("TokenAccount") {
        return Vec::new();
    }
    if constraint.has_key("associated_token::mint")
        || constraint.has_key("associated_token::authority")
        || constraint.has_key("associated_token::token_program")
    {
        return Vec::new();
    }
    let init_key = constraint.init_constraint_key().unwrap_or("init");

    let mut diagnostics = Vec::new();
    for missing in ["token::mint", "token::authority"] {
        if !constraint.has_key(missing) {
            diagnostics.push(diagnostic_from_range(
                constraint.range(),
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`{}` initializes a token account but is missing `{missing} = ...`.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "constraint": init_key,
                    "missing": missing,
                })),
            ));
        }
    }
    diagnostics
}
fn token_interface_mint_reference_diagnostic(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if field.type_name() != Some("InterfaceAccount") || !field.has_generic_type("TokenAccount") {
        return None;
    }

    let reference = constraint
        .account_references()
        .into_iter()
        .find(|reference| matches!(reference.key, "token::mint" | "associated_token::mint"))?;
    let mint = accounts
        .fields()
        .iter()
        .find(|candidate| candidate.field.name == reference.name)?;
    if mint.type_name() == Some("InterfaceAccount") && mint.has_generic_type("Mint") {
        return None;
    }
    if !token_data_uses_extension_program(accounts, constraint) {
        return None;
    }
    if !mint.has_generic_type("Mint") {
        return None;
    }

    Some(diagnostic_from_range(
        mint.field.type_range.unwrap_or(mint.field.selection_range),
        AnchorDiagnosticKind::AnchorConstraintShape,
        format!(
            "`{}` uses `{}` with InterfaceAccount token data, but `{}` is not `InterfaceAccount<'info, Mint>`.",
            field.field.name, reference.key, mint.field.name
        ),
        Some(serde_json::json!({
            "account": mint.field.name,
            "constraint": reference.key,
            "requiredBy": field.field.name,
            "quickfix": "replace-account-type",
            "expected": "InterfaceAccount<'info, Mint>",
        })),
    ))
}
fn token_data_uses_extension_program(
    accounts: &AccountSetEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> bool {
    token_program_for_token_data(accounts, constraint).is_some_and(|program| {
        program.has_generic_type("Token2022") || program.has_generic_type("TokenInterface")
    })
}
fn token_program_for_token_data<'a>(
    accounts: &'a AccountSetEvidence<'a>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<&'a FieldEvidence<'a>> {
    let program_name = constraint
        .account_references()
        .into_iter()
        .find(|reference| {
            matches!(
                reference.key,
                "token::token_program" | "associated_token::token_program"
            )
        })
        .map(|reference| reference.name)
        .unwrap_or("token_program");

    accounts
        .fields()
        .iter()
        .find(|field| field.field.name == program_name)
}
fn token_program_init_diagnostics(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    if !constraint.has_init_like_constraint() {
        return Vec::new();
    }

    let mut diagnostics = Vec::new();
    let initializes_token_account = field.has_generic_type("TokenAccount");
    let initializes_mint = field.has_generic_type("Mint");
    let uses_associated_token = constraint.has_key("associated_token::mint")
        || constraint.has_key("associated_token::authority")
        || constraint.has_key("associated_token::token_program");

    if initializes_token_account || initializes_mint {
        let token_program_name = constraint
            .account_references()
            .into_iter()
            .find(|reference| {
                matches!(
                    reference.key,
                    "token::token_program"
                        | "associated_token::token_program"
                        | "mint::token_program"
                )
            })
            .map(|reference| reference.name)
            .unwrap_or("token_program");

        diagnostics.extend(required_token_program_diagnostic(
            accounts,
            field,
            constraint,
            token_program_name,
        ));
    }

    if uses_associated_token {
        diagnostics.extend(required_associated_token_program_diagnostic(
            accounts, field, constraint,
        ));
    }

    diagnostics
}
fn required_token_program_diagnostic(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
    program_name: &str,
) -> Option<Diagnostic> {
    match accounts.fields().iter().find(|field| field.field.name == program_name) {
        Some(program)
            if is_token_program_field(program, field)
                && (field.is_optional() || !program.is_optional()) =>
        {
            None
        }
        Some(program) if is_token_program_field(program, field) => Some(diagnostic_from_range(
            program.field.selection_range,
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{program_name}` is optional but `{}` is required; Anchor requires token program accounts for required init constraints to be required.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "field": program_name,
                "accountsStruct": accounts.accounts.name,
                "constraint": "init",
            })),
        )),
        Some(program) => Some(diagnostic_from_range(
            program
                .field
                .type_range
                .unwrap_or(program.field.selection_range),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{program_name}` must be an Anchor token program account because `{}` initializes a token or mint account.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "field": program_name,
                "accountsStruct": accounts.accounts.name,
                "constraint": "init",
                "quickfix": "program-field-type",
                "expected": token_program_type_for_initialized_field(field),
                "quickfixType": token_program_quickfix_type_for_initialized_field(field),
            })),
        )),
        None => Some(diagnostic_from_range(
            constraint.range(),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{}` initializes a token or mint account but `{}` has no `{program_name}` account.",
                field.field.name, accounts.accounts.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "accountsStruct": accounts.accounts.name,
                "constraint": "init",
                "missing": program_name,
                "expected": token_program_type_for_initialized_field(field),
                "quickfixType": token_program_quickfix_type_for_initialized_field(field),
            })),
        )),
    }
}
fn required_associated_token_program_diagnostic(
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    match accounts
        .fields()
        .iter()
        .find(|field| field.field.name == "associated_token_program")
    {
        Some(program)
            if program.type_name() == Some("Program")
                && program.has_generic_type("AssociatedToken")
                && (field.is_optional() || !program.is_optional()) =>
        {
            None
        }
        Some(program)
            if program.type_name() == Some("Program")
                && program.has_generic_type("AssociatedToken") =>
        {
            Some(diagnostic_from_range(
                program.field.selection_range,
                AnchorDiagnosticKind::AnchorConstraintShape,
                format!(
                    "`associated_token_program` is optional but `{}` is required; Anchor requires associated token program accounts for required init constraints to be required.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "field": "associated_token_program",
                    "accountsStruct": accounts.accounts.name,
                    "constraint": "associated_token",
                })),
            ))
        }
        Some(program) => Some(diagnostic_from_range(
            program
                .field
                .type_range
                .unwrap_or(program.field.selection_range),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`associated_token_program` must be typed `Program<'info, AssociatedToken>` because `{}` initializes an associated token account.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "field": "associated_token_program",
                "accountsStruct": accounts.accounts.name,
                "constraint": "associated_token",
                "quickfix": "program-field-type",
                "expected": "Program<'info, AssociatedToken>",
            })),
        )),
        None => Some(diagnostic_from_range(
            constraint.range(),
            AnchorDiagnosticKind::AnchorConstraintShape,
            format!(
                "`{}` initializes an associated token account but `{}` has no `associated_token_program` account.",
                field.field.name, accounts.accounts.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "accountsStruct": accounts.accounts.name,
                "constraint": "associated_token",
                "missing": "associated_token_program",
                "expected": "Program<'info, AssociatedToken>",
            })),
        )),
    }
}
fn is_token_program_field(program: &FieldEvidence<'_>, _initialized: &FieldEvidence<'_>) -> bool {
    matches!(program.type_name(), Some("Program") | Some("Interface"))
        && (program.has_generic_type("Token")
            || program.has_generic_type("Token2022")
            || program.has_generic_type("TokenInterface"))
}
fn token_program_type_for_initialized_field(_field: &FieldEvidence<'_>) -> &'static str {
    TOKEN_PROGRAM_EXPECTED_TYPE
}
fn token_program_quickfix_type_for_initialized_field(field: &FieldEvidence<'_>) -> &'static str {
    if field.type_name() == Some("InterfaceAccount") {
        INTERFACE_TOKEN_PROGRAM_QUICKFIX_TYPE
    } else {
        DEFAULT_TOKEN_PROGRAM_QUICKFIX_TYPE
    }
}
