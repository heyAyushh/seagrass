use {
    crate::{
        anchor::extractor,
        constraint_catalog::{self, ConstraintFamily},
        diagnostics::{diagnostic_from_range, registry::AnchorDiagnosticKind},
        document::ParsedDocument,
        evidence::{AccountSetEvidence, ConstraintEvidence, EvidenceGraph, FieldEvidence},
        semantic::{AccountType, SemanticModel},
    },
    tower_lsp::lsp_types::{Diagnostic, Range},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenProgramKind {
    Token,
    Token2022,
    Interface,
}

const TOKEN_PROGRAM_EXPECTED_TYPE: &str = "Program<'info, Token>, Program<'info, Token2022>, Program<'info, TokenInterface>, or Interface<'info, TokenInterface>";

pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    let semantic_model = extractor::extract(document);
    let semantic_model = &semantic_model;
    EvidenceGraph::from_document(document)
        .account_sets()
        .iter()
        .flat_map(|accounts| {
            accounts.fields().iter().flat_map(move |field| {
                field.constraints().iter().flat_map(move |constraint| {
                    constraint_diagnostics(document, accounts, field, constraint, semantic_model)
                })
            })
        })
        .collect()
}

fn constraint_diagnostics(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
    semantic_model: &SemanticModel,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    diagnostics.extend(mint_reference_diagnostics(
        document, accounts, field, constraint,
    ));
    diagnostics.extend(target_container_diagnostic(document, field, constraint));
    diagnostics.extend(token_program_override_diagnostics(
        document,
        accounts,
        field,
        constraint,
        semantic_model,
    ));
    diagnostics.extend(token2022_extension_diagnostics(
        document, accounts, field, constraint,
    ));
    diagnostics.extend(mint_decimals_argument_diagnostic(
        document, accounts, constraint,
    ));
    diagnostics.extend(transfer_hook_program_diagnostic(
        document, accounts, constraint,
    ));
    diagnostics
}

fn mint_reference_diagnostics(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    ["token::mint", "associated_token::mint"]
        .into_iter()
        .filter_map(|key| {
            let name = constraint.simple_identifier_value(key)?;
            let mint_field = account_field(accounts, name)?;
            if is_mint_data_field(document, mint_field) {
                return None;
            }
            Some(diagnostic_from_range(
                value_range(document, constraint, key, name),
                AnchorDiagnosticKind::AnchorSplTokenInterface,
                format!(
                    "`{}` references `{name}` via `{key}`, but `{name}` is not typed as a Mint account.",
                    field.field.name
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "reference": name,
                    "constraint": key,
                    "expected": "Account<'info, Mint> or InterfaceAccount<'info, Mint>",
                    "reason": "mint-reference-type",
                })),
            ))
        })
        .collect()
}

fn target_container_diagnostic(
    document: &ParsedDocument,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    if uses_token_constraints(constraint) && !is_token_account_data_field(document, field) {
        return Some(type_diagnostic(
            field,
            "token account constraints",
            "Account<'info, TokenAccount> or InterfaceAccount<'info, TokenAccount>",
            "token-target-type",
        ));
    }
    if uses_mint_constraints(constraint) && !is_mint_data_field(document, field) {
        return Some(type_diagnostic(
            field,
            "mint constraints",
            "Account<'info, Mint> or InterfaceAccount<'info, Mint>",
            "mint-target-type",
        ));
    }
    None
}

fn token_program_override_diagnostics(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
    semantic_model: &SemanticModel,
) -> Vec<Diagnostic> {
    ["token::token_program", "associated_token::token_program", "mint::token_program"]
        .into_iter()
        .filter_map(|key| {
            let name = constraint.simple_identifier_value(key)?;
            let program = account_field(accounts, name)?;
            let program_kind = token_program_kind(program);
            if program_kind.is_some()
                || semantic_token_program_candidate(semantic_model, &accounts.accounts.name, name)
            {
                return None;
            }
            Some(diagnostic_from_range(
                value_range(document, constraint, key, name),
                AnchorDiagnosticKind::AnchorSplTokenInterface,
                format!(
                    "`{key}` points at `{name}`, but `{name}` is not a compatible SPL token program account."
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "reference": name,
                    "constraint": key,
                    "expected": TOKEN_PROGRAM_EXPECTED_TYPE,
                    "reason": "token-program-type",
                })),
            ))
        })
        .collect()
}

fn semantic_token_program_candidate(
    semantic_model: &SemanticModel,
    accounts_struct_name: &str,
    name: &str,
) -> bool {
    semantic_model
        .all_fields_for_struct(accounts_struct_name)
        .into_iter()
        .any(|field| {
            field.name == name
                && field.token_interface_candidate
                && matches!(
                    field.account_type,
                    AccountType::Interface | AccountType::Program
                )
        })
}

fn token2022_extension_diagnostics(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    field: &FieldEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Vec<Diagnostic> {
    if !uses_token2022_extension_constraint(constraint) {
        return Vec::new();
    }

    let Some((program_name, program)) = effective_token_program(accounts, constraint) else {
        return Vec::new();
    };
    let Some(program_kind) = token_program_kind(program) else {
        return Vec::new();
    };

    let mut diagnostics = Vec::new();
    if program_kind == TokenProgramKind::Token {
        diagnostics.push(diagnostic_from_range(
            field.field.selection_range,
            AnchorDiagnosticKind::AnchorSplTokenInterface,
            format!(
                "`{}` uses Token-2022 extension constraints but `{program_name}` is `Program<'info, Token>`.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "tokenProgram": program_name,
                "constraint": "extensions",
                "expected": "Program<'info, Token2022> or Interface<'info, TokenInterface>",
                "reason": "token2022-extension-program",
            })),
        ));
        return diagnostics;
    }

    if field.type_name() == Some("Account")
        && (is_mint_data_field(document, field) || is_token_account_data_field(document, field))
    {
        diagnostics.push(diagnostic_from_range(
            field
                .field
                .type_range
                .unwrap_or(field.field.selection_range),
            AnchorDiagnosticKind::AnchorSplTokenInterface,
            format!(
                "`{}` uses Token-2022-capable program `{program_name}` but is wrapped in `Account`; use `InterfaceAccount` for Token-2022 data.",
                field.field.name
            ),
            Some(serde_json::json!({
                "account": field.field.name,
                "tokenProgram": program_name,
                "constraint": "extensions",
                "expected": "InterfaceAccount<'info, Mint> or InterfaceAccount<'info, TokenAccount>",
                "reason": "token2022-wrapper",
            })),
        ));
    }

    diagnostics
}

fn mint_decimals_argument_diagnostic(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    let name = constraint.simple_identifier_value("mint::decimals")?;
    let type_name = accounts.instruction_argument_type(name)?;
    if type_name == "u8" {
        return None;
    }
    Some(diagnostic_from_range(
        value_range(document, constraint, "mint::decimals", name),
        AnchorDiagnosticKind::AnchorSplTokenInterface,
        format!("`mint::decimals = {name}` resolves to `{type_name}`, but Anchor requires `u8`."),
        Some(serde_json::json!({
            "argument": name,
            "argumentType": type_name,
            "constraint": "mint::decimals",
            "expected": "u8",
            "reason": "mint-decimals-type",
        })),
    ))
}

fn transfer_hook_program_diagnostic(
    document: &ParsedDocument,
    accounts: &AccountSetEvidence<'_>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<Diagnostic> {
    let key = "extensions::transfer_hook::program_id";
    let name = constraint.simple_identifier_value(key)?;
    let field = account_field(accounts, name)?;
    if token_program_kind(field).is_some()
        || matches!(field.type_name(), Some("ProgramData"))
        || field.is_unchecked_account()
            && field.has_any_constraint(&["executable", "address", "constraint"])
    {
        return None;
    }
    Some(diagnostic_from_range(
        value_range(document, constraint, key, name),
        AnchorDiagnosticKind::AnchorSplTokenInterface,
        format!(
            "`{key}` points at `{name}`, but transfer hooks should reference a program-like account."
        ),
        Some(serde_json::json!({
            "account": name,
            "constraint": key,
            "expected": "Program, Interface, or executable checked account",
            "reason": "transfer-hook-program",
        })),
    ))
}

fn type_diagnostic(
    field: &FieldEvidence<'_>,
    constraint_label: &str,
    expected: &str,
    reason: &str,
) -> Diagnostic {
    diagnostic_from_range(
        field
            .field
            .type_range
            .unwrap_or(field.field.selection_range),
        AnchorDiagnosticKind::AnchorSplTokenInterface,
        format!(
            "`{}` uses {constraint_label} but is not typed `{expected}`.",
            field.field.name
        ),
        Some(serde_json::json!({
            "account": field.field.name,
            "expected": expected,
            "reason": reason,
        })),
    )
}

fn uses_token_constraints(constraint: &ConstraintEvidence<'_>) -> bool {
    ["token::mint", "token::authority", "token::token_program"]
        .into_iter()
        .any(|key| constraint.has_key(key))
        || [
            "associated_token::mint",
            "associated_token::authority",
            "associated_token::token_program",
        ]
        .into_iter()
        .any(|key| constraint.has_key(key))
}

fn uses_mint_constraints(constraint: &ConstraintEvidence<'_>) -> bool {
    [
        "mint::authority",
        "mint::freeze_authority",
        "mint::decimals",
        "mint::token_program",
    ]
    .into_iter()
    .any(|key| constraint.has_key(key))
        || uses_token2022_extension_constraint(constraint)
}

fn uses_token2022_extension_constraint(constraint: &ConstraintEvidence<'_>) -> bool {
    constraint_catalog::family_keys(ConstraintFamily::MintExtension)
        .any(|key| constraint.has_key(key))
}

fn effective_token_program<'a>(
    accounts: &'a AccountSetEvidence<'a>,
    constraint: &ConstraintEvidence<'_>,
) -> Option<(&'a str, &'a FieldEvidence<'a>)> {
    for key in [
        "mint::token_program",
        "token::token_program",
        "associated_token::token_program",
    ] {
        if let Some(name) = constraint.simple_identifier_value(key) {
            return account_field(accounts, name).map(|field| (field.field.name.as_str(), field));
        }
    }
    account_field(accounts, "token_program").map(|field| (field.field.name.as_str(), field))
}

fn account_field<'a>(
    accounts: &'a AccountSetEvidence<'a>,
    name: &str,
) -> Option<&'a FieldEvidence<'a>> {
    accounts
        .fields()
        .iter()
        .find(|field| field.field.name == name)
}

fn is_token_account_data_field(document: &ParsedDocument, field: &FieldEvidence<'_>) -> bool {
    is_data_field(document, field, "TokenAccount")
}

fn is_mint_data_field(document: &ParsedDocument, field: &FieldEvidence<'_>) -> bool {
    is_data_field(document, field, "Mint")
}

fn is_data_field(document: &ParsedDocument, field: &FieldEvidence<'_>, generic: &str) -> bool {
    matches!(field.type_name(), Some("Account" | "InterfaceAccount"))
        && field.field.generic_type_names.iter().any(|name| {
            // Triage 2026-06-10: program-examples transfer-hook aliases
            // `anchor_spl::token_interface::Mint` as `MintAccount`. Import
            // aliases are single-file syntax evidence, so resolve them
            // before deciding whether InterfaceAccount<MintAccount> is a mint.
            document.symbols().resolve_type_alias(name) == generic
        })
}

fn token_program_kind(field: &FieldEvidence<'_>) -> Option<TokenProgramKind> {
    match field.type_name()? {
        "Program" if field.has_generic_type("Token") => Some(TokenProgramKind::Token),
        "Program" if field.has_generic_type("Token2022") => Some(TokenProgramKind::Token2022),
        "Program" if field.has_generic_type("TokenInterface") => Some(TokenProgramKind::Interface),
        "Interface" if field.has_generic_type("TokenInterface") => {
            Some(TokenProgramKind::Interface)
        }
        _ => None,
    }
}

fn value_range(
    document: &ParsedDocument,
    constraint: &ConstraintEvidence<'_>,
    key: &str,
    value: &str,
) -> Range {
    constraint
        .value_range(document.source(), key, value)
        .unwrap_or(constraint.range())
}

#[cfg(test)]
mod tests {
    use {super::*, tower_lsp::lsp_types::NumberOrString};

    #[test]
    fn warns_when_token_mint_references_token_account() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::mint = token, token::authority = authority)]
    pub vault: Account<'info, TokenAccount>,
    pub token: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(has_reason(&diagnostics, "mint-reference-type"));
    }

    #[test]
    fn warns_when_program_token_account_uses_token_constraints() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::authority = authority)]
    pub wrong: Program<'info, TokenAccount>,
    pub authority: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(has_reason(&diagnostics, "token-target-type"));
    }

    #[test]
    fn warns_for_token2022_extension_with_token_program() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::token_program = token_program,
        extensions::metadata_pointer::authority = payer,
        extensions::metadata_pointer::metadata_address = mint
    )]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create(ctx: Context<Create>, decimals: u16) -> Result<()> {
    Ok(())
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(has_reason(&diagnostics, "token2022-extension-program"));
        assert!(has_reason(&diagnostics, "mint-decimals-type"));
    }

    #[test]
    fn ignores_extension_key_text_inside_unrelated_constraint_expression() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        mint::authority = payer,
        mint::token_program = token_program,
        constraint = memo == "extensions::metadata_pointer::authority"
    )]
    pub mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub payer: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_reason(&diagnostics, "token2022-extension-program"));
        assert!(!has_reason(&diagnostics, "token2022-wrapper"));
    }

    #[test]
    fn accepts_token2022_interface_account_pair() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Mint, TokenInterface};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::token_program = token_program,
        extensions::metadata_pointer::authority = payer,
        extensions::metadata_pointer::metadata_address = mint
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Interface<'info, TokenInterface>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create(ctx: Context<Create>, decimals: u8) -> Result<()> {
    Ok(())
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_code(&diagnostics, "anchor-spl-token-interface"));
    }

    #[test]
    fn accepts_token2022_program_for_interface_account_pair() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::{token_2022::Token2022, token_interface::Mint};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = payer,
        mint::decimals = decimals,
        mint::authority = payer,
        mint::token_program = token_program
    )]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Program<'info, Token2022>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

pub fn create(ctx: Context<Create>, decimals: u8) -> Result<()> {
    Ok(())
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_code(&diagnostics, "anchor-spl-token-interface"));
    }

    #[test]
    fn no_false_positive_for_program_token2022_on_mint_constraint() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::{token_2022::Token2022, token_interface::Mint};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::authority = payer, mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, Mint>,
    pub token_program: Program<'info, Token2022>,
    pub payer: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_reason(&diagnostics, "token-program-type"));
    }

    #[test]
    fn no_false_positive_for_program_token_on_token_account_constraint() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_interface::TokenAccount};

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub payer: Signer<'info>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_reason(&diagnostics, "token-program-type"));
    }

    #[test]
    fn accepts_import_alias_for_interface_account_mint() {
        let document = ParsedDocument::parse(
            r#"
use anchor_lang::prelude::*;
use anchor_spl::{
    token_2022::Token2022,
    token_interface::Mint as MintAccount,
};

#[derive(Accounts)]
pub struct ChangeMode<'info> {
    #[account(mut, mint::token_program = token_program)]
    pub mint: InterfaceAccount<'info, MintAccount>,
    pub token_program: Program<'info, Token2022>,
}
"#,
        )
        .unwrap();

        let diagnostics = collect(&document);

        assert!(!has_code(&diagnostics, "anchor-spl-token-interface"));
    }

    fn has_reason(diagnostics: &[Diagnostic], reason: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .is_some_and(|data| data["reason"] == reason)
        })
    }

    fn has_code(diagnostics: &[Diagnostic], expected: &str) -> bool {
        diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code.as_ref(),
                Some(NumberOrString::String(code)) if code == expected
            )
        })
    }
}
