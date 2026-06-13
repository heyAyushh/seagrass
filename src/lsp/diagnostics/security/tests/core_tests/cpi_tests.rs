use super::*;

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
fn ignores_unchecked_cpi_program_in_ambiguous_reachable_helper() {
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
    let duplicate_helper_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
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
                Url::parse("file:///tmp/other_instructions.rs").unwrap(),
                duplicate_helper_source.to_string(),
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
fn accepts_reachable_cpi_program_with_address_constraint() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    #[account(address = external_program::ID)]
    pub external_program: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    #[account(address = external_program::ID)]
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

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
}

#[test]
fn flags_reachable_cpi_program_checked_against_sibling_account() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
    pub expected_program: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
    pub expected_program: AccountInfo<'info>,
}

pub fn check_program(ctx: Context<Cpi>) -> Result<()> {
    if ctx.accounts.external_program.key() != ctx.accounts.expected_program.key() {
        return err!(ErrorCode::InvalidProgram);
    }
    Ok(())
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
        instructions::check_program(ctx)?;
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
fn accepts_reachable_cpi_program_checked_against_static_program_id() {
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

pub fn check_program(ctx: Context<Cpi>) -> Result<()> {
    if ctx.accounts.external_program.key() != external_program::ID {
        return err!(ErrorCode::InvalidProgram);
    }
    Ok(())
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
        instructions::check_program(ctx)?;
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

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
}

#[test]
fn accepts_same_file_cpi_program_checked_against_static_program_id() {
    let diagnostics = security_diagnostics(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> Result<()> {
        if ctx.accounts.external_program.key() != external_program::ID {
            return err!(ErrorCode::InvalidProgram);
        }
        let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
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
fn ignores_unchecked_cpi_program_in_unreachable_context_reference_helper() {
    let diagnostics = security_diagnostics(
        r#"
#[program]
pub mod demo {
    use super::*;

    pub fn read(ctx: Context<Cpi>) -> Result<()> {
        let _ = ctx.accounts.external_program.key();
        Ok(())
    }
}

pub fn call_external(ctx: &Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}

#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}
"#,
    );

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
