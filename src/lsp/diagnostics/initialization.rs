use {
    crate::{
        diagnostics::{diagnostic_from_range_with_related, registry::AnchorDiagnosticKind},
        document::{InstructionSymbol, ParsedDocument},
        evidence::{AccountSetEvidence, EvidenceGraph},
    },
    tower_lsp::lsp_types::{Diagnostic, DiagnosticRelatedInformation, Location, Url},
};

pub fn collect(document: &ParsedDocument) -> Vec<Diagnostic> {
    EvidenceGraph::from_document(document)
        .account_sets()
        .iter()
        .filter_map(|accounts| {
            accounts
                .init_like_instruction()
                .map(|instruction| (accounts, instruction))
        })
        .filter(|(accounts, _)| accounts.has_payer_candidate() && accounts.has_system_program())
        .flat_map(|(accounts, instruction)| missing_init_constraints(accounts, instruction))
        .collect()
}

fn missing_init_constraints(
    accounts: &AccountSetEvidence<'_>,
    instruction: Option<&InstructionSymbol>,
) -> Vec<Diagnostic> {
    let mutably_used_accounts = instruction
        .map(|instruction| {
            accounts
                .fields()
                .iter()
                .filter(|field| {
                    field
                        .used_by_instructions()
                        .iter()
                        .any(|usage| usage.instruction == instruction.name && usage.mutable)
                })
                .map(|field| field.field.name.as_str())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    accounts
        .fields()
        .iter()
        .filter(|field| field.type_name() == Some("Account"))
        .filter(|field| !field.has_init_constraint())
        .filter(|field| {
            mutably_used_accounts.is_empty()
                || mutably_used_accounts.contains(&field.field.name.as_str())
        })
        .map(|field| {
            let account_type = field
                .field
                .generic_type_names
                .last()
                .map(String::as_str)
                .unwrap_or("AccountType");
            diagnostic_from_range_with_related(
                field.field.selection_range,
                AnchorDiagnosticKind::AnchorMissingInitConstraint,
                format!(
                    "`{}` is in `{}` but is missing `#[account(init, payer = ..., space = 8 + {}::INIT_SPACE)]`.",
                    field.field.name, accounts.accounts.name, account_type
                ),
                Some(serde_json::json!({
                    "account": field.field.name,
                    "accountType": account_type,
                    "accountsStruct": accounts.accounts.name,
                })),
                instruction.map(|instruction| {
                    vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: Url::parse("file:///seagrass/current-document.rs").unwrap(),
                            range: instruction.selection_range,
                        },
                        message: format!(
                            "`{}` uses `Context<{}>`.",
                            instruction.name, accounts.accounts.name
                        ),
                    }]
                }),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::diagnostics::registry::ANCHOR_MISSING_INIT_CONSTRAINT_CODE,
        tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString},
    };

    #[test]
    fn reports_missing_init_from_initialize_instruction_context() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
mod basic_1 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        let diagnostic = missing_init_diagnostic(&diagnostics);
        // AnchorMissingInitConstraint is WholeProgram provability, so it defaults to WARNING.
        assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(diagnostic.range.start.line, 12);
        assert_eq!(
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("account"))
                .and_then(|account| account.as_str()),
            Some("my_account")
        );
    }

    #[test]
    fn does_not_report_when_context_is_used_by_non_init_instruction() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
mod basic_1 {
    use super::*;

    pub fn read(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    #[test]
    fn stays_quiet_without_program_instruction_evidence() {
        let diagnostics = diagnostics_for(
            r#"
#[derive(Accounts)]
pub struct Initialize<'info> {
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    #[test]
    fn accepts_initialized_account() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
mod basic_1 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = user, space = 8 + MyAccount::INIT_SPACE)]
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    #[test]
    fn accepts_init_if_needed_as_initialization_constraint() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
mod basic_1 {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init_if_needed, payer = user, space = 8 + MyAccount::INIT_SPACE)]
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    #[test]
    fn reports_only_mutated_missing_init_field_when_instruction_has_evidence() {
        let diagnostics = diagnostics_for(
            r#"
#[program]
mod demo {
    use super::*;

    pub fn init_my_account(ctx: Context<InitMyAccount>) -> Result<()> {
        ctx.accounts.account.data = 1337;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitMyAccount<'info> {
    pub base: Account<'info, BaseAccount>,
    #[account(init, payer = payer, space = 8 + 8)]
    pub account: Account<'info, MyAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    #[test]
    fn does_not_report_for_update_account() {
        let diagnostics = diagnostics_for(
            r#"
#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut)]
    pub my_account: Account<'info, MyAccount>,
}
"#,
        );

        assert!(diagnostics
            .iter()
            .all(|diagnostic| !has_missing_init_code(diagnostic)));
    }

    fn diagnostics_for(source: &str) -> Vec<Diagnostic> {
        let document = ParsedDocument::parse(source).unwrap();
        collect(&document)
    }

    fn missing_init_diagnostic(diagnostics: &[Diagnostic]) -> &Diagnostic {
        diagnostics
            .iter()
            .find(|diagnostic| has_missing_init_code(diagnostic))
            .unwrap_or_else(|| panic!("expected missing init diagnostic; got {diagnostics:?}"))
    }

    fn has_missing_init_code(diagnostic: &Diagnostic) -> bool {
        matches!(
            diagnostic.code.as_ref(),
            Some(NumberOrString::String(code)) if code == ANCHOR_MISSING_INIT_CONSTRAINT_CODE
        )
    }
}
