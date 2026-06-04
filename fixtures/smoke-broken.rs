//! Deliberately incomplete Anchor accounts used by `scripts/smoke-install.sh`.
//! Expect Seagrass to report missing `payer` and/or `space` for `#[account(init)]`.

use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}