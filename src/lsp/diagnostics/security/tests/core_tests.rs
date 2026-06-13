use super::*;
mod cpi_tests;

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
fn flags_unchecked_signer_used_in_reachable_split_helper() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
    let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
    Ok(())
}
"#;
    let program_source = r#"
#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        instructions::log_message(ctx)
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

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("account"))
            .and_then(|value| value.as_str()),
        Some("authority")
    );
}

#[test]
fn accepts_reachable_split_helper_signer_check() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
    require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
    let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
    Ok(())
}
"#;
    let program_source = r#"
#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        instructions::log_message(ctx)
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

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn accepts_reachable_split_helper_signer_check_by_field_name() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct OtherLogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn validate_authority(ctx: Context<OtherLogMessage>) -> Result<()> {
    require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
    Ok(())
}
"#;
    let program_source = r#"
use anchor_lang::solana_program::instruction::AccountMeta;

#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        instructions::validate_authority(ctx)?;
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
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

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
}

#[test]
fn accepts_reachable_method_call_signer_check() {
    let accounts_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}
"#;
    let helper_source = r#"
#[derive(Accounts)]
pub struct LogMessage<'info> {
    pub authority: AccountInfo<'info>,
}

pub fn validate_authority(ctx: Context<LogMessage>) -> Result<()> {
    require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
    Ok(())
}
"#;
    let program_source = r#"
#[program]
pub mod demo {
    pub fn log_message(ctx: Context<LogMessage>) -> Result<()> {
        ctx.accounts.authority.validate_authority();
        let metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
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

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
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
