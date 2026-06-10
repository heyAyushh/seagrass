use super::*;

// The security-bundle test checks instruction-data-bounds, signer-authorization,
// and arbitrary-cpi together.  The pda-seed-collision check is tested separately
// below because the new trie-based detector requires two distinct PDAs whose
// literal prefixes are ambiguous — a single struct with purely-dynamic seeds
// (like [user.key(), mint.key()]) carries no literal prefix and is intentionally
// not flagged (open-world: prefer silence over a false positive).
#[test]
fn reports_native_signer_cpi_instruction_bounds() {
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
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    for attack in [
        "instruction-data-bounds",
        "signer-authorization",
        "arbitrary-cpi",
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

/// True positive: two PDAs whose leading literal bytes form a proper prefix
/// relationship — `b"pr"` is a byte-prefix of `b"product"`.  The Solana
/// runtime cannot distinguish these PDAs if the variable-length opaque
/// component happens to start with the bytes that "product" appends.
#[test]
fn reports_pda_seed_prefix_collision() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct InitProduct<'info> {
    #[account(seeds = [b"product", authority.key().as_ref()], bump)]
    product: AccountInfo<'info>,
    authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitPr<'info> {
    // b"pr" is a byte-prefix of b"product" — genuine Zellic-class collision.
    #[account(seeds = [b"pr", authority.key().as_ref()], bump)]
    pr_account: AccountInfo<'info>,
    authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    assert_has_attack(&diagnostics, "pda-seed-collision");
    // Every colliding site must be WARNING, never ERROR (Heuristic provability).
    for diagnostic in &diagnostics {
        if diagnostic
            .data
            .as_ref()
            .and_then(|d| d.get("attack"))
            == Some(&serde_json::json!("pda-seed-collision"))
        {
            assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
        }
    }
}

/// True negative: `b"escrow"` and `b"vault"` start with different bytes —
/// no prefix relationship, no diagnostic.
#[test]
fn no_collision_for_distinct_literal_prefixes() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct MakeEscrow<'info> {
    #[account(seeds = [b"escrow", maker.key().as_ref(), seed.to_le_bytes().as_ref()], bump)]
    escrow: AccountInfo<'info>,
    maker: Signer<'info>,
}

#[derive(Accounts)]
pub struct MakeVault<'info> {
    #[account(seeds = [b"vault", depositor.key().as_ref()], bump)]
    vault: AccountInfo<'info>,
    depositor: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);
    assert_no_attack(&diagnostics, "pda-seed-collision");
}

/// True negative: a single PDA with a static domain separator has no
/// second entry to collide with — trie carries one key, zero pairs.
#[test]
fn no_collision_for_single_pda_with_static_domain() {
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

/// True negative: purely-dynamic seeds `[user.key(), mint.key()]` have an
/// empty literal-byte prefix — the new detector skips them rather than
/// guessing.
#[test]
fn no_collision_for_purely_dynamic_seeds() {
    let source = r#"
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct SeedsA<'info> {
    #[account(seeds = [user.key().as_ref(), mint.key().as_ref()], bump)]
    state: AccountInfo<'info>,
    user: Signer<'info>,
    mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SeedsB<'info> {
    #[account(seeds = [owner.key().as_ref(), token.key().as_ref()], bump)]
    state: AccountInfo<'info>,
    owner: Signer<'info>,
    token: AccountInfo<'info>,
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
