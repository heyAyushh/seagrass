use super::*;

#[test]
fn reachable_cpi_program_usages_include_called_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/call_external.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn cpi(ctx: Context<Cpi>) -> Result<()> {
        instructions::call_external(ctx)
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let usages = index
        .reachable_cpi_program_usage_names_for_context("Cpi")
        .expect("reachable CPI usages");

    assert!(usages.contains("external_program"));
}

#[test]
fn reachable_cpi_program_usages_ignore_uncalled_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/call_external.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<Cpi>) -> Result<()> {
        let _ = ctx.accounts.external_program.key();
        Ok(())
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
#[derive(Accounts)]
pub struct Cpi<'info> {
    pub external_program: AccountInfo<'info>,
}

pub fn call_external(ctx: Context<Cpi>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.external_program.to_account_info(), ());
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let usages = index
        .reachable_cpi_program_usage_names_for_context("Cpi")
        .expect("reachable CPI usages");

    assert!(!usages.contains("external_program"));
}

#[test]
fn reachable_signer_usages_include_called_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/log_message.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        instructions::log_message(ctx)
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
    let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let usages = index
        .reachable_signer_usage_names_for_context("LogMessage")
        .expect("reachable signer usages");

    assert!(usages.contains("authority"));
}

#[test]
fn reachable_signer_checks_ignore_uncalled_split_helpers() {
    let lib_uri = Url::parse("file:///tmp/lib.rs").unwrap();
    let helper_uri = Url::parse("file:///tmp/instructions/log_message.rs").unwrap();
    let index = WorkspaceIndex::build(
        &[],
        [
            (
                lib_uri,
                r#"
#[program]
pub mod demo {
    pub fn read(ctx: Context<LogMessage>) -> Result<()> {
        let _ = ctx.accounts.authority.key();
        Ok(())
    }
}
"#
                .to_string(),
            ),
            (
                helper_uri,
                r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn check_authority(ctx: Context<LogMessage>) -> Result<()> {
    require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
    Ok(())
}
"#
                .to_string(),
            ),
        ],
    );

    let checks = index
        .reachable_signer_check_names_for_context("LogMessage")
        .expect("reachable signer checks");

    assert!(!checks.contains("authority"));
}
