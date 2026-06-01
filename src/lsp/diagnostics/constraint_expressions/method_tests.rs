use {super::collect, crate::document::ParsedDocument};

#[test]
fn reports_constraint_field_called_as_method() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = position.position_bundle_mint() == mint.key())]
    pub position: Account<'info, PositionBundle>,
    pub mint: AccountInfo<'info>,
}

#[account]
pub struct PositionBundle {
    pub position_bundle_mint: Pubkey,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("calls `position_bundle_mint` as a method")),
        "missing field-called-as-method constraint diagnostic: {diagnostics:#?}"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("`mint.key()`")),
        "account key helper methods should stay quiet: {diagnostics:#?}"
    );
}

#[test]
fn reports_unknown_account_loader_method_in_constraint() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bump: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct Run<'info> {
    #[account(constraint = position.load_typo()?.bump == bump)]
    pub position: AccountLoader<'info, PositionBundle>,
}

#[account(zero_copy)]
pub struct PositionBundle {
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("`position.load_typo()` does not resolve")),
        "missing unknown AccountLoader method diagnostic: {diagnostics:#?}"
    );
}

#[test]
fn accepts_known_constraint_methods() {
    let source = r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, bump: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(bump: u8)]
pub struct Run<'info> {
    #[account(
        constraint = position.load()?.bump == bump,
        constraint = authority.key() != Pubkey::default(),
    )]
    pub position: AccountLoader<'info, PositionBundle>,
    pub authority: Signer<'info>,
}

#[account(zero_copy)]
pub struct PositionBundle {
    pub bump: u8,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(
        diagnostics.is_empty(),
        "known constraint methods should stay quiet: {diagnostics:#?}"
    );
}
