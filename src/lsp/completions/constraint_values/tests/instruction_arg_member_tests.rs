use {
    super::*,
    crate::{
        document::ParsedDocument,
        lsp::completions::{proptest_support::rust_identifier, should_offer_completion},
    },
    proptest::prelude::*,
};

#[test]
fn completes_instruction_argument_struct_members() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(constraint = params.position_m)]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "params.position_m"))
        .expect("instruction argument member completions");

    assert_eq!(completions[0].label, "position_mint");
}

#[test]
fn wakes_and_completes_instruction_argument_struct_members_after_dot() {
    let source = r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(constraint = params.)]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#;
    let position = position_after(source, "params.");
    assert!(should_offer_completion(source, position));

    let document = ParsedDocument::parse(source).unwrap();
    let completions =
        completions(&document, position).expect("instruction argument member completions");
    let labels = completions
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_mint"));
    assert!(labels.contains(&"position_bitmap"));
}

proptest! {
    #[test]
    fn completes_generated_instruction_argument_struct_members(
        instruction_arg in rust_identifier(),
        owner in "[A-Z][A-Za-z0-9_]{1,10}",
        field in rust_identifier(),
    ) {
        let prefix = field.chars().next().unwrap_or_default().to_string();
        let source = format!(
            r#"
#[program]
pub mod demo {{
    pub fn run(ctx: Context<Run>, {instruction_arg}: {owner}) -> Result<()> {{ Ok(()) }}
}}

#[derive(Accounts)]
#[instruction({instruction_arg}: {owner})]
pub struct Run<'info> {{
    #[account(constraint = {instruction_arg}.{prefix})]
    pub mint: AccountInfo<'info>,
}}

pub struct {owner} {{
    pub {field}: Pubkey,
}}
"#
        );
        let document = ParsedDocument::parse(&source).unwrap();
        let completion_line = format!("{instruction_arg}.{prefix}");
        let completions = completions(&document, position_after(&source, &completion_line))
            .expect("generated instruction argument member completions");

        prop_assert!(
            completions.iter().any(|item| item.label == field),
            "expected generated instruction arg member completion, got {completions:#?}"
        );
    }
}
