use {
    super::*,
    tower_lsp::lsp_types::{DiagnosticSeverity, NumberOrString},
};

mod instruction_bounds;
mod native_validation;
mod unsafe_unwrap;

#[test]
fn reports_unchecked_balance_arithmetic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Use checked arithmetic")));
}

#[test]
fn ignores_deref_in_account_attribute() {
    let source = r#"
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{Interface, TokenInterface};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(address = *token_mint_a.to_account_info().owner)]
    pub token_program_a: Interface<'info, TokenInterface>,
    #[account(address = *token_mint_b.to_account_info().owner)]
    pub token_program_b: Interface<'info, TokenInterface>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_unary_deref_in_executable_body() {
    let source = r#"
use solana_program::program_error::ProgramError;

fn read(ptr: &u64) -> Result<u64, ProgramError> {
    Ok(*ptr)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_token_word_inside_unrelated_identifier() {
    let source = r#"
use solana_program::program_error::ProgramError;

fn process(token_mint_a: u64, other: u64) -> Result<(), ProgramError> {
    let _difference = token_mint_a - other;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn ignores_arithmetic_text_in_comments_and_strings() {
    let source = r#"
use solana_program::program_error::ProgramError;

/// amount - fee is checked by caller.
fn process() -> Result<(), ProgramError> {
    let message = "token - fee";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert_no_checked_arithmetic_diagnostic(&document);
}

#[test]
fn diagnostic_source_is_seagrass() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostic = collect(&document)
        .into_iter()
        .find(|diagnostic| diagnostic.message.contains("Use checked arithmetic"))
        .expect("expected unchecked arithmetic diagnostic");

    assert_eq!(diagnostic.source.as_deref(), Some("seagrass"));
    let data = diagnostic.data.as_ref().expect("expected diagnostic data");
    assert_eq!(data["rule"], "unchecked-arithmetic");
    assert_eq!(data["confidence"], "heuristic");
    assert_eq!(
        data["topic"],
        "seagrass/solana.code-quality.unchecked-arithmetic"
    );
    assert_eq!(data["applicability"], "Unspecified");
}

#[test]
fn resolved_framework_context_drives_code_quality_without_rescanning_imports() {
    let source = r#"
fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    Ok(amount - fee)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect_with_framework(
        &document,
        crate::solana::frameworks::FrameworkContext::new(
            crate::solana::frameworks::FrameworkId::Pinocchio,
        ),
    );

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("Use checked arithmetic"))
        .expect("expected unchecked arithmetic diagnostic from resolved framework context");

    assert_eq!(
        diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("programKind")),
        Some(&serde_json::json!("pinocchio"))
    );
}

#[test]
fn unchecked_arithmetic_declares_lint_contract() {
    assert_eq!(
        unchecked_arithmetic_scope(),
        &[
            crate::diagnostics::lint::Region::InstructionBody,
            crate::diagnostics::lint::Region::HelperFnBody,
        ]
    );
    assert_eq!(unchecked_arithmetic_confidence().as_str(), "heuristic");
    assert_eq!(unchecked_arithmetic_applicability().as_str(), "Unspecified");
    assert_eq!(
        unchecked_arithmetic_topic(),
        "seagrass/solana.code-quality.unchecked-arithmetic"
    );
}

#[test]
fn ignores_checked_balance_arithmetic() {
    let source = r#"
use pinocchio::program_error::ProgramError;

fn withdraw(amount: u64, fee: u64) -> Result<u64, ProgramError> {
    amount.checked_sub(fee).ok_or(ProgramError::InvalidArgument)
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}

fn assert_no_checked_arithmetic_diagnostic(document: &ParsedDocument) {
    assert!(!collect(document)
        .iter()
        .any(|diagnostic| diagnostic.message.contains("Use checked arithmetic")));
}

#[test]
fn reports_non_canonical_pda_bump() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn set_value(ctx: Context<SetValue>, key: u64, bump: u8) -> ProgramResult {
    let address = Pubkey::create_program_address(&[key.to_le_bytes().as_ref(), &[bump]], ctx.program_id)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.message.contains("canonical PDA bump"))
        .expect("expected PDA bump canonicalization diagnostic");
    assert_eq!(diagnostic.severity, Some(DiagnosticSeverity::WARNING));
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String("solana-code-quality".to_string()))
    );
    let data = diagnostic.data.as_ref().expect("expected diagnostic data");
    assert_eq!(data["attack"], "bump-seed-canonicalization");
    assert_eq!(data["corpusMode"], "legacy-invariant");
    assert_eq!(data["absorbedFrom"], "coral-xyz/sealevel-attacks");
}

#[test]
fn ignores_canonical_find_program_address() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn set_value(ctx: Context<SetValue>, key: u64) -> ProgramResult {
    let (_address, _bump) = Pubkey::find_program_address(&[key.to_le_bytes().as_ref()], ctx.program_id);
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);

    assert!(collect(&document).is_empty());
}

#[test]
fn ignores_create_program_address_text_in_comments_strings_and_attributes() {
    let source = r#"
use anchor_lang::prelude::*;

// Pubkey::create_program_address accepts any bump; do not lint comments.
pub fn note() -> Result<()> {
    let _template = "Pubkey::create_program_address(&[seed], program_id)";
    Ok(())
}

#[derive(Accounts)]
pub struct SetValue<'info> {
    #[account(constraint = note == "create_program_address")]
    pub note: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "bump-seed-canonicalization");
}

#[test]
fn ignores_project_helper_named_create_program_address() {
    let source = r#"
use anchor_lang::prelude::*;

mod helpers {
    use anchor_lang::prelude::*;

    pub fn create_program_address(_seed: &[u8]) -> Pubkey {
        Pubkey::default()
    }
}

pub fn set_value(seed: &[u8]) -> Result<()> {
    let _address = helpers::create_program_address(seed);
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "bump-seed-canonicalization");
}

#[test]
fn reports_native_raw_account_owner_and_type_invariants() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let account = &_accounts[0];
    let data = account.try_borrow_data()?;
    let state = State::try_from_slice(&data)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("owner-checks"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("type-cosplay"))
    }));
}

#[test]
fn accepts_native_raw_account_with_owner_and_discriminator_checks() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let account = &_accounts[0];
    if account.owner != &crate::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    let data = account.try_borrow_data()?;
    if &data[..8] != State::DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    let state = State::try_from_slice(&data[8..])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(!diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.data.as_ref().and_then(|data| data.get("attack")),
            Some(value) if value == "owner-checks" || value == "type-cosplay"
        )
    }));
}

#[test]
fn ignores_raw_account_text_in_comments_and_strings() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

/// account.try_borrow_data()? and State::try_from_slice(&data)? are docs.
pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    let _template = "account.try_borrow_data()?; State::try_from_slice(&data)?";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_raw_account_text_inside_dead_macro() {
    let source = r#"
use solana_program::{account_info::AccountInfo, entrypoint::ProgramResult};

macro_rules! dead_account_template {
    () => {
        let data = account.try_borrow_data()?;
        let state = State::try_from_slice(&data)?;
    };
}

pub fn process(_accounts: &[AccountInfo]) -> ProgramResult {
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_non_account_try_borrow_data_method() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

struct Buffer;

impl Buffer {
    fn try_borrow_data(&self) -> Result<&[u8], solana_program::program_error::ProgramError> {
        Ok(&[])
    }
}

pub fn process(buffer: Buffer) -> ProgramResult {
    let _data = buffer.try_borrow_data()?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_account_info_substring_inside_non_account_type() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

struct AccountInfoTemplate;

impl AccountInfoTemplate {
    fn try_borrow_data(&self) -> Result<&[u8], solana_program::program_error::ProgramError> {
        Ok(&[])
    }
}

pub fn process(template: AccountInfoTemplate) -> ProgramResult {
    let _data = template.try_borrow_data()?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn ignores_deserialize_text_without_raw_account_data_read() {
    let source = r#"
use solana_program::entrypoint::ProgramResult;

pub fn process(instruction_data: &[u8]) -> ProgramResult {
    let _state = State::try_from_slice(instruction_data)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "owner-checks");
    assert_no_attack(&diagnostics, "type-cosplay");
}

#[test]
fn accepts_project_helper_owner_type_signer_program_and_bounds_checks() {
    let source = r#"
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {
    validate_owner(&accounts[0])?;
    validate_type(&accounts[0])?;
    validate_signer(&accounts[0])?;
    validate_program_id(program_id)?;
    validate_instruction_data(instruction_data)?;
    let tag = instruction_data[0];
    let account = &accounts[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    let metas = vec![AccountMeta::new(*account.key, true)];
    let ix = Instruction { program_id: *program_id, accounts: metas, data: vec![tag] };
    invoke(&ix, accounts)?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    for attack in [
        "owner-checks",
        "type-cosplay",
        "instruction-data-bounds",
        "signer-authorization",
        "arbitrary-cpi",
    ] {
        assert!(
            !diagnostics.iter().any(|diagnostic| {
                diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                    == Some(&serde_json::json!(attack))
            }),
            "unexpected {attack}: {diagnostics:#?}"
        );
    }
}

#[test]
fn reports_manual_close_reinit_and_stale_cpi() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
    ctx.accounts.vault.assign(&system_program::ID);
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("account-closing"))
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.data.as_ref().and_then(|data| data.get("attack"))
            == Some(&serde_json::json!("stale-account-after-cpi"))
    }));
}

#[test]
fn ignores_stale_cpi_text_in_comments_and_strings() {
    let source = r#"
use anchor_lang::prelude::*;

/// CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
pub fn note(ctx: Context<Note>) -> Result<()> {
    let _template = "CpiContext::new(...); ctx.accounts.vault.amount";
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn reports_stale_cpi_even_with_unrelated_reload() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn helper(ctx: Context<Close>) -> Result<()> {
    ctx.accounts.vault.reload()?;
    Ok(())
}

pub fn close(ctx: Context<Close>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn ignores_stale_cpi_when_non_context_struct_has_accounts_field() {
    let source = r#"
use anchor_lang::prelude::*;

struct FakeContext {
    accounts: FakeAccounts,
}

struct FakeAccounts {
    vault: Vault,
}

struct Vault {
    amount: u64,
}

pub fn close(ctx: Context<Close>, fake: FakeContext) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    let amount = fake.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn accepts_stale_cpi_when_account_reloads_before_read() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
    token::transfer(cpi_ctx, 1)?;
    ctx.accounts.vault.reload()?;
    let amount = ctx.accounts.vault.amount;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "stale-account-after-cpi");
}

#[test]
fn reports_manual_close_even_with_unrelated_close_attribute() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn close(ctx: Context<Close>) -> Result<()> {
    **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
    ctx.accounts.vault.assign(&system_program::ID);
    Ok(())
}

#[derive(Accounts)]
pub struct UsesAnchorClose<'info> {
    #[account(mut, close = receiver)]
    pub temp: AccountInfo<'info>,
    pub receiver: SystemAccount<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "account-closing");
}

#[test]
fn ignores_manual_close_and_reinit_text_in_comments_strings_and_attributes() {
    let source = r#"
use anchor_lang::prelude::*;

/// **ctx.accounts.vault.try_borrow_mut_lamports()? = 0;
/// ctx.accounts.vault.assign(&system_program::ID);
pub fn note(ctx: Context<Note>) -> Result<()> {
    let _template = "try_borrow_mut_lamports assign(&system_program::ID try_deserialize_unchecked";
    Ok(())
}

#[derive(Accounts)]
pub struct Note<'info> {
    #[account(constraint = note == "try_deserialize_unchecked")]
    pub note: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_no_attack(&diagnostics, "account-closing");
    assert_no_attack(&diagnostics, "initialization");
}

#[test]
fn reports_try_deserialize_unchecked_initialization() {
    let source = r#"
use anchor_lang::prelude::*;

pub fn load(account: &AccountInfo) -> Result<()> {
    let _state = State::try_deserialize_unchecked(&mut &account.data.borrow()[..])?;
    Ok(())
}
"#;
    let document = ParsedDocument::parse_or_empty(source);
    let diagnostics = collect(&document);

    assert_has_attack(&diagnostics, "initialization");
}

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

fn assert_no_attack(diagnostics: &[Diagnostic], attack: &str) {
    assert!(
        !diagnostics.iter().any(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!(attack))
        }),
        "unexpected {attack}: {diagnostics:#?}"
    );
}

fn assert_has_attack(diagnostics: &[Diagnostic], attack: &str) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.data.as_ref().and_then(|data| data.get("attack"))
                == Some(&serde_json::json!(attack))
        }),
        "missing {attack}: {diagnostics:#?}"
    );
}
