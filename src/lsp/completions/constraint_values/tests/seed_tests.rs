use super::*;

#[test]
fn keeps_general_program_completion_for_seeds_program() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds::program = m)]
    pub metadata: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
    pub metadata_program: Program<'info, Metadata>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds::program = m")).unwrap();

    assert!(!completions
        .iter()
        .any(|item| item.label == "system_program"));
    assert!(completions
        .iter()
        .any(|item| item.label == "metadata_program"));
}

#[test]
fn completes_pda_seed_account_key_after_typed_prefix() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [u])]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds = [u")).unwrap();

    assert_eq!(completions[0].label, "user.key().as_ref()");
    assert!(!completions
        .iter()
        .any(|item| item.label == "state.key().as_ref()"));
}

#[test]
fn completes_static_seed_from_current_account_field() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [b])]
    pub vault_state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "seeds = [b")).unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "b\"vault_state\""));
}

#[test]
fn completes_pda_seed_instruction_arguments_by_type() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, name: String, id: u64, raw: Vec<u8>) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [n])]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let name_items = completions(&document, position_after(source, "seeds = [n")).unwrap();
    assert_eq!(name_items[0].label, "name.as_bytes()");

    let id_source = source.replace("seeds = [n", "seeds = [i");
    let id_document = ParsedDocument::parse(&id_source).unwrap();
    let id_items = completions(&id_document, position_after(&id_source, "seeds = [i")).unwrap();
    assert_eq!(id_items[0].label, "id.to_le_bytes().as_ref()");
    assert!(!id_items.iter().any(|item| item.label.starts_with("raw")));
}

#[test]
fn completes_pda_seed_byte_container_arguments() {
    let source = r#"
#[program]
pub mod demo {
    pub fn create(ctx: Context<Create>, raw: Vec<u8>, fixed: [u8; 32], slice: &[u8]) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [])]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "seeds = ["))
        .expect("seed completions for byte-container arguments");
    let labels = items
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"raw.as_ref()"), "Vec<u8> seed: {labels:?}");
    assert!(
        labels.contains(&"fixed.as_ref()"),
        "[u8; N] seed: {labels:?}"
    );
    assert!(labels.contains(&"slice"), "&[u8] seed: {labels:?}");
}

#[test]
fn completes_file_level_constant_seed() {
    let source = r#"
const VAULT_SEED: &[u8] = b"vault";

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [])]
    pub state: Account<'info, State>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let items = completions(&document, position_after(source, "seeds = ["))
        .expect("constant seed completions");
    assert!(items.iter().any(|item| item.label == "VAULT_SEED.as_ref()"));
}

#[test]
fn completes_empty_seed_list_after_space_or_manual_trigger() {
    let source = r#"
#[derive(Accounts)]
pub struct Create<'info> {
    #[account(seeds = [])]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();

    let items = completions(&document, position_after(source, "seeds = ["))
        .expect("expected seed completions after list opener");

    assert!(items.iter().any(|item| item.label == "user.key().as_ref()"));
}
