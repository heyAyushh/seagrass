use super::*;

#[test]
fn reports_native_signer_cpi_instruction_bounds_and_pda_seed_collision() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    let tag = instruction_data[0];
    let metas = vec![AccountMeta::new(*accounts[0].key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![tag] };
    invoke(&ix, accounts)?;
    Ok(())
}

#[derive(Accounts)]
pub struct Seeds<'info> {
    #[account(seeds = [user.key().as_ref(), mint.key().as_ref()], bump)]
    state: AccountInfo<'info>,
    user: Signer<'info>,
    mint: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    for attack in [
        "instruction-data-bounds",
        "signer-authorization",
        "arbitrary-cpi",
        "pda-seed-collision",
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "missing {attack}: {diagnostics:#?}"
        );
    }
}

#[test]
fn accepts_pda_seed_collision_with_static_domain_separator() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Seeds<'info> {
    #[account(seeds = [b"vault", user.key().as_ref(), mint.key().as_ref()], bump)]
    state: AccountInfo<'info>,
    user: Signer<'info>,
    mint: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "pda-seed-collision");
}

#[test]
fn ignores_pda_seed_collision_text_in_comments_and_strings() {
    let source = r#"
use anchor_lang::prelude::*;

fn note() {
    // seeds = [user.key().as_ref(), mint.key().as_ref()]
    let _template = "seeds = [user.key().as_ref(), mint.key().as_ref()]";
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "pda-seed-collision");
}

#[test]
fn reports_pinocchio_instruction_bounds_as_pinocchio_program_kind() {
    let source = r#"
use pinocchio::{entrypoint, AccountView, Address, ProgramResult};

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &[AccountView],
    _instruction_data: &[u8],
) -> ProgramResult {
    let tag = _instruction_data[0];
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!("instruction-data-bounds"))
        })
        .expect("expected Pinocchio bounds diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("programKind")),
        Some(&serde_json::json!("pinocchio"))
    );
}

#[test]
fn reports_modular_native_solana_instruction_bounds() {
    let source = r#"
use {
    solana_account_info::AccountInfo,
    solana_address::Address,
    solana_msg::msg,
    solana_program_error::ProgramResult,
};

solana_program_entrypoint::entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let tag = instruction_data[0];
    msg!("tag {tag}");
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!("instruction-data-bounds"))
        })
        .expect("expected native modular bounds diagnostic");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("programKind")),
        Some(&serde_json::json!("native-solana"))
    );
}
