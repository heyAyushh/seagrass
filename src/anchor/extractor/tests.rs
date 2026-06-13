use {
    super::*,
    crate::{
        document::ParsedDocument,
        semantic::{AccountType, CheckKind, ConstraintValue, PopulatedFields},
    },
};

#[test]
fn extracts_flat_accounts_struct_constraints_and_pda() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8, has_one = authority, seeds = [b"state"], bump)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub authority: UncheckedAccount<'info>,
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let create = model.accounts_struct("Create").unwrap();
    let state = create
        .fields
        .iter()
        .find(|field| field.name == "state")
        .unwrap();

    assert_eq!(state.account_type, AccountType::Account);
    assert!(state.constraints.iter().any(|constraint| {
        constraint.key == "payer"
            && constraint.value == ConstraintValue::AccountRef("payer".to_string())
    }));
    assert!(state.constraints.iter().any(|constraint| {
        constraint.key == "has_one"
            && constraint.value == ConstraintValue::AccountRef("authority".to_string())
    }));
    assert!(model.accounts_structs[0]
        .populated_fields
        .has(PopulatedFields::PDA_SEEDS));
}

#[test]
fn extracts_same_document_composite_refs() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Trade<'info> {
    pub taker: Signer<'info>,
}

#[derive(Accounts)]
pub struct CloseEscrow<'info> {
    pub trade: Trade<'info>,
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let close = model.accounts_struct("CloseEscrow").unwrap();

    assert_eq!(close.composite_refs[0].target_struct_name, "Trade");
    assert!(close.composite_refs[0].resolved.is_some());
}

#[test]
fn program_token_catalog_marks_token_interface_candidate() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;
use anchor_spl::token::Token;

#[derive(Accounts)]
pub struct Create<'info> {
    pub token_program: Program<'info, Token>,
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let token_program = model
        .accounts_struct("Create")
        .unwrap()
        .fields
        .first()
        .unwrap();

    assert_eq!(token_program.account_type, AccountType::Program);
    assert!(token_program.token_interface_candidate);
}

#[test]
fn interface_account_marks_token_interface_candidate() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;
use anchor_spl::token_interface::Mint;

#[derive(Accounts)]
pub struct Create<'info> {
    pub mint: InterfaceAccount<'info, Mint>,
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let mint = model
        .accounts_struct("Create")
        .unwrap()
        .fields
        .first()
        .unwrap();

    assert_eq!(mint.account_type, AccountType::InterfaceAccount);
    assert!(mint.token_interface_candidate);
}

#[test]
fn nested_payer_field_is_visible_from_composite_struct() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Trade<'info> {
    #[account(mut)]
    pub taker: Signer<'info>,
}

#[derive(Accounts)]
pub struct CloseEscrow<'info> {
    pub trade: Trade<'info>,
    #[account(init, payer = trade.taker, space = 8)]
    pub new_acc: Account<'info, State>,
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let names = model
        .all_fields_for_struct("CloseEscrow")
        .into_iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();

    assert!(names.contains(&"taker"));
}

#[test]
fn instruction_signer_checks_are_populated_from_symbols() {
    let document = ParsedDocument::parse(
        r#"
use anchor_lang::prelude::*;

#[program]
pub mod demo {
    pub fn check(ctx: Context<CheckAccounts>) -> Result<()> {
        require!(ctx.accounts.authority.is_signer, ErrorCode::MissingSigner);
        Ok(())
    }
}
"#,
    )
    .unwrap();

    let model = extract(&document);
    let instruction = model.instructions.first().unwrap();

    assert!(instruction
        .populated_fields
        .has(PopulatedFields::SIGNER_CHECKS));
    assert!(instruction.inner.signer_checks.iter().any(|check| {
        check.kind == CheckKind::Signer && check.subject_ref.as_deref() == Some("authority")
    }));
}
