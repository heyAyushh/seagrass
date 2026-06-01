use super::*;

#[test]
fn flags_raw_account_data_without_owner_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let data = ctx.accounts.user.data.borrow();
        Ok(())
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("attack"))
            .and_then(|value| value.as_str()),
        Some("owner-checks")
    );
}

#[test]
fn flags_alias_raw_account_data_without_owner_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let user = &ctx.accounts.user;
        let data = user.data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn accepts_raw_account_data_with_owner_constraint() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    #[account(owner = crate::ID)]
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let data = ctx.accounts.user.data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn accepts_raw_account_data_with_manual_owner_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        require!(ctx.accounts.user.owner == &crate::ID, ErrorCode::InvalidOwner);
        let data = ctx.accounts.user.data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn flags_raw_account_data_when_owner_check_only_in_debug_assert() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        debug_assert!(ctx.accounts.user.owner == &crate::ID);
        let data = ctx.accounts.user.data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn flags_raw_account_data_when_owner_check_is_unrelated() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn helper(ctx: Context<ReadRaw>) -> ProgramResult {
        assert_owner(&ctx.accounts.user, &crate::ID)?;
        Ok(())
    }

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let data = ctx.accounts.user.data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn ignores_raw_account_text_in_comments_strings_and_attributes() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    #[doc = "ctx.accounts.user.data.borrow(); UserState::try_from_slice(&data)"]
    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        // ctx.accounts.user.data.borrow()
        let _template = "UserState::try_from_slice(&ctx.accounts.user.data.borrow())";
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
    assert_no_code(&diagnostics, ANCHOR_SECURITY_TYPE_COSPLAY_CODE);
}

#[test]
fn ignores_security_evidence_inside_dead_macro_body() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct Guard<'info> {
    authority: AccountInfo<'info>,
    token: AccountInfo<'info>,
    external_program: AccountInfo<'info>,
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<Guard>) -> ProgramResult {
        dead_code!(
            AccountMeta::new(ctx.accounts.authority.key(), true),
            SplTokenAccount::unpack(&ctx.accounts.token.data.borrow()),
            CpiContext::new(ctx.accounts.external_program.to_account_info(), ()),
            UserState::try_from_slice(&ctx.accounts.user.data.borrow())
        );
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_SIGNER_CODE);
    assert_no_code(&diagnostics, ANCHOR_SECURITY_TOKEN_ACCOUNT_CODE);
    assert_no_code(&diagnostics, ANCHOR_SECURITY_CPI_PROGRAM_CODE);
    assert_no_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
    assert_no_code(&diagnostics, ANCHOR_SECURITY_TYPE_COSPLAY_CODE);
}

#[test]
fn accepts_raw_account_data_with_deref_owner_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let user = &ctx.accounts.user;
        require!((*user).owner == &crate::ID, ErrorCode::InvalidOwner);
        let data = (*user).data.borrow();
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_OWNER_CHECK_CODE);
}

#[test]
fn flags_raw_deserialization_without_discriminator_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    #[account(owner = crate::ID)]
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let user = UserState::try_from_slice(&ctx.accounts.user.data.borrow())?;
        Ok(())
    }
}
"#,
    );

    let diagnostic = assert_has_code(&diagnostics, ANCHOR_SECURITY_TYPE_COSPLAY_CODE);
    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("attack"))
            .and_then(|value| value.as_str()),
        Some("type-cosplay")
    );
}

#[test]
fn accepts_raw_deserialization_with_visible_discriminator_check() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    #[account(owner = crate::ID)]
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let data = ctx.accounts.user.data.borrow();
        require!(&data[..8] == UserState::DISCRIMINATOR, ErrorCode::InvalidDiscriminator);
        let user = UserState::try_from_slice(&data[8..])?;
        Ok(())
    }
}
"#,
    );

    assert_no_code(&diagnostics, ANCHOR_SECURITY_TYPE_COSPLAY_CODE);
}

#[test]
fn flags_raw_deserialization_when_safe_deserialize_is_unrelated() {
    let diagnostics = security_diagnostics(
        r#"
#[derive(Accounts)]
pub struct ReadRaw<'info> {
    #[account(owner = crate::ID)]
    user: AccountInfo<'info>,
}

#[program]
pub mod demo {
    use super::*;

    pub fn helper(ctx: Context<ReadRaw>) -> ProgramResult {
        let user = UserState::try_deserialize(&mut &ctx.accounts.user.data.borrow()[..])?;
        Ok(())
    }

    pub fn handler(ctx: Context<ReadRaw>) -> ProgramResult {
        let user = UserState::try_from_slice(&ctx.accounts.user.data.borrow())?;
        Ok(())
    }
}
"#,
    );

    assert_has_code(&diagnostics, ANCHOR_SECURITY_TYPE_COSPLAY_CODE);
}
