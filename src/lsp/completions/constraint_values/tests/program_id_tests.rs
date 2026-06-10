use {super::*, crate::document::ParsedDocument};

fn labels(items: &[CompletionItem]) -> Vec<&str> {
    items.iter().map(|item| item.label.as_str()).collect()
}

#[test]
fn offers_well_known_ids_and_pubkey_literal_for_address() {
    let source = r#"
use anchor_lang::system_program;
use anchor_spl::{associated_token, token, token_2022};
use mpl_token_metadata;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = )]
    pub program: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items =
        completions(&document, position_after(source, "address = ")).expect("address completions");
    let labels = labels(&items);

    assert!(labels.contains(&"token::ID"), "{labels:?}");
    assert!(labels.contains(&"token_2022::ID"), "{labels:?}");
    assert!(labels.contains(&"system_program::ID"), "{labels:?}");
    assert!(labels.contains(&"associated_token::ID"), "{labels:?}");
    assert!(labels.contains(&"mpl_token_metadata::ID"), "{labels:?}");
    assert!(labels.iter().any(|label| label.starts_with("pubkey!")));
}

#[test]
fn omits_well_known_ids_without_matching_imports() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(address = )]
    pub program: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items =
        completions(&document, position_after(source, "address = ")).expect("address completions");
    let labels = labels(&items);

    assert!(!labels.contains(&"token::ID"), "{labels:?}");
    assert!(!labels.contains(&"system_program::ID"), "{labels:?}");
    assert!(labels.iter().any(|label| label.starts_with("pubkey!")));
}

#[test]
fn offers_well_known_ids_for_seeds_program() {
    let source = r#"
use anchor_spl::associated_token;

#[derive(Accounts)]
pub struct Run<'info> {
    #[account(seeds = [b"x"], bump, seeds::program = )]
    pub pda: AccountInfo<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "seeds::program = "))
        .expect("seeds::program completions");
    assert!(labels(&items).contains(&"associated_token::ID"));
}

#[test]
fn omits_well_known_ids_for_boolean_constraint() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(constraint = )]
    pub mint: AccountInfo<'info>,
    pub authority: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "constraint = "));
    let has_program_id = items
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|item| item.label == "token::ID");
    assert!(
        !has_program_id,
        "constraint = should not suggest program IDs"
    );
}

#[test]
fn offers_space_discriminator_variants() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = )]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items =
        completions(&document, position_after(source, "space = ")).expect("space completions");
    let labels = labels(&items);

    assert!(labels.contains(&"8 + State::INIT_SPACE"), "{labels:?}");
    assert!(
        labels.contains(&"State::DISCRIMINATOR.len() + State::INIT_SPACE"),
        "{labels:?}"
    );
}

#[test]
fn offers_size_of_space_for_zero_copy_account() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = )]
    pub data: AccountLoader<'info, Data>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account(zero_copy)]
pub struct Data {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "space = "))
        .expect("zero-copy space completions");
    assert!(labels(&items).contains(&"8 + std::mem::size_of::<Data>()"));
}

#[test]
fn offers_discriminator_associated_const_for_account_type() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = State::)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "space = State::"))
        .expect("associated value completions");
    assert!(labels(&items).contains(&"DISCRIMINATOR"));
}
