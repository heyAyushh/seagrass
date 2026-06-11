use super::*;

const UNCHECKED_ARITHMETIC_TOPIC: &str = "seagrass/solana.code-quality.unchecked-arithmetic";

#[test]
fn reports_unchecked_token_amount_arithmetic() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let next = ctx.accounts.vault.amount - fee;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    let diagnostic = unchecked_arithmetic_diagnostic(&diagnostics);
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(evidence_source(diagnostic), Some("token-account-amount"));
}

#[test]
fn reports_unchecked_token_amount_arithmetic_through_aliases() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let vault = &ctx.accounts.vault;
        let amount = vault.amount;
        let next = amount - fee;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_eq!(
        evidence_source(unchecked_arithmetic_diagnostic(&diagnostics)),
        Some("derived-local")
    );
}

#[test]
fn reports_unchecked_token_amount_arithmetic_through_accounts_alias() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let accounts = &ctx.accounts;
        let vault = &accounts.vault;
        let amount = vault.amount;
        let next = amount - fee;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_eq!(
        evidence_source(unchecked_arithmetic_diagnostic(&diagnostics)),
        Some("derived-local")
    );
}

#[test]
fn reports_unchecked_token_amount_compound_assignment() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let mut amount = ctx.accounts.vault.amount;
        amount -= fee;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_eq!(
        evidence_source(unchecked_arithmetic_diagnostic(&diagnostics)),
        Some("derived-local")
    );
}

#[test]
fn reports_unchecked_lamports_arithmetic() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    use super::*;

    pub fn top_up(ctx: Context<TopUp>, extra: u64) -> Result<()> {
        let current = ctx.accounts.vault.to_account_info().lamports();
        let next = current + extra;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TopUp<'info> {
    pub vault: SystemAccount<'info>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_eq!(
        evidence_source(unchecked_arithmetic_diagnostic(&diagnostics)),
        Some("derived-local")
    );
}

#[test]
fn ignores_shadowed_token_amount_alias() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let amount = ctx.accounts.vault.amount;
        let amount = fee;
        let next = amount - 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

#[test]
fn ignores_name_only_amount_arithmetic() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    use super::*;

    pub fn count(ctx: Context<Count>, total_amount: u64, step: u64) -> Result<()> {
        let next = total_amount + step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Count<'info> {
    pub authority: Signer<'info>,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

#[test]
fn ignores_non_token_account_amount_field() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    use super::*;

    pub fn count(ctx: Context<Count>, step: u64) -> Result<()> {
        let next = ctx.accounts.counter.amount + step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Count<'info> {
    pub counter: Account<'info, CounterState>,
}

#[account]
pub struct CounterState {
    pub amount: u64,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

#[test]
fn ignores_same_field_name_on_non_token_context() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn count(token_ctx: Context<TokenVault>, counter_ctx: Context<CounterVault>, step: u64) -> Result<()> {
        let next = counter_ctx.accounts.vault.amount + step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TokenVault<'info> {
    pub vault: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct CounterVault<'info> {
    pub vault: Account<'info, CounterState>,
}

#[account]
pub struct CounterState {
    pub amount: u64,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

#[test]
fn ignores_same_field_name_through_non_token_accounts_alias() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn count(token_ctx: Context<TokenVault>, counter_ctx: Context<CounterVault>, step: u64) -> Result<()> {
        let accounts = &counter_ctx.accounts;
        let next = accounts.vault.amount + step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct TokenVault<'info> {
    pub vault: Account<'info, TokenAccount>,
}

#[derive(Accounts)]
pub struct CounterVault<'info> {
    pub vault: Account<'info, CounterState>,
}

#[account]
pub struct CounterState {
    pub amount: u64,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

#[test]
fn ignores_checked_token_amount_arithmetic() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token::TokenAccount;

#[program]
pub mod demo {
    use super::*;

    pub fn withdraw(ctx: Context<Withdraw>, fee: u64) -> Result<()> {
        let next = ctx.accounts.vault.amount.checked_sub(fee).ok_or(ErrorCode::Overflow)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    pub vault: Account<'info, TokenAccount>,
}

#[error_code]
pub enum ErrorCode {
    #[msg("overflow")]
    Overflow,
}
"#;
    let diagnostics = collect(&ParsedDocument::parse_or_empty(source));

    assert_no_unchecked_arithmetic(&diagnostics);
}

fn unchecked_arithmetic_diagnostic(diagnostics: &[Diagnostic]) -> &Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| has_unchecked_arithmetic_topic(diagnostic))
        .unwrap_or_else(|| panic!("expected unchecked arithmetic diagnostic: {diagnostics:#?}"))
}

fn assert_no_unchecked_arithmetic(diagnostics: &[Diagnostic]) {
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !has_unchecked_arithmetic_topic(diagnostic)),
        "unexpected unchecked arithmetic diagnostic: {diagnostics:#?}"
    );
}

fn has_unchecked_arithmetic_topic(diagnostic: &Diagnostic) -> bool {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("topic"))
        .and_then(|value| value.as_str())
        == Some(UNCHECKED_ARITHMETIC_TOPIC)
}

fn evidence_source(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic
        .data
        .as_ref()
        .and_then(|data| data.get("evidenceSource"))
        .and_then(|value| value.as_str())
}
