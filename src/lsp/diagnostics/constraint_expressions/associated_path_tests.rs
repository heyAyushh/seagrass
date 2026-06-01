use super::collect;
use crate::document::ParsedDocument;

#[test]
fn reports_unknown_leading_module_for_associated_space_constant() {
    let source = r#"
#[derive(Accounts)]
pub struct Run<'info> {
    #[account(init, payer = payer, space = fake::State::SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
pub struct State {
    pub value: u64,
}

impl State {
    const SPACE: usize = 8 + 8;
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let diagnostics = collect(&document);

    assert!(diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`fake::State::SPACE` does not resolve")));
}
