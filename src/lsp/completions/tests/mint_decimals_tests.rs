use super::*;

#[test]
fn completes_mint_decimals_with_instruction_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, _token_decimals: u8) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = _)]
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 8,
            character: 32,
        },
    )
    .unwrap();

    assert!(completions
        .iter()
        .any(|item| item.label == "_token_decimals"));
}

#[test]
fn does_not_complete_mint_decimals_from_misleading_non_u8_argument() {
    let source = r#"
#[program]
pub mod demo {
    pub fn initialize(ctx: Context<Create>, token_decimals_label: String, token_decimals: u64) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(mint::decimals = t)]
    pub mint: Account<'info, Mint>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(
        &document,
        Position {
            line: 8,
            character: 33,
        },
    );

    assert!(completions.is_none());
}
