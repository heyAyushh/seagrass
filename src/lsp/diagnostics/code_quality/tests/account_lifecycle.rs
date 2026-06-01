use super::*;

#[test]
fn reports_manual_close_reinit_and_stale_cpi() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
    ctx.accounts.vault.assign(&system_program::ID);
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("account-closing"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("stale-account-after-cpi"))
    }));
}

#[test]
fn ignores_stale_cpi_text_in_comments_and_strings() {
    let source = r#"
use anchor_lang::prelude::*;

/// CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
pub fn note(ctx: Context<Note>) -> Result<()> {
    let _template = "CpiContext::new(...); ctx.accounts.vault.amount";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn reports_stale_cpi_even_with_unrelated_reload() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn helper(ctx: Context<Close>) -> Result<()> {
    ctx.accounts.vault.reload()?;
    Ok(())
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn ignores_stale_cpi_when_non_context_struct_has_accounts_field() {
    let source = r#"
use anchor_lang::prelude::*;

struct FakeContext {
    accounts: FakeAccounts,
}

struct FakeAccounts {
    vault: Vault,
}

struct Vault {
    amount: u64,
}

pub fn close(ctx: Context<Close>, fake: FakeContext) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = fake.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn accepts_stale_cpi_when_account_reloads_before_read() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    ctx.accounts.vault.reload()?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn reports_manual_close_even_with_unrelated_close_attribute() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
    ctx.accounts.vault.assign(&system_program::ID);
    Ok(())
}

#[derive(Accounts)]
pub struct UsesAnchorClose<'info> {
    #[account(mut, close = receiver)]
    pub temp: AccountInfo<'info>,
    pub receiver: SystemAccount<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "account-closing");
}

#[test]
fn ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes() {
    let source = r#"
use anchor_lang::prelude::*;

/// **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
/// ctx.accounts.vault.assign(&system_program::ID);
pub fn note(ctx: Context<Note>) -> Result<()> {
    let _template = "try_borrow_mut_lamports assign(&system_program::ID try_deserialize_unchecked";
    Ok(())
}

#[derive(Accounts)]
pub struct Note<'info> {
    #[account(constraint = note == "try_deserialize_unchecked")]
    pub note: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "account-closing");
    assert_no_attack(&diagnostics, "initialization");
}

#[test]
fn reports_try_deserialize_unchecked_initialization() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn load(account: &AccountInfo) -> Result<()> {
    let _state = State::try_deserialize_unchecked(&mut &account.data.borrow()[..])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "initialization");
}
