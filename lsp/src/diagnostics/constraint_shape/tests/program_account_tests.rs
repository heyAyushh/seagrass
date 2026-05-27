use super::*;

#[test]
fn reports_unchecked_program_account() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Proxy<'info> {
    pub token_program: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn proxy(ctx: Context<Proxy>) -> Result<()> {
        let _cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
        Ok(())
    }
}
"#,
    );

    assert!(has_code(
        &diagnostics,
        ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE
    ));
}
#[test]
fn accepts_unchecked_program_named_field_without_program_usage() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Proxy<'info> {
    pub token_program: AccountInfo<'info>,
}
"#,
    );

    assert!(!has_code(
        &diagnostics,
        ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE
    ));
}
#[test]
fn reports_unchecked_cpi_program_usage_without_program_field_name() {
    let diagnostics = diagnostics_for(
        r#"
#[derive(Accounts)]
pub struct Proxy<'info> {
    pub external: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn proxy(ctx: Context<Proxy>) -> Result<()> {
        let _cpi_ctx = CpiContext::new(ctx.accounts.external.to_account_info(), ());
        Ok(())
    }
}
"#,
    );

    assert!(has_code(
        &diagnostics,
        ANCHOR_SECURITY_UNCHECKED_ACCOUNT_CODE
    ));
}
