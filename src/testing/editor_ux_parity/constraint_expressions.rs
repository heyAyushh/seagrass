use {
    super::strip_markers,
    crate::{completions, diagnostics, document::ParsedDocument},
};

#[test]
fn editor_ux_resolves_instruction_argument_members_in_constraints() {
    let marked = strip_markers(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(
        constraint = params.position_m/*caret:completion*/ == mint.key(),
        constraint = params.missing == mint.key(),
    )]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );
    let document = ParsedDocument::parse_or_empty(&marked.source);
    let completions = completions::completions(&document, marked.positions["completion"])
        .expect("instruction argument member completions");

    assert_eq!(completions[0].label, "position_mint");

    let diagnostics = diagnostics::collect(&document);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("`params.missing`")),
        "unknown instruction argument member should be flagged: {diagnostics:#?}"
    );
}

#[test]
fn editor_ux_wakes_instruction_argument_members_after_dot() {
    let marked = strip_markers(
        r#"
#[program]
pub mod demo {
    pub fn run(ctx: Context<Run>, params: RunParams) -> Result<()> { Ok(()) }
}

#[derive(Accounts)]
#[instruction(params: RunParams)]
pub struct Run<'info> {
    #[account(constraint = params./*caret:completion*/ == mint.key())]
    pub mint: AccountInfo<'info>,
}

pub struct RunParams {
    pub position_mint: Pubkey,
    pub position_bitmap: [u8; 32],
}
"#,
    );
    let document = ParsedDocument::parse_or_empty(&marked.source);
    let completions = completions::completions(&document, marked.positions["completion"])
        .expect("empty-prefix instruction argument member completions");
    let labels = completions
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"position_mint"));
    assert!(labels.contains(&"position_bitmap"));
}
