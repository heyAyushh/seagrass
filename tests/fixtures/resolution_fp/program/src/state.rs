use anchor_lang::prelude::*;

pub const SPACE: usize = 8 + State::INIT_SPACE;

#[account]
#[derive(InitSpace)]
pub struct State {
    pub bump: u8,
}
