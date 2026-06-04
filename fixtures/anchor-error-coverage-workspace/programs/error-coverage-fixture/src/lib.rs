use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::AccountMeta;

declare_id!("11111111111111111111111111111111");

#[program]
pub mod error_coverage_fixture {
    use super::*;

    pub fn exercise_static_coverage(ctx: Context<ExerciseStaticCoverage>) -> Result<()> {
        let _metas = vec![AccountMeta::new(ctx.accounts.authority.key(), true)];
        ctx.accounts.vault.amount = ctx.accounts.vault.amount.checked_add(1).unwrap();
        Ok(())
    }
}

#[derive(Accounts)]
pub struct ExerciseStaticCoverage<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
    pub authority: AccountInfo<'info>,
    pub vault: Account<'info, Vault>,
}

#[account]
pub struct State {
    pub nonce: u64,
}

#[account]
pub struct Vault {
    pub amount: u64,
}
