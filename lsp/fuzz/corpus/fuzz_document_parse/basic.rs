use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8)]
    pub state: Account<'info, State>,
    pub payer: Signer<'info>,
}
