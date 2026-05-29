use {
    super::{completions, position_after},
    crate::document::ParsedDocument,
};

#[test]
fn completes_constraint_expression_values_after_typed_identifier_prefix() {
    let source = r#"
const SLIPPAGE_LIMIT: u64 = 1;

#[program]
pub mod demo {
    pub fn close(ctx: Context<Close>, slot: u64) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Close<'info> {
    #[account(
        constraint = s
    )]
    pub bundled_position: Account<'info, Position>,
    pub signer: Signer<'info>,
}
"#;
    let document = ParsedDocument::parse(source).unwrap();
    let completions = completions(&document, position_after(source, "constraint = s"))
        .expect("expected expression value completions");
    let labels = completions
        .iter()
        .map(|item| item.label.as_str())
        .collect::<Vec<_>>();

    assert!(labels.contains(&"signer"));
    assert!(labels.contains(&"slot"));
    assert!(labels.contains(&"SLIPPAGE_LIMIT"));
    assert!(!labels.iter().any(|label| label.starts_with("constraint")));
}
