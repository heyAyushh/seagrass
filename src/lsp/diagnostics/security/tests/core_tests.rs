use super::*;

#[test]
fn flags_unchecked_account_used_as_instruction_signer() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(diagnostic.range.start.line, 5);
    assert_eq!(
        diagnostic.range.end.character - diagnostic.range.start.character,
        "AccountInfo".len() as u32
    );
    assert!(diagnostic.code_description.is_some());
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("rule"))
            .and_then(|rule| rule.as_str()),
        Some("sealevel-attacks/signer-authorization")
    );
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("confidence"))
            .and_then(|confidence| confidence.as_str()),
        Some("authoritative")
    );
}

#[test]
fn stays_quiet_for_unchecked_authority_name_without_signer_evidence() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn accepts_manual_signer_check() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
        let metas = vec![AccountMeta::new_readonly(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn accepts_manual_signer_check_through_local_alias() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let authority = &ctx.accounts.authority;
        require!(authority.is_signer, ErrorCode::MissingSigner);
        let metas = vec![AccountMeta::new(authority.key(), true)];
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn flags_signer_usage_through_local_alias() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let authority = &ctx.accounts.authority;
        let metas = vec![AccountMeta::new(authority.key(), true)];
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn flags_deref_signer_usage_without_manual_validation() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let metas = vec![AccountMeta::new((*ctx.accounts.authority).key(), true)];
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn signer_constraint_detection_does_not_match_expression_substrings() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    #[account(constraint = cosigner.key() != authority.key())]
    authority: AccountInfo<'info>,
    cosigner: Signer<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn accepts_typed_signer_authority() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    authority: Signer<'info>,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn flags_manual_token_unpacking_into_account_info() {
    let diagnostics = security_diagnostics(
        r#"
use spl_token::state::Account as SplTokenAccount;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    token: AccountInfo<'info>,
    authority: Signer<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<LogMessage>) -> ProgramResult {
        let token = SplTokenAccount::unpack(&ctx.accounts.token.data.borrow())?;
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE);
}

#[test]
fn flags_manual_token_unpacking_by_usage_not_field_name() {
    let diagnostics = security_diagnostics(
        r#"
use spl_token::state::Account as SplTokenAccount;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    vaultish: AccountInfo<'info>,
    unused: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<LogMessage>) -> ProgramResult {
        let parsed = SplTokenAccount::unpack(&ctx.accounts.vaultish.data.borrow())?;
        Ok(())
    }
}
"#,
    );

    let token_diagnostics = diagnostics_with_code(&diagnostics, ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE);
    assert_eq!(token_diagnostics.len(), 1);
    assert_eq!(
        token_diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|value| value.as_str()),
        Some("vaultish")
    );
}

#[test]
fn accepts_manual_token_unpacking_with_spaced_owner_constraint() {
    let diagnostics = security_diagnostics(
        r#"
use spl_token::state::Account as SplTokenAccount;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    #[account(owner = spl_token::ID)]
    token: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<LogMessage>) -> ProgramResult {
        let token = SplTokenAccount::unpack(&ctx.accounts.token.data.borrow())?;
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE);
}

#[test]
fn accepts_typed_token_account() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_spl::token::TokenAccount;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    token: Account<'info, TokenAccount>,
    authority: Signer<'info>,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE);
}

#[test]
fn flags_unchecked_cpi_program_account() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::{instruction::Instruction, program::invoke};

#[derive(Accounts)]
pub struct Cpi<'info> {
    source: AccountInfo<'info>,
    destination: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    metadata_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        invoke(&ix, &[ctx.accounts.source.clone(), ctx.accounts.metadata_program.clone()])
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|value| value.as_str()),
        Some("metadata_program")
    );
}

#[test]
fn flags_unchecked_cpi_context_program_account() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    external_program: AccountInfo<'info>,
    source: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let cpi_accounts = ();
        let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), cpi_accounts);
        Ok(())
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
    assert!(diagnostic
        .message
        .contains("`external_program` is used as a CPI program account"));
}

#[test]
fn flags_unchecked_cpi_context_program_account_through_local_alias() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    external_program: AccountInfo<'info>,
    source: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let cpi_accounts = ();
        let external_program = ctx.accounts.external_program.to_account_info();
        let cpi_ctx = CpiContext::new(external_program, cpi_accounts);
        Ok(())
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
    assert!(diagnostic
        .message
        .contains("`external_program` is used as a CPI program account"));
}

#[test]
fn flags_unchecked_cpi_program_used_in_reachable_split_helper() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#;
    let program_source = r#"
#[program]
pub mod demo {
    pub fn cpi(ctx: Context<Cpi>) -> Result<()> {
        instructions::call_external(ctx)
    }
}
"#;
    let accounts_document = ParsedDocument::parse(accounts_source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/accounts.rs").unwrap(),
                accounts_source.to_string(),
            ),
            (
                Url::parse("file:///tmp/instructions.rs").unwrap(),
                helper_source.to_string(),
            ),
            (
                Url::parse("file:///tmp/lib.rs").unwrap(),
                program_source.to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&accounts_document, Some(&workspace_index));

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|value| value.as_str()),
        Some("external_program")
    );
}

#[test]
fn ignores_unchecked_cpi_program_in_unreachable_split_helper() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#;
    let program_source = r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Cpi>) -> Result<()> {
        let _ = ctx.accounts.external_program.key();
        Ok(())
    }
}
"#;
    let accounts_document = ParsedDocument::parse(accounts_source).unwrap();
    let workspace_index = WorkspaceIndex::build(
        &[],
        [
            (
                Url::parse("file:///tmp/accounts.rs").unwrap(),
                accounts_source.to_string(),
            ),
            (
                Url::parse("file:///tmp/instructions.rs").unwrap(),
                helper_source.to_string(),
            ),
            (
                Url::parse("file:///tmp/lib.rs").unwrap(),
                program_source.to_string(),
            ),
        ],
    );

    let diagnostics = collect_with_workspace(&accounts_document, Some(&workspace_index));

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
}

#[test]
fn accepts_cpi_program_account_with_spaced_address_constraint() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::{instruction::Instruction, program::invoke};

#[derive(Accounts)]
pub struct Cpi<'info> {
    source: AccountInfo<'info>,
    #[account(address = mpl_token_metadata::ID)]
    metadata_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        invoke(&ix, &[ctx.accounts.source.clone(), ctx.accounts.metadata_program.clone()])
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
}

#[test]
fn does_not_flag_plain_cpi_account_infos_as_program_accounts() {
    let diagnostics = security_diagnostics(
        r#"
use anchor_lang::solana_program::program::invoke;

#[derive(Accounts)]
pub struct Cpi<'info> {
    source: AccountInfo<'info>,
    destination: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        invoke(ix, &[ctx.accounts.source.clone(), ctx.accounts.destination.clone()])
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
}

#[test]
fn flags_unchecked_rent_sysvar() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {
    rent: AccountInfo<'info>,
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_SYSVAR_CODE);
}

#[test]
fn flags_generated_sysvars_without_handwritten_name_list() {
    let generated_sysvar_fields = anchor_types::field_completions()
        .iter()
        .filter(|completion| completion.kind == AnchorFieldCompletionKind::Sysvar)
        .filter(|completion| !completion.label.ends_with(", T>"))
        .filter_map(|completion| completion.field_label.split_once(':').map(|(name, _)| name))
        .collect::<Vec<_>>();

    assert!(generated_sysvar_fields.len() >= 8);

    for field_name in generated_sysvar_fields {
        let diagnostics = security_diagnostics(&format!(
            r#"
#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {{
    {field_name}: AccountInfo<'info>,
}}
"#
        ));

        assert_has_code(&diagnostics, ANCHOR_SECURITY_SYSVAR_CODE);
    }
}

#[test]
fn accepts_typed_rent_sysvar() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct CheckSysvarAddress<'info> {
    rent: Sysvar<'info, Rent>,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SYSVAR_CODE);
}
