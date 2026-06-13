use anchor_lang::prelude::*;

declare_id!("FpB2222222222222222222222222222222222222222");

#[program]
pub mod composite_payer {}

#[account]
pub struct Record {
    pub data: u64,
    pub authority: Pubkey,
}

/// Shared signer struct; payer lives here, not in the main Accounts struct.
#[derive(Accounts)]
pub struct SharedSigners<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: Signer<'info>,
}

/// Main struct that references payer from the nested sub-struct.
/// seagrass must not emit false-positive payer-mutability or
/// missing-account-reference diagnostics here.
#[derive(Accounts)]
pub struct CreateRecord<'info> {
    #[account(init, payer = signers.payer, space = 8 + 8 + 32)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
    pub system_program: Program<'info, System>,
}

/// has_one referencing a field in a nested sub-struct.
#[derive(Accounts)]
pub struct UpdateRecord<'info> {
    #[account(mut, has_one = authority)]
    pub record: Account<'info, Record>,
    pub signers: SharedSigners<'info>,
}
