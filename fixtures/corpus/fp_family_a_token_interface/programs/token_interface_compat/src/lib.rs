use anchor_lang::prelude::*;
use anchor_spl::{token::Token, token_2022::Token2022, token_interface::TokenInterface};

declare_id!("FpA1111111111111111111111111111111111111111");

#[program]
pub mod token_interface_compat {}

/// Accepts Program<Token2022> as a valid token program for Account<TokenAccount> init.
#[derive(Accounts)]
pub struct InitWithToken2022<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, anchor_spl::token::TokenAccount>,
    pub mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token2022>,
    pub system_program: Program<'info, System>,
}

/// Accepts Interface<TokenInterface> as a valid token program for Account<TokenAccount> init.
#[derive(Accounts)]
pub struct InitWithTokenInterface<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer)]
    pub token: Account<'info, anchor_spl::token::TokenAccount>,
    pub mint: Account<'info, anchor_spl::token::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}

/// InterfaceAccount with Interface<TokenInterface> must still pass.
#[derive(Accounts)]
pub struct InitInterfaceAccount<'info> {
    #[account(init, payer = payer, token::mint = mint, token::authority = payer, token::token_program = token_program)]
    pub token: InterfaceAccount<'info, anchor_spl::token_interface::TokenAccount>,
    pub mint: InterfaceAccount<'info, anchor_spl::token_interface::Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Interface<'info, TokenInterface>,
    pub system_program: Program<'info, System>,
}
